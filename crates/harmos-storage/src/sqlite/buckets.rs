/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Advance Durable Time Buckets

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

// Third Party Imports
use arrow_array::RecordBatch;
use rusqlite::{OptionalExtension, params};

// Local Imports
use super::*;
use harmos_query::columnar::ArrowScan;
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use crate::query::projection::ProjectionPage;
use crate::query::projection::ProjectionRead;
use crate::query::projection::ProjectionTransform;
use harmos_query::time_buckets::self;
use harmos_query::time_buckets::Accumulator;
use harmos_query::time_buckets::Bucket;
use harmos_query::time_buckets::Sample;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn advance(
    state: &mut State, snapshot: &BackendSnapshot, name: &crate::Identifier, expected: Option<u64>, work: ReadLimits, cancel: &Cancellation,
) -> Result<Option<u64>> {
    cancel.check()?;
    state.snapshot(snapshot)?;
    let (definition, revision) = projection::definition(&state.connection, name)?;
    if expected.is_some_and(|expected| expected != revision) {
        return Err(conflict("projection rebuilt before advancement"));
    }
    let ProjectionTransform::TimeBuckets(plan) = &definition.transform else { return Ok(None) };
    // Object Snapshots Resolve Superseded Items Before Identity Deduplication.
    if definition.source.kind == ColumnKind::Objects {
        return Ok(None);
    }
    let schema = rows::schema(&state.connection, &definition.source.schema, snapshot.at.sequence)?;
    if projection::status(state, name, revision, &snapshot.at)?.is_some() {
        return Ok(Some(revision));
    }
    let previous = projection::frontier(state, name, revision)?;
    if previous.as_ref().is_some_and(|frontier| frontier.sequence > snapshot.at.sequence) {
        return Err(conflict("projection frontiers must advance monotonically"));
    }
    let tx = state.connection.unchecked_transaction().map_err(db)?;
    let prior: Option<(u64, usize, usize)> = tx
        .query_row(
            "SELECT frontier,source_rows,output_rows FROM projection_bucket_state WHERE name=?1 AND revision=?2",
            params![name.as_str(), integer(revision)?],
            |row| Ok((unsigned(row, 0)?, size(row, 1)?, size(row, 2)?)),
        )
        .optional()
        .map_err(db)?;
    let (start, mut source_rows, _) = prior.unwrap_or((0, 0, 0));
    if start > snapshot.at.sequence {
        return Err(conflict("bucket state is ahead of requested cut"));
    }
    let mut columns = plan.group.clone();
    columns.extend([plan.identity.clone(), plan.time.clone(), plan.value.clone(), "_storage.position".into()]);
    columns.sort();
    columns.dedup();
    let mut read = ArrowScan {
        source: definition.source.clone(),
        columns,
        predicate: None,
        time: None,
        start,
        after: None,
        limits: work,
        work,
    };
    let mut legacy = BTreeMap::new();
    let mut legacy_key_bytes = 0usize;
    if prior.is_none()
        && let Some(frontier) = &previous
    {
        let mut read = ProjectionRead {
            name: name.clone(),
            revision,
            frontier: frontier.clone(),
            after: 0,
            limits: work,
            work,
        };
        loop {
            cancel.check()?;
            let page = projection::read(state, &read)?;
            for (key, value) in time_buckets::fingerprints(&page.batch, work.bytes.get() / 4)? {
                legacy_key_bytes = legacy_key_bytes.saturating_add(key.capacity());
                legacy.insert(key, value);
            }
            if crate::btree_map_memory::<Vec<u8>, [u8; 32]>(legacy.len()).saturating_add(legacy_key_bytes) > work.bytes.get() / 4 {
                return Err(crate::query::columnar::exhausted());
            }
            let Some(next) = page.next else {
                break;
            };
            read.after = next;
        }
    }
    let mut affected = BTreeSet::new();
    let mut examined = 0usize;
    let mut bytes = 0usize;
    let mut owned = crate::btree_map_memory::<Vec<u8>, [u8; 32]>(legacy.len()).saturating_add(legacy_key_bytes);
    let mut lookup =
        tx.prepare("SELECT bucket_key FROM projection_bucket_members WHERE name=?1 AND revision=?2 AND identity=?3").map_err(db)?;
    let mut insert = tx.prepare("INSERT INTO projection_bucket_members VALUES(?1,?2,?3,?4,?5) ON CONFLICT(name,revision,identity) DO UPDATE SET bucket_key=excluded.bucket_key,data=excluded.data").map_err(db)?;
    loop {
        cancel.check()?;
        work.check(examined.saturating_add(1), bytes.saturating_add(owned)).map_err(|_| crate::query::columnar::exhausted())?;
        read.work = ReadLimits {
            rows: NonZeroUsize::new(work.rows.get() - examined).ok_or_else(crate::query::columnar::exhausted)?,
            bytes: NonZeroUsize::new(work.bytes.get() - bytes - owned).ok_or_else(crate::query::columnar::exhausted)?,
        };
        read.limits = read.work;
        let page = columns::scan(state, snapshot, &read, cancel, true)?;
        examined += page.scanned;
        source_rows += page.batch.num_rows();
        bytes = bytes.saturating_add(page.bytes_read);
        if source_rows > plan.max_samples.get() {
            return Err(crate::query::columnar::exhausted());
        }
        let mut accumulator = Accumulator::new(plan, page.batch.schema())?;
        accumulator.push(&page.batch, &mut || cancel.check())?;
        if bytes.saturating_add(owned).saturating_add(page.batch.get_array_memory_size()).saturating_add(accumulator.memory_bytes())
            > work.bytes.get()
        {
            return Err(crate::query::columnar::exhausted());
        }
        for sample in accumulator.into_samples() {
            cancel.check()?;
            let old: Option<Vec<u8>> = if prior.is_some() {
                lookup.query_row(params![name.as_str(), integer(revision)?, &sample.identity], |row| row.get(0)).optional().map_err(db)?
            } else {
                None
            };
            for key in old.into_iter().chain(std::iter::once(sample.key.clone())) {
                if !affected.contains(&key) {
                    owned = owned.saturating_add(key.capacity() + 64);
                    work.check(examined, bytes.saturating_add(owned)).map_err(|_| crate::query::columnar::exhausted())?;
                    affected.insert(key);
                }
            }
            let data = postcard::to_allocvec(&sample).map_err(|_| invalid("bucket member encoding failed"))?;
            insert.execute(params![name.as_str(), integer(revision)?, sample.identity, sample.key, data]).map_err(db)?;
        }
        if page.complete {
            break;
        }
        read.after = page.next;
    }
    drop(insert);
    drop(lookup);
    let input = crate::query::harmos_query::columnar::empty(&schema, definition.source.kind)?.schema();
    let mut total = previous
        .as_ref()
        .map(|frontier| projection::status(state, name, revision, frontier))
        .transpose()?
        .flatten()
        .map_or(0, |status| status.rows);
    if let Some((_, _, output_rows)) = prior {
        total = output_rows;
    }
    let mut changed = 0usize;
    let mut bucket_rows = 0usize;
    let mut bucket_steps = 0u64;
    let mut members =
        tx.prepare("SELECT data FROM projection_bucket_members WHERE name=?1 AND revision=?2 AND bucket_key=?3").map_err(db)?;
    let mut prior_output = tx
        .prepare("SELECT data FROM projection_bucket_outputs WHERE name=?1 AND revision=?2 AND key=?3 ORDER BY frontier DESC LIMIT 1")
        .map_err(db)?;
    let mut store = tx.prepare("INSERT INTO projection_bucket_outputs VALUES(?1,?2,?3,?4,?5)").map_err(db)?;
    let mut change = tx.prepare("INSERT INTO projection_changes VALUES(?1,?2,?3,?4,?5)").map_err(db)?;
    for key in &affected {
        cancel.check()?;
        let mut query = members.query(params![name.as_str(), integer(revision)?, key]).map_err(db)?;
        let mut values = Vec::new();
        let mut retained = 0usize;
        while let Some(row) = query.next().map_err(db)? {
            let data = row.get_ref(0).map_err(db)?.as_blob().map_err(|_| incompatible())?;
            let sample: Sample = postcard::from_bytes(data).map_err(|_| incompatible())?;
            retained = retained.saturating_add(sample.memory_bytes());
            bucket_rows += 1;
            work.check(bucket_rows, bytes.saturating_add(owned).saturating_add(retained.saturating_mul(3)))
                .map_err(|_| crate::query::columnar::exhausted())?;
            values.push(sample);
        }
        drop(query);
        bucket_steps += members.reset_status(rusqlite::StatementStatus::VmStep) as u64;
        let accumulator = Accumulator::from_samples(plan, input.clone(), values)?;
        let mut buckets = accumulator.finish_buckets(&mut || cancel.check())?;
        let bucket = buckets.pop();
        let old: Option<Option<Vec<u8>>> = if prior.is_some() {
            prior_output.query_row(params![name.as_str(), integer(revision)?, key], |row| row.get(0)).optional().map_err(db)?
        } else {
            None
        };
        let old = old.flatten().map(|data| postcard::from_bytes::<Bucket>(&data).map_err(|_| incompatible())).transpose()?;
        let old_revision = if prior.is_some() { old.as_ref().map(|bucket| bucket.revision) } else { legacy.get(key).copied() };
        let changed_key = old_revision != bucket.as_ref().map(|bucket| bucket.revision);
        if prior.is_some() && !changed_key {
            continue;
        }
        total = total + usize::from(bucket.is_some()) - usize::from(old_revision.is_some());
        let data = bucket.as_ref().map(postcard::to_allocvec).transpose().map_err(|_| invalid("bucket encoding failed"))?;
        if data.as_ref().is_some_and(|data| data.len() > state.settings.chunk_bytes.get()) {
            return Err(crate::query::columnar::exhausted());
        }
        store.execute(params![name.as_str(), integer(revision)?, key, integer(snapshot.at.sequence)?, data]).map_err(db)?;
        if changed_key {
            change
                .execute(params![name.as_str(), integer(revision)?, integer(snapshot.at.sequence)?, integer(changed as u64)?, key])
                .map_err(db)?;
            changed += 1;
        }
    }
    drop(members);
    drop(prior_output);
    drop(store);
    drop(change);
    let schema = columns::ipc(&RecordBatch::new_empty(Arc::new(time_buckets::output_schema(plan, &input)?)))?;
    if schema.len() > state.settings.chunk_bytes.get() {
        return Err(crate::query::columnar::exhausted());
    }
    tx.execute(
        "INSERT INTO projection_cuts VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            name.as_str(),
            integer(revision)?,
            integer(snapshot.at.sequence)?,
            integer(total as u64)?,
            integer(changed as u64)?,
            schema
        ],
    )
    .map_err(db)?;
    tx.execute(
        "INSERT INTO projection_bucket_cuts VALUES(?1,?2,?3)",
        params![name.as_str(), integer(revision)?, integer(snapshot.at.sequence)?],
    )
    .map_err(db)?;
    tx.execute("INSERT INTO projection_bucket_state VALUES(?1,?2,?3,?4,?5) ON CONFLICT(name,revision) DO UPDATE SET frontier=excluded.frontier,source_rows=excluded.source_rows,output_rows=excluded.output_rows",params![name.as_str(),integer(revision)?,integer(snapshot.at.sequence)?,integer(source_rows as u64)?,integer(total as u64)?]).map_err(db)?;
    cancel.check()?;
    tx.commit().map_err(db)?;
    let mut counters = state.work.get();
    counters.bucket_rows += bucket_rows as u64;
    counters.bucket_steps += bucket_steps;
    counters.buckets += affected.len() as u64;
    state.work.set(counters);
    Ok(Some(revision))
}

