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

// Third Party Imports
use arrow_array::RecordBatch;
use rusqlite::{Connection, OptionalExtension, params};

// Local Imports
use super::*;
use crate::query::projection::{
    self as execution, ProjectionDefinition, ProjectionPage, ProjectionPublication, ProjectionRead, ProjectionStatus,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn definition(connection: &Connection, name: &crate::Identifier) -> Result<(ProjectionDefinition, u64)> {
    let value=connection.query_row("SELECT d.definition,a.revision FROM projection_active a JOIN projection_definitions d ON d.name=a.name AND d.revision=a.revision WHERE a.name=?1",[name.as_str()],|row|Ok((row.get::<_,Vec<u8>>(0)?,unsigned(row,1)?))).optional().map_err(db)?;
    let (data, revision) = value.ok_or(Error::NotReady { domain: "projection definition" })?;
    Ok((decode(&data)?, revision))
}

pub(super) fn definition_at(connection: &Connection, name: &crate::Identifier, revision: u64) -> Result<ProjectionDefinition> {
    let data = connection
        .query_row(
            "SELECT definition FROM projection_definitions WHERE name=?1 AND revision=?2",
            params![name.as_str(), integer(revision)?],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(db)?;
    decode(&data.ok_or(Error::NotReady { domain: "projection revision" })?)
}

pub(super) fn register(state: &mut State, value: &ProjectionDefinition, rebuild: bool) -> Result<u64> {
    state.check()?;
    value.validate()?;
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let prior=tx.query_row("SELECT d.definition,a.revision FROM projection_active a JOIN projection_definitions d ON d.name=a.name AND d.revision=a.revision WHERE a.name=?1",[value.name.as_str()],|row|Ok((row.get::<_,Vec<u8>>(0)?,unsigned(row,1)?))).optional().map_err(db)?;
    let mut revision = 1;
    if let Some((data, current)) = prior {
        if decode::<ProjectionDefinition>(&data)? == *value {
            return Ok(current);
        }
        if !rebuild {
            return Err(conflict("changed projection requires explicit rebuild"));
        }
        revision = current.checked_add(1).ok_or_else(|| invalid("projection revision overflow"))?;
    }
    tx.execute(
        "INSERT INTO projection_definitions VALUES(?1,?2,?3,NULL)",
        params![value.name.as_str(), integer(revision)?, encode(value)?],
    )
    .map_err(db)?;
    tx.execute(
        "INSERT INTO projection_active VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET revision=excluded.revision",
        params![value.name.as_str(), integer(revision)?],
    )
    .map_err(db)?;
    tx.commit().map_err(db)?;
    Ok(revision)
}

pub(super) fn status(state: &State, name: &crate::Identifier, revision: u64, cut: &StorageCursor) -> Result<Option<ProjectionStatus>> {
    state.check()?;
    state.history.check(&cut.history)?;
    check_retired(&state.connection, name, revision, cut.sequence)?;
    let result = state
        .connection
        .query_row(
            "SELECT rows,changes FROM projection_cuts WHERE name=?1 AND revision=?2 AND frontier=?3",
            params![name.as_str(), integer(revision)?, integer(cut.sequence)?],
            |row| Ok((size(row, 0)?, size(row, 1)?)),
        )
        .optional()
        .map_err(db)?;
    Ok(result.map(|(rows, changed_keys)| ProjectionStatus {
        frontier: cut.clone(),
        revision,
        rows,
        changed_keys,
    }))
}

pub(super) fn frontier(state: &State, name: &crate::Identifier, revision: u64) -> Result<Option<StorageCursor>> {
    state.check()?;
    let exists: bool = state
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projection_definitions WHERE name=?1 AND revision=?2)",
            params![name.as_str(), integer(revision)?],
            |row| row.get(0),
        )
        .map_err(db)?;
    if !exists {
        return Err(Error::NotReady { domain: "projection revision" });
    }
    let value: Option<i64> = state
        .connection
        .query_row(
            "SELECT max(frontier) FROM projection_cuts WHERE name=?1 AND revision=?2",
            params![name.as_str(), integer(revision)?],
            |row| row.get(0),
        )
        .map_err(db)?;
    Ok(value.map(|sequence| StorageCursor {
        history: state.history.clone(),
        sequence: sequence as u64,
    }))
}

