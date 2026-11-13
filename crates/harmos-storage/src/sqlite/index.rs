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
use crate::query::columnar::{ArrowScan, Cancellation, ColumnKind, ColumnSource};
use crate::query::index::{self as execution, IndexDefinition, IndexEntry, IndexPage, IndexRead};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn definition(connection: &Connection, name: &crate::Identifier) -> Result<(IndexDefinition, u64)> {
    let value = connection
        .query_row("SELECT definition,revision FROM storage_indexes WHERE name=?1", [name.as_str()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, unsigned(row, 1)?))
        })
        .optional()
        .map_err(db)?;
    let Some((data, revision)) = value else {
        return Err(Error::IndexNotFound { name: name.clone() });
    };
    Ok((decode(&data)?, revision))
}

pub(super) fn register(
    state: &mut State, definition: &IndexDefinition, expected: Option<u64>, rebuild: bool, work: ReadLimits, cancel: &Cancellation,
    mutation: Option<&crate::MutationHeader>,
) -> Result<crate::DefinitionOutcome> {
    cancel.check()?;
    state.check()?;
    definition.validate()?;
    let mut tx = rusqlite::Transaction::new_unchecked(&state.connection, rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    if let Some(outcome) = super::definition_mutations::replay(&tx, mutation, crate::DefinitionKind::Index, &definition.name)? {
        return Ok(outcome);
    }
    let at = state.cursor()?;
    let token = [b"index-build:".as_slice(), definition.name.as_str().as_bytes()].concat();
    state.snapshots.insert(token.clone(), at.clone());
    let snapshot = BackendSnapshot { at, token: token.clone() };
    let result = (|| -> Result<u64> {
        let edit = tx.savepoint().map_err(db)?;
        let prior = edit
            .query_row("SELECT definition,revision FROM storage_indexes WHERE name=?1", [definition.name.as_str()], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, unsigned(row, 1)?))
            })
            .optional()
            .map_err(db)?;
        if prior.as_ref().map(|(_, revision)| *revision) != expected {
            return Err(conflict("set index definition: get again and retry"));
        }
        let mut revision = 1;
        if let Some((data, current)) = prior {
            if decode::<IndexDefinition>(&data)? == *definition {
                return Ok(current);
            }
            if !rebuild {
                return Err(conflict("changed index requires explicit rebuild"));
            }
            revision = current.checked_add(1).ok_or_else(|| invalid("index revision overflow"))?;
        }
        edit.execute("INSERT INTO storage_indexes VALUES(?1,?2,?3,?4) ON CONFLICT(name) DO UPDATE SET revision=excluded.revision,source=excluded.source,definition=excluded.definition",params![definition.name.as_str(),integer(revision)?,text(&definition.source)?,encode(definition)?]).map_err(db)?;
        edit.execute("DELETE FROM storage_index_entries WHERE name=?1", [definition.name.as_str()]).map_err(db)?;
        let result = (|| {
            let mut read = ArrowScan {
                source: definition.source.clone(),
                columns: definition.columns.clone(),
                predicate: None,
                time: None,
                start: 0,
                after: None,
                limits: work,
                work,
            };
            if !read.columns.iter().any(|name| name == "_storage.position") {
                read.columns.push("_storage.position".into());
            }
            let mut rows = 0usize;
            let mut bytes = 0usize;
            let mut key_bytes = 0usize;
            let mut entries = Vec::<IndexEntry>::new();
            loop {
                cancel.check()?;
                let retained = entries.capacity().saturating_mul(std::mem::size_of::<IndexEntry>()).saturating_add(key_bytes);
                if rows >= work.rows.get() || bytes.saturating_add(retained) >= work.bytes.get() {
                    return Err(crate::query::columnar::exhausted());
                }
                read.work = ReadLimits {
                    rows: NonZeroUsize::new(work.rows.get() - rows).unwrap(),
                    bytes: NonZeroUsize::new(work.bytes.get() - bytes - retained).unwrap(),
                };
                read.limits = read.work;
                let page = columns::scan(state, &snapshot, &read, cancel, true)?;
                rows += page.scanned;
                bytes = bytes.saturating_add(page.bytes_read);
                let capacity = entries.len().saturating_add(page.batch.num_rows());
                if bytes.saturating_add(key_bytes).saturating_add(capacity.saturating_mul(std::mem::size_of::<IndexEntry>()))
                    > work.bytes.get()
                {
                    return Err(crate::query::columnar::exhausted());
                }
                if capacity > entries.capacity() {
                    let available = work.bytes.get().saturating_sub(bytes).saturating_sub(key_bytes) / std::mem::size_of::<IndexEntry>();
                    let target = capacity.max(entries.capacity().saturating_mul(2)).min(available);
                    entries.try_reserve_exact(target - entries.len()).map_err(|_| crate::query::columnar::exhausted())?;
                }
                for row in 0..page.batch.num_rows() {
                    if row % 256 == 0 {
                        cancel.check()?;
                    }
                    let entry = execution::entry(&page.batch, definition, row)?;
                    key_bytes = key_bytes.saturating_add(entry.key.capacity());
                    if bytes.saturating_add(key_bytes).saturating_add(entries.capacity().saturating_mul(std::mem::size_of::<IndexEntry>()))
                        > work.bytes.get()
                    {
                        return Err(crate::query::columnar::exhausted());
                    }
                    entries.push(entry);
                }
                if page.complete {
                    break;
                }
                read.after = page.next;
            }
            Ok(entries)
        })();
        let mut entries = result?;
        cancel.check()?;
        entries.sort_unstable();
        cancel.check()?;
        let mut insert = edit.prepare("INSERT INTO storage_index_entries VALUES(?1,?2,?3,?4)").map_err(db)?;
        for (ordinal, entry) in entries.iter().enumerate() {
            if ordinal % 256 == 0 {
                cancel.check()?;
            }
            insert.execute(params![definition.name.as_str(), integer(revision)?, &entry.key, integer(entry.position)?]).map_err(db)?;
        }
        drop(insert);
        cancel.check()?;
        edit.commit().map_err(db)?;
        let mut counters = state.work.get();
        counters.index_prepares += 1;
        counters.index_entries += entries.len() as u64;
        state.work.set(counters);
        Ok(revision)
    })();
    state.snapshots.remove(&token);
    let outcome = crate::DefinitionOutcome::capture(result)?;
    super::definition_mutations::write(&tx, mutation, crate::DefinitionKind::Index, &definition.name, &outcome)?;
    tx.commit().map_err(db)?;
    Ok(outcome)
}