pub(super) fn read(state: &State, read: &ProjectionRead, total_rows: usize, total_keys: usize) -> Result<ProjectionPage> {
    let mut query = state.connection.prepare("SELECT b.data FROM projection_bucket_outputs b WHERE b.name=?1 AND b.revision=?2 AND b.frontier<=?3 AND b.frontier=(SELECT max(v.frontier) FROM projection_bucket_outputs v WHERE v.name=b.name AND v.revision=b.revision AND v.key=b.key AND v.frontier<=?3) AND b.data IS NOT NULL ORDER BY b.key LIMIT ?4 OFFSET ?5").map_err(db)?;
    let count = read.limits.rows.get().min(read.work.rows.get());
    let mut rows = query
        .query(params![
            read.name.as_str(),
            integer(read.revision)?,
            integer(read.frontier.sequence)?,
            integer(count as u64)?,
            integer(read.after as u64)?
        ])
        .map_err(db)?;
    let mut buckets = Vec::new();
    let mut used = 0usize;
    while let Some(row) = rows.next().map_err(db)? {
        let data = row.get_ref(0).map_err(db)?.as_blob().map_err(|_| incompatible())?;
        used = used.saturating_add(data.len().saturating_mul(32));
        if used > read.work.bytes.get() {
            return Err(crate::query::columnar::exhausted());
        }
        buckets.push(postcard::from_bytes::<Bucket>(data).map_err(|_| incompatible())?);
    }
    let output = time_buckets::output(&buckets)?;
    let mut query = state
        .connection
        .prepare(
            "SELECT key FROM projection_changes WHERE name=?1 AND revision=?2 AND frontier=?3 AND ordinal>=?4 ORDER BY ordinal LIMIT ?5",
        )
        .map_err(db)?;
    let mut rows = query
        .query(params![
            read.name.as_str(),
            integer(read.revision)?,
            integer(read.frontier.sequence)?,
            integer(read.after as u64)?,
            integer(count as u64)?
        ])
        .map_err(db)?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next().map_err(db)? {
        let key: Vec<u8> = row.get(0).map_err(db)?;
        used = used.saturating_add(key.capacity() + std::mem::size_of::<Vec<u8>>());
        if used > read.work.bytes.get() {
            return Err(crate::query::columnar::exhausted());
        }
        keys.push(key);
    }
    crate::query::projection::page(read, &output, read.after, &keys, total_rows, total_keys)
}

/* === Private Functions === */

/* === Tests === */