pub(super) fn publish(state: &mut State, publication: &ProjectionPublication) -> Result<()> {
    publication.check_cancelled()?;
    state.snapshot(publication.snapshot())?;
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let (definition, revision) = definition(&tx, &publication.definition().name)?;
    if revision != publication.revision() || definition != *publication.definition() {
        return Err(conflict("projection definition changed during advancement"));
    }
    let name = definition.name.as_str();
    let cut = publication.snapshot().at.sequence;
    check_retired(&tx, &definition.name, revision, cut)?;
    let existing: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projection_cuts WHERE name=?1 AND revision=?2 AND frontier=?3)",
            params![name, integer(revision)?, integer(cut)?],
            |row| row.get(0),
        )
        .map_err(db)?;
    if existing {
        return Ok(());
    }
    let latest: Option<i64> = tx
        .query_row("SELECT max(frontier) FROM projection_cuts WHERE name=?1 AND revision=?2", params![name, integer(revision)?], |row| {
            row.get(0)
        })
        .map_err(db)?;
    if latest.is_some_and(|latest| latest > cut as i64) {
        return Err(conflict("projection frontiers must advance monotonically"));
    }
    if latest.map(|value| value as u64) != publication.expected_frontier().map(|frontier| frontier.sequence) {
        return Err(conflict("projection frontier changed during advancement"));
    }
    let first = publication.batches().first().ok_or_else(|| invalid("projection publication requires a schema"))?;
    let schema = columns::ipc(&RecordBatch::new_empty(first.schema()))?;
    if schema.len() > state.settings.chunk_bytes.get() {
        return Err(crate::query::columnar::exhausted());
    }
    let mut rows = 0usize;
    for batch in publication.batches() {
        if batch.schema() != first.schema() {
            return Err(invalid("projection output schemas differ"));
        }
        for offset in (0..batch.num_rows()).step_by(state.settings.chunk_rows.get()) {
            publication.check_cancelled()?;
            let count = (batch.num_rows() - offset).min(state.settings.chunk_rows.get());
            let data = columns::ipc(&batch.slice(offset, count))?;
            if data.len() > state.settings.chunk_bytes.get() {
                return Err(crate::query::columnar::exhausted());
            }
            tx.execute(
                "INSERT INTO projection_pages VALUES(?1,?2,?3,?4,?5,?6)",
                params![name, integer(revision)?, integer(cut)?, integer(rows as u64)?, integer(count as u64)?, data],
            )
            .map_err(db)?;
            rows = rows.checked_add(count).ok_or_else(|| invalid("projection row count overflow"))?;
        }
    }
    if publication.changed_keys().windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid("projection changed keys must increase"));
    }
    for (ordinal, key) in publication.changed_keys().iter().enumerate() {
        publication.check_cancelled()?;
        tx.execute(
            "INSERT INTO projection_changes VALUES(?1,?2,?3,?4,?5)",
            params![name, integer(revision)?, integer(cut)?, integer(ordinal as u64)?, key],
        )
        .map_err(db)?;
    }
    tx.execute(
        "INSERT INTO projection_cuts VALUES(?1,?2,?3,?4,?5,?6)",
        params![name, integer(revision)?, integer(cut)?, integer(rows as u64)?, integer(publication.changed_keys().len() as u64)?, schema],
    )
    .map_err(db)?;
    publication.check_cancelled()?;
    tx.commit().map_err(db)
}