pub(super) fn maintain(connection: &Connection, source: &ColumnSource, batch: &RecordBatch) -> Result<()> {
    let mut query = connection.prepare("SELECT definition,revision FROM storage_indexes WHERE source=?1 ORDER BY name").map_err(db)?;
    let mut definitions = query.query([text(source)?]).map_err(db)?;
    while let Some(row) = definitions.next().map_err(db)? {
        let definition: IndexDefinition = decode(&row.get::<_, Vec<u8>>(0).map_err(db)?)?;
        let revision = unsigned(row, 1).map_err(db)?;
        let mut insert = connection.prepare("INSERT INTO storage_index_entries VALUES(?1,?2,?3,?4)").map_err(db)?;
        for row in 0..batch.num_rows() {
            let entry = execution::entry(batch, &definition, row)?;
            insert.execute(params![definition.name.as_str(), integer(revision)?, entry.key, integer(entry.position)?]).map_err(db)?;
        }
    }
    Ok(())
}

pub(super) fn scan(state: &State, snapshot: &BackendSnapshot, read: &IndexRead, cancel: &Cancellation) -> Result<IndexPage> {
    cancel.check()?;
    state.snapshot(snapshot)?;
    let tx = state.connection.unchecked_transaction().map_err(db)?;
    let (definition, revision) = definition(&tx, &read.name)?;
    read.validate(snapshot, &definition, revision)?;
    let direction = if read.descending { "DESC" } else { "ASC" };
    let base = read.lower.as_deref().unwrap_or_default().max(read.prefix.as_slice());
    let mut lower = (base, 0u64);
    let mut exclusive = false;
    if !read.descending
        && let Some(after) = &read.after
        && (after.key.as_slice(), after.position) >= lower
    {
        lower = (after.key.as_slice(), after.position);
        exclusive = true;
    }
    let end = prefix_end(&read.prefix);
    let mut upper = read.upper.as_deref().into_iter().chain(end.as_deref()).map(|key| (key, 0u64)).min();
    if read.descending
        && let Some(after) = &read.after
    {
        let seek = (after.key.as_slice(), after.position);
        upper = Some(upper.map_or(seek, |bound| bound.min(seek)));
    }
    let lower_comparison = if exclusive { ">" } else { ">=" };
    let upper_bound = if upper.is_some() { "AND (i.key,i.position)<(?6,?7)" } else { "AND ?6 IS NULL AND ?7 IS NULL" };
    let join = if definition.source.kind == ColumnKind::Objects { "JOIN storage_rows s ON s.position=i.position" } else { "" };
    let latest = if definition.source.kind == ColumnKind::Objects {
        "AND s.position=(SELECT max(o.position) FROM storage_rows o WHERE o.kind=1 AND o.schema_key=s.schema_key AND o.item=s.item AND o.position<=?3)"
    } else {
        ""
    };
    let sql = format!(
        "SELECT i.key,i.position FROM storage_index_entries i {join} WHERE i.name=?1 AND i.revision=?2 AND i.position<=?3 AND (i.key,i.position){lower_comparison}(?4,?5) {upper_bound} {latest} ORDER BY i.key {direction},i.position {direction} LIMIT {}",
        read.limits.rows.get().saturating_add(1)
    );
    let mut query = tx.prepare(&sql).map_err(db)?;
    let mut rows = query
        .query(params![
            read.name.as_str(),
            integer(revision)?,
            integer(snapshot.at.sequence)?,
            lower.0,
            integer(lower.1)?,
            upper.map(|bound| bound.0),
            upper.map(|bound| integer(bound.1)).transpose()?,
        ])
        .map_err(db)?;
    let mut candidates = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next().map_err(db)? {
        cancel.check()?;
        let key = row.get_ref(0).map_err(db)?.as_blob().map_err(|_| incompatible())?;
        bytes = bytes.saturating_add(key.len() + std::mem::size_of::<IndexEntry>());
        if candidates.len() == read.work.rows.get() || bytes > read.work.bytes.get() {
            return Err(crate::query::columnar::exhausted());
        }
        candidates.push(IndexEntry {
            key: key.to_vec(),
            position: unsigned(row, 1).map_err(db)?,
        });
    }
    drop(rows);
    let mut counters = state.work.get();
    counters.index_entries += candidates.len() as u64;
    counters.index_steps += query.get_status(rusqlite::StatementStatus::VmStep) as u64;
    counters.index_prepares += 1;
    state.work.set(counters);
    execution::finish(snapshot, read, &definition, revision, candidates)
}

/* === Private Functions === */

fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.pop() {
        if last != u8::MAX {
            end.push(last + 1);
            return Some(end);
        }
    }
    None
}

/* === Tests === */
