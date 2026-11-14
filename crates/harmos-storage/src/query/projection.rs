/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports
use arrow_array::RecordBatch;
use serde::{Deserialize, Serialize};

// Local Imports
use super::columnar;
use crate::{BackendSnapshot, Error, Identifier, ReadLimits, Result, StorageBackend, StorageCursor};
use harmos_query::columnar::{ArrowScan, Cancellation, ColumnSource};
use harmos_query::time_buckets;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectionDefinition {
    pub name: Identifier,
    pub version: u32,
    pub source: ColumnSource,
    pub transform: ProjectionTransform,
}

#[derive(Clone, Debug)]
pub struct ProjectionRead {
    pub name: Identifier,
    pub revision: u64,
    pub frontier: StorageCursor,
    pub after: usize,
    pub limits: ReadLimits,
    pub work: ReadLimits,
}

#[derive(Debug)]
pub struct ProjectionPage {
    pub batch: RecordBatch,
    pub frontier: StorageCursor,
    pub revision: u64,
    pub changed_keys: Vec<Vec<u8>>,
    pub next: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionStatus {
    pub frontier: StorageCursor,
    pub revision: u64,
    pub rows: usize,
    pub changed_keys: usize,
}

#[derive(Clone)]
pub struct ProjectionPublication {
    snapshot: BackendSnapshot,
    definition: ProjectionDefinition,
    revision: u64,
    batches: Vec<RecordBatch>,
    changed_keys: Vec<Vec<u8>>,
    cancel: Cancellation,
    previous: Option<StorageCursor>,
}

#[derive(Default)]
pub struct ProjectionCatalog {
    active: BTreeMap<Identifier, u64>,
    revisions: BTreeMap<(Identifier, u64), Revision>,
}

struct Revision {
    definition: ProjectionDefinition,
    retired: Option<u64>,
    outputs: BTreeMap<u64, StoredOutput>,
}

#[derive(Clone)]
struct StoredOutput {
    frontier: StorageCursor,
    batches: Vec<RecordBatch>,
    changed_keys: Vec<Vec<u8>>,
}

struct CancelWork(Cancellation);

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjectionTransform {
    Select(Vec<String>),
    TimeBuckets(time_buckets::TimeBucketPlan),
}

/* === Traits === */

/* === Implementations === */

impl Drop for CancelWork {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl ProjectionDefinition {
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 {
            return Err(columnar::invalid("projection version must be nonzero"));
        }
        let columns = match &self.transform {
            ProjectionTransform::Select(columns) => columns.clone(),
            ProjectionTransform::TimeBuckets(plan) => bucket_columns(plan),
        };
        ArrowScan {
            source: self.source.clone(),
            columns,
            predicate: None,
            time: None,
            start: 0,
            after: None,
            limits: ReadLimits {
                rows: std::num::NonZeroUsize::MIN,
                bytes: std::num::NonZeroUsize::MIN,
            },
            work: ReadLimits {
                rows: std::num::NonZeroUsize::MIN,
                bytes: std::num::NonZeroUsize::MIN,
            },
        }
        .validate()
        .map_err(Into::into)
    }
}

impl ProjectionPublication {
    pub fn snapshot(&self) -> &BackendSnapshot {
        &self.snapshot
    }
    pub fn definition(&self) -> &ProjectionDefinition {
        &self.definition
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }
    pub fn changed_keys(&self) -> &[Vec<u8>] {
        &self.changed_keys
    }
    pub fn expected_frontier(&self) -> Option<&StorageCursor> {
        self.previous.as_ref()
    }
    pub fn check_cancelled(&self) -> Result<()> {
        Ok(self.cancel.check()?)
    }
}

impl ProjectionCatalog {
    pub fn register(&mut self, definition: &ProjectionDefinition, expected: Option<u64>, rebuild: bool) -> Result<u64> {
        definition.validate()?;
        if self.active.get(&definition.name).copied() != expected {
            return Err(conflict("set projection definition: get again and retry"));
        }
        let mut revision = 1;
        if let Some(current) = self.active.get(&definition.name) {
            let prior = self.revisions.get(&(definition.name.clone(), *current)).expect("active revision exists");
            if prior.definition == *definition {
                return Ok(*current);
            }
            if !rebuild {
                return Err(conflict("changed projection requires explicit rebuild"));
            }
            revision = current.checked_add(1).ok_or_else(|| columnar::invalid("projection revision overflow"))?;
        }
        self.revisions.insert(
            (definition.name.clone(), revision),
            Revision {
                definition: definition.clone(),
                retired: None,
                outputs: BTreeMap::new(),
            },
        );
        self.active.insert(definition.name.clone(), revision);
        Ok(revision)
    }
    pub fn definition(&self, name: &Identifier) -> Result<(ProjectionDefinition, u64)> {
        let revision = *self.active.get(name).ok_or(Error::NotReady { domain: "projection definition" })?;
        Ok((self.revisions.get(&(name.clone(), revision)).expect("active revision exists").definition.clone(), revision))
    }
    pub fn definition_at(&self, name: &Identifier, revision: u64) -> Result<ProjectionDefinition> {
        Ok(self.revision(name, revision)?.definition.clone())
    }
    pub fn status(&self, name: &Identifier, revision: u64, frontier: &StorageCursor) -> Result<Option<ProjectionStatus>> {
        let definition = self.revision(name, revision)?;
        check_retired(definition.retired, frontier.sequence)?;
        Ok(definition.outputs.get(&frontier.sequence).map(|output| ProjectionStatus {
            frontier: output.frontier.clone(),
            revision,
            rows: output.batches.iter().map(RecordBatch::num_rows).sum(),
            changed_keys: output.changed_keys.len(),
        }))
    }
    pub fn frontier(&self, name: &Identifier, revision: u64) -> Result<Option<StorageCursor>> {
        Ok(self.revision(name, revision)?.outputs.last_key_value().map(|(_, output)| output.frontier.clone()))
    }
    pub fn publish(&mut self, publication: &ProjectionPublication) -> Result<()> {
        publication.check_cancelled()?;
        let (definition, revision) = self.definition(&publication.definition.name)?;
        if revision != publication.revision || definition != publication.definition {
            return Err(conflict("projection definition changed during advancement"));
        }
        let target = self.revisions.get_mut(&(definition.name, revision)).expect("active revision exists");
        check_retired(target.retired, publication.snapshot.at.sequence)?;
        if target.outputs.contains_key(&publication.snapshot.at.sequence) {
            return Ok(());
        }
        if target.outputs.last_key_value().is_some_and(|(cut, _)| *cut > publication.snapshot.at.sequence) {
            return Err(conflict("projection frontiers must advance monotonically"));
        }
        if target.outputs.last_key_value().map(|(_, output)| &output.frontier) != publication.expected_frontier() {
            return Err(conflict("projection frontier changed during advancement"));
        }
        target.outputs.insert(
            publication.snapshot.at.sequence,
            StoredOutput {
                frontier: publication.snapshot.at.clone(),
                batches: publication.batches.clone(),
                changed_keys: publication.changed_keys.clone(),
            },
        );
        Ok(())
    }
    pub fn read(&self, read: &ProjectionRead) -> Result<ProjectionPage> {
        let definition = self.revision(&read.name, read.revision)?;
        check_retired(definition.retired, read.frontier.sequence)?;
        let output = definition.outputs.get(&read.frontier.sequence).ok_or(Error::NotReady { domain: "projection frontier" })?;
        read.frontier.history.check(&output.frontier.history)?;
        let total = output.batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        let mut start = 0;
        let mut selected = &output.batches[0];
        for batch in &output.batches {
            if read.after < start + batch.num_rows() {
                selected = batch;
                break;
            }
            start += batch.num_rows();
        }
        if read.after >= total {
            selected = &output.batches[0];
            start = read.after;
        }
        let empty = RecordBatch::new_empty(selected.schema());
        if read.after >= total {
            selected = &empty;
        }
        let keys = output.changed_keys.get(read.after..).unwrap_or(&[]);
        page(read, selected, start, keys, total, output.changed_keys.len())
    }
    pub fn retire(&mut self, name: &Identifier, revision: u64, through: &StorageCursor) -> Result<()> {
        let definition = self.revisions.get_mut(&(name.clone(), revision)).ok_or(Error::NotReady { domain: "projection revision" })?;
        definition.outputs.retain(|cut, _| *cut > through.sequence);
        definition.retired = Some(definition.retired.unwrap_or(0).max(through.sequence));
        Ok(())
    }
    fn revision(&self, name: &Identifier, revision: u64) -> Result<&Revision> {
        self.revisions.get(&(name.clone(), revision)).ok_or(Error::NotReady { domain: "projection revision" })
    }
}

/* === Public Functions === */

pub async fn advance_projection(
    backend: &dyn StorageBackend, snapshot: &BackendSnapshot, name: &Identifier, expected: Option<u64>, work: ReadLimits,
    cancel: &Cancellation,
) -> Result<u64> {
    cancel.check()?;
    let native_cancel = cancel.child();
    let native_guard = CancelWork(native_cancel.clone());
    let native = backend.advance_projection(snapshot, name, expected, work, &native_cancel).await?;
    drop(native_guard);
    if let Some(revision) = native {
        return Ok(revision);
    }
    let (definition, revision) = backend.get_projection_definition(name, None).await?;
    if expected.is_some_and(|expected| expected != revision) {
        return Err(conflict("projection rebuilt before advancement"));
    }
    backend
        .list_schemas(
            snapshot,
            &super::SchemaSelection {
                schema: Some(definition.source.schema.clone()),
            },
            crate::ScanPage {
                after: 0,
                rows: std::num::NonZeroUsize::MIN,
                bytes: work.bytes,
            },
        )
        .await?;
    if backend.get_projection_status(name, revision, Some(&snapshot.at)).await?.is_some() {
        return Ok(revision);
    }
    let previous = backend.get_projection_status(name, revision, None).await?.map(|status| status.frontier);
    if previous.as_ref().is_some_and(|frontier| frontier.sequence > snapshot.at.sequence) {
        return Err(conflict("projection frontiers must advance monotonically"));
    }
    let columns = match &definition.transform {
        ProjectionTransform::Select(columns) => columns.clone(),
        ProjectionTransform::TimeBuckets(plan) => bucket_columns(plan),
    };
    let mut scan = ArrowScan {
        source: definition.source.clone(),
        columns,
        predicate: None,
        time: None,
        start: 0,
        after: None,
        limits: work,
        work,
    };
    let mut rows = 0usize;
    let mut bytes = 0usize;
    let mut retained = 0usize;
    let mut batches = Vec::new();
    loop {
        cancel.check()?;
        if rows >= work.rows.get() || bytes.saturating_add(retained) >= work.bytes.get() {
            return Err(columnar::exhausted());
        }
        scan.work = ReadLimits {
            rows: std::num::NonZeroUsize::new(work.rows.get() - rows).unwrap(),
            bytes: std::num::NonZeroUsize::new(work.bytes.get() - bytes - retained).unwrap(),
        };
        scan.limits = scan.work;
        let page = columnar::list_column_rows(backend, snapshot, &scan, cancel).await?;
        rows += page.scanned;
        bytes = bytes.saturating_add(page.bytes_read);
        retained = retained.saturating_add(page.batch.get_array_memory_size());
        if bytes.saturating_add(retained) > work.bytes.get() {
            return Err(columnar::exhausted());
        }
        batches.push(page.batch);
        if page.complete {
            break;
        }
        scan.after = page.next;
    }
    let (batches, changed_keys) = match &definition.transform {
        ProjectionTransform::Select(_) => (batches, vec![]),
        ProjectionTransform::TimeBuckets(plan) => {
            let worker_cancel = cancel.child();
            let guard = CancelWork(worker_cancel.clone());
            let plan = plan.clone();
            let context = crate::execution::Context::current();
            let output = tokio::task::spawn_blocking(move || {
                context.blocking(|| -> Result<RecordBatch> {
                    let mut accumulator = time_buckets::Accumulator::new(&plan, batches[0].schema())?;
                    for batch in &batches {
                        accumulator.push(batch, &mut || worker_cancel.check())?;
                        if bytes.saturating_add(retained).saturating_add(accumulator.memory_bytes()) > work.bytes.get() {
                            return Err(columnar::exhausted());
                        }
                    }
                    drop(batches);
                    Ok(accumulator.finish(&mut || worker_cancel.check())?)
                })
            })
            .await
            .map_err(|source| Error::backend("projection worker", crate::BackendFailure::Permanent, source))??;
            drop(guard);
            let free = work.bytes.get().saturating_sub(bytes).saturating_sub(output.get_array_memory_size());
            let budget = free / 4;
            let current = time_buckets::fingerprints(&output, budget)?;
            let mut prior = BTreeMap::new();
            let mut prior_key_bytes = 0usize;
            if let Some(frontier) = &previous {
                let mut read = ProjectionRead {
                    name: name.clone(),
                    revision,
                    frontier: frontier.clone(),
                    after: 0,
                    limits: ReadLimits {
                        rows: work.rows,
                        bytes: std::num::NonZeroUsize::new(budget).ok_or_else(columnar::exhausted)?,
                    },
                    work,
                };
                loop {
                    cancel.check()?;
                    let page = backend.list_projection_rows(&read).await?;
                    for (key, value) in time_buckets::fingerprints(&page.batch, budget)? {
                        prior_key_bytes = prior_key_bytes.saturating_add(key.capacity());
                        prior.insert(key, value);
                        let used = crate::btree_map_memory::<Vec<u8>, [u8; 32]>(prior.len()).saturating_add(prior_key_bytes);
                        if used > budget {
                            return Err(columnar::exhausted());
                        }
                    }
                    let Some(next) = page.next else {
                        break;
                    };
                    read.after = next;
                }
            }
            let mut changed = Vec::new();
            let mut used = 0usize;
            for (key, value) in &current {
                if prior.get(key) != Some(value) {
                    used = used.saturating_add(key.capacity() + std::mem::size_of::<Vec<u8>>());
                    if used > budget {
                        return Err(columnar::exhausted());
                    }
                    changed.push(key.clone());
                }
            }
            for key in prior.keys() {
                if !current.contains_key(key) {
                    used = used.saturating_add(key.capacity() + std::mem::size_of::<Vec<u8>>());
                    if used > budget {
                        return Err(columnar::exhausted());
                    }
                    changed.push(key.clone());
                }
            }
            changed.sort();
            (vec![output], changed)
        }
    };
    let publication = ProjectionPublication {
        snapshot: snapshot.clone(),
        definition,
        revision,
        batches,
        changed_keys,
        cancel: cancel.clone(),
        previous,
    };
    backend.set_projection_output(publication).await?;
    Ok(revision)
}

/* === Private Functions === */

pub(crate) fn page(
    read: &ProjectionRead, batch: &RecordBatch, start: usize, keys: &[Vec<u8>], total_rows: usize, total_keys: usize,
) -> Result<ProjectionPage> {
    if batch.num_rows() > read.work.rows.get() || batch.get_array_memory_size().saturating_mul(3) > read.work.bytes.get() {
        return Err(columnar::exhausted());
    }
    let offset = read.after.saturating_sub(start).min(batch.num_rows());
    let mut available = if read.after < total_rows {
        batch.num_rows() - offset
    } else {
        keys.len().min(read.work.rows.get()).min(total_keys.saturating_sub(read.after))
    };
    if read.after < total_keys {
        available = available.min(keys.len());
    }
    let mut low = 0;
    let mut high = available.min(read.limits.rows.get());
    while low < high {
        let count = low + (high - low).div_ceil(2);
        let indices = (offset..offset + count.min(batch.num_rows() - offset)).map(|row| row as u64).collect::<Vec<_>>();
        let candidate = harmos_query::columnar::take(batch, &indices)?;
        let bytes = harmos_query::columnar::page_bytes(&candidate)?
            .saturating_add(keys[..count.min(keys.len())].iter().map(|key| key.capacity() + std::mem::size_of::<Vec<u8>>()).sum::<usize>());
        if bytes <= read.limits.bytes.get() {
            low = count;
        } else {
            high = count - 1;
        }
    }
    if available != 0 && low == 0 {
        return Err(columnar::exhausted());
    }
    let indices = (offset..offset + low.min(batch.num_rows() - offset)).map(|row| row as u64).collect::<Vec<_>>();
    let batch = harmos_query::columnar::take(batch, &indices)?;
    let changed_keys = keys[..low.min(keys.len())].to_vec();
    let next = (read.after.saturating_add(low) < total_rows.max(total_keys)).then_some(read.after + low);
    Ok(ProjectionPage {
        batch,
        frontier: read.frontier.clone(),
        revision: read.revision,
        changed_keys,
        next,
    })
}

pub(crate) fn check_retired(retired: Option<u64>, requested: u64) -> Result<()> {
    if let Some(through) = retired
        && requested <= through
    {
        return Err(Error::Gap {
            requested,
            available: through.saturating_add(1),
        });
    }

    Ok(())
}
fn bucket_columns(plan: &time_buckets::TimeBucketPlan) -> Vec<String> {
    let mut columns = plan.group.clone();
    columns.extend([plan.time.clone(), plan.value.clone(), plan.identity.clone(), "_storage.position".into()]);
    columns.sort();
    columns.dedup();
    columns
}

fn conflict(reason: &'static str) -> Error {
    Error::backend("storage projection", crate::BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidInput, reason))
}

/* === Tests === */

impl crate::MemorySize for ProjectionDefinition {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(crate::MemorySize::memory_size(&self.name))
            .saturating_add(crate::MemorySize::memory_size(&self.source))
            .saturating_add(crate::MemorySize::memory_size(&self.transform))
    }
}

impl crate::MemorySize for ProjectionStatus {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(crate::MemorySize::memory_size(&self.frontier))
    }
}

impl crate::MemorySize for ProjectionPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.batch.get_array_memory_size())
            .saturating_add(crate::MemorySize::memory_size(&self.changed_keys))
    }
}

impl crate::MemorySize for ProjectionTransform {
    fn memory_size(&self) -> usize {
        match self {
            Self::Select(value) => crate::MemorySize::memory_size(value),
            Self::TimeBuckets(value) => std::mem::size_of_val(value)
                .saturating_add(value.time.capacity())
                .saturating_add(value.value.capacity())
                .saturating_add(value.identity.capacity())
                .saturating_add(crate::MemorySize::memory_size(&value.group)),
        }
    }
}