pub(super) fn read(state: &State, read: &ProjectionRead) -> Result<ProjectionPage> {
    state.check()?;
    state.history.check(&read.frontier.history)?;
    check_retired(&state.connection, &read.name, read.revision, read.frontier.sequence)?;
    let metadata = state
        .connection
        .query_row(
            "SELECT rows,changes,length(schema) FROM projection_cuts WHERE name=?1 AND revision=?2 AND frontier=?3",
            params![read.name.as_str(), integer(read.revision)?, integer(read.frontier.sequence)?],
            |row| Ok((size(row, 0)?, size(row, 1)?, size(row, 2)?)),
        )
        .optional()
        .map_err(db)?;
    let (total_rows, total_keys, schema_bytes) = metadata.ok_or(Error::NotReady { domain: "projection frontier" })?;
    let page=state.connection.query_row("SELECT first_row,rows,length(data) FROM projection_pages WHERE name=?1 AND revision=?2 AND frontier=?3 AND first_row<=?4 AND first_row+rows>?4 ORDER BY first_row DESC LIMIT 1",params![read.name.as_str(),integer(read.revision)?,integer(read.frontier.sequence)?,integer(read.after as u64)?],|row|Ok((size(row,0)?,size(row,1)?,size(row,2)?))).optional().map_err(db)?;
    let (batch, start) = if let Some((start, rows, bytes)) = page {
        if rows > read.work.rows.get() || bytes.saturating_mul(4) > read.work.bytes.get() {
            return Err(crate::query::columnar::exhausted());
        }
        let data: Vec<u8> = state
            .connection
            .query_row(
                "SELECT data FROM projection_pages WHERE name=?1 AND revision=?2 AND frontier=?3 AND first_row=?4",
                params![read.name.as_str(), integer(read.revision)?, integer(read.frontier.sequence)?, integer(start as u64)?],
                |row| row.get(0),
            )
            .map_err(db)?;
        (columns::from_ipc(&data)?, start)
    } else {
        if read.after < total_rows {
            return Err(incompatible());
        }
        if schema_bytes.saturating_mul(4) > read.work.bytes.get() {
            return Err(crate::query::columnar::exhausted());
        }
        let data: Vec<u8> = state
            .connection
            .query_row(
                "SELECT schema FROM projection_cuts WHERE name=?1 AND revision=?2 AND frontier=?3",
                params![read.name.as_str(), integer(read.revision)?, integer(read.frontier.sequence)?],
                |row| row.get(0),
            )
            .map_err(db)?;
        (columns::from_ipc(&data)?, read.after)
    };
    let key_limit = read.limits.rows.get().min(read.work.rows.get());
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
            integer(key_limit as u64)?
        ])
        .map_err(db)?;
    let mut keys = Vec::new();
    let mut used = 0usize;
    while let Some(row) = rows.next().map_err(db)? {
        let key = row.get_ref(0).map_err(db)?.as_blob().map_err(|_| incompatible())?;
        let bytes = key.len() + std::mem::size_of::<Vec<u8>>();
        if used.saturating_add(bytes).saturating_add(batch.get_array_memory_size().saturating_mul(3)) > read.work.bytes.get() {
            if keys.is_empty() {
                return Err(crate::query::columnar::exhausted());
            }
            break;
        }
        used += bytes;
        keys.push(key.to_vec());
    }
    execution::page(read, &batch, start, &keys, total_rows, total_keys)
}

pub(super) fn retire(state: &mut State, name: &crate::Identifier, revision: u64, through: &StorageCursor) -> Result<()> {
    state.check()?;
    state.history.check(&through.history)?;
    if through.sequence > state.cursor()?.sequence {
        return Err(invalid("projection retirement is ahead of durable data"));
    }
    let tx = state.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let changed = tx
        .execute(
            "UPDATE projection_definitions SET retired=max(coalesce(retired,0),?1) WHERE name=?2 AND revision=?3",
            params![integer(through.sequence)?, name.as_str(), integer(revision)?],
        )
        .map_err(db)?;
    if changed == 0 {
        return Err(Error::NotReady { domain: "projection revision" });
    }
    tx.execute(
        "DELETE FROM projection_pages WHERE name=?1 AND revision=?2 AND frontier<=?3",
        params![name.as_str(), integer(revision)?, integer(through.sequence)?],
    )
    .map_err(db)?;
    tx.execute(
        "DELETE FROM projection_changes WHERE name=?1 AND revision=?2 AND frontier<=?3",
        params![name.as_str(), integer(revision)?, integer(through.sequence)?],
    )
    .map_err(db)?;
    tx.execute(
        "DELETE FROM projection_cuts WHERE name=?1 AND revision=?2 AND frontier<=?3",
        params![name.as_str(), integer(revision)?, integer(through.sequence)?],
    )
    .map_err(db)?;
    tx.commit().map_err(db)
}

/* === Private Functions === */

fn check_retired(connection: &Connection, name: &crate::Identifier, revision: u64, cut: u64) -> Result<()> {
    let retired: Option<Option<i64>> = connection
        .query_row(
            "SELECT retired FROM projection_definitions WHERE name=?1 AND revision=?2",
            params![name.as_str(), integer(revision)?],
            |row| row.get(0),
        )
        .optional()
        .map_err(db)?;
    let retired = retired.ok_or(Error::NotReady { domain: "projection revision" })?.map(|value| value as u64);
    execution::check_retired(retired, cut)
}

/* === Tests === */
