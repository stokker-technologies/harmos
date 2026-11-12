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
use super::columnar::{self, ArrowScan, Cancellation, ColumnSource};
use crate::{BackendSnapshot, Error, Identifier, ReadLimits, Result, StorageBackend, StorageCursor};

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

pub struct ProjectionPage {
    pub batch: RecordBatch,
    pub frontier: StorageCursor,
    pub revision: u64,
    pub changed_keys: Vec<i64>,
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
    changed_keys: Vec<i64>,
    cancel: Cancellation,
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
    changed_keys: Vec<i64>,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjectionTransform {
    Select(Vec<String>),
}

/* === Traits === */

/* === Implementations === */

impl ProjectionDefinition {
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 {
            return Err(columnar::invalid("projection version must be nonzero"));
        }
        let ProjectionTransform::Select(columns) = &self.transform;
        ArrowScan {
            source: self.source.clone(),
            columns: columns.clone(),
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
    pub fn changed_keys(&self) -> &[i64] {
        &self.changed_keys
    }
    pub fn check_cancelled(&self) -> Result<()> {
        self.cancel.check()
    }
}

impl ProjectionCatalog {
    pub fn register(&mut self, definition: &ProjectionDefinition, rebuild: bool) -> Result<u64> {
        definition.validate()?;
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
    backend: &dyn StorageBackend, snapshot: &BackendSnapshot, name: &Identifier, work: ReadLimits, cancel: &Cancellation,
) -> Result<u64> {
    cancel.check()?;
    let (definition, revision) = backend.projection_definition(name).await?;
    backend
        .read_schemas(
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
    if backend.projection_status(name, revision, &snapshot.at).await?.is_some() {
        return Ok(revision);
    }
    if backend.projection_frontier(name, revision).await?.is_some_and(|frontier| frontier.sequence > snapshot.at.sequence) {
        return Err(conflict("projection frontiers must advance monotonically"));
    }
    let ProjectionTransform::Select(columns) = &definition.transform;
    let mut scan = ArrowScan {
        source: definition.source.clone(),
        columns: columns.clone(),
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
        let page = columnar::scan_arrow(backend, snapshot, &scan, cancel).await?;
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
    let publication = ProjectionPublication {
        snapshot: snapshot.clone(),
        definition,
        revision,
        batches,
        changed_keys: vec![],
        cancel: cancel.clone(),
    };
    backend.publish_projection(publication).await?;
    Ok(revision)
}

/* === Private Functions === */

pub(crate) fn page(
    read: &ProjectionRead, batch: &RecordBatch, start: usize, keys: &[i64], total_rows: usize, total_keys: usize,
) -> Result<ProjectionPage> {
    if batch.num_rows() > read.work.rows.get() || batch.get_array_memory_size().saturating_mul(3) > read.work.bytes.get() {
        return Err(columnar::exhausted());
    }
    let offset = read.after.saturating_sub(start).min(batch.num_rows());
    let available = if read.after < total_rows {
        batch.num_rows() - offset
    } else {
        keys.len().min(read.work.rows.get()).min(total_keys.saturating_sub(read.after))
    };
    let mut low = 0;
    let mut high = available.min(read.limits.rows.get());
    while low < high {
        let count = low + (high - low).div_ceil(2);
        let indices = (offset..offset + count.min(batch.num_rows() - offset)).map(|row| row as u64).collect::<Vec<_>>();
        let candidate = columnar::take(batch, &indices)?;
        let bytes = candidate.get_array_memory_size().saturating_add(count.min(keys.len()).saturating_mul(8));
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
    let batch = columnar::take(batch, &indices)?;
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
    if let Some(through) = retired {
        if requested <= through {
            return Err(Error::Gap {
                requested,
                available: through.saturating_add(1),
            });
        }
    }
    Ok(())
}
fn conflict(reason: &'static str) -> Error {
    Error::backend("storage projection", crate::BackendFailure::Conflict, std::io::Error::new(std::io::ErrorKind::InvalidInput, reason))
}

/* === Tests === */
