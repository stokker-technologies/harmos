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
    state: &mut State, definition: &IndexDefinition, rebuild: bool, work: ReadLimits, cancel: &Cancellation,
) -> Result<u64> {
    cancel.check()?;
    state.check()?;
    definition.validate()?;
    let tx = rusqlite::Transaction::new_unchecked(&state.connection, rusqlite::TransactionBehavior::Immediate).map_err(db)?;
    let prior = tx
        .query_row("SELECT definition,revision FROM storage_indexes WHERE name=?1", [definition.name.as_str()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, unsigned(row, 1)?))
        })
        .optional()
        .map_err(db)?;
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
    tx.execute("INSERT INTO storage_indexes VALUES(?1,?2,?3,?4) ON CONFLICT(name) DO UPDATE SET revision=excluded.revision,source=excluded.source,definition=excluded.definition", params![definition.name.as_str(),integer(revision)?,text(&definition.source)?,encode(definition)?]).map_err(db)?;
    tx.execute("DELETE FROM storage_index_entries WHERE name=?1", [definition.name.as_str()]).map_err(db)?;
    let at = state.cursor()?;
    let token = [b"index-build:".as_slice(), definition.name.as_str().as_bytes()].concat();
    state.snapshots.insert(token.clone(), at.clone());
    let snapshot = BackendSnapshot { at, token: token.clone() };
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
        loop {
            cancel.check()?;
            if rows >= work.rows.get() || bytes >= work.bytes.get() {
                return Err(crate::query::columnar::exhausted());
            }
            read.work = ReadLimits {
                rows: NonZeroUsize::new(work.rows.get() - rows).unwrap(),
                bytes: NonZeroUsize::new(work.bytes.get() - bytes).unwrap(),
            };
            read.limits = read.work;
            let page = columns::scan(state, &snapshot, &read, cancel, true)?;
            rows += page.scanned;
            bytes = bytes.saturating_add(page.bytes_read);
            for row in 0..page.batch.num_rows() {
                if row % 256 == 0 {
                    cancel.check()?;
                }
                let entry = execution::entry(&page.batch, definition, row)?;
                if bytes.saturating_add(entry.key.capacity() + std::mem::size_of::<IndexEntry>()) > work.bytes.get() {
                    return Err(crate::query::columnar::exhausted());
                }
                tx.execute(
                    "INSERT INTO storage_index_entries VALUES(?1,?2,?3,?4)",
                    params![definition.name.as_str(), integer(revision)?, entry.key, integer(entry.position)?],
                )
                .map_err(db)?;
            }
            if page.complete {
                break;
            }
            read.after = page.next;
        }
        cancel.check()?;
        Ok(revision)
    })();
    state.snapshots.remove(&token);
    let revision = result?;
    tx.commit().map_err(db)?;
    Ok(revision)
}

pub(super) fn maintain(connection: &Connection, source: &ColumnSource, batch: &RecordBatch) -> Result<()> {
    let mut query = connection.prepare("SELECT definition,revision FROM storage_indexes WHERE source=?1 ORDER BY name").map_err(db)?;
    let mut definitions = query.query([text(source)?]).map_err(db)?;
    while let Some(row) = definitions.next().map_err(db)? {
        let definition: IndexDefinition = decode(&row.get::<_, Vec<u8>>(0).map_err(db)?)?;
        let revision = unsigned(row, 1).map_err(db)?;
        for row in 0..batch.num_rows() {
            let entry = execution::entry(batch, &definition, row)?;
            connection
                .execute(
                    "INSERT INTO storage_index_entries VALUES(?1,?2,?3,?4)",
                    params![definition.name.as_str(), integer(revision)?, entry.key, integer(entry.position)?],
                )
                .map_err(db)?;
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
    let comparison = if read.descending { "<" } else { ">" };
    let direction = if read.descending { "DESC" } else { "ASC" };
    let latest = if definition.source.kind == ColumnKind::Objects {
        "AND s.position=(SELECT max(o.position) FROM storage_rows o WHERE o.kind=1 AND o.schema_key=s.schema_key AND o.item=s.item AND o.position<=?3)"
    } else {
        ""
    };
    let sql = format!(
        "SELECT i.key,i.position FROM storage_index_entries i JOIN storage_rows s ON s.position=i.position WHERE i.name=?1 AND i.revision=?2 AND i.position<=?3 AND (?4 IS NULL OR i.key>=?4) AND (?5 IS NULL OR i.key<?5) AND i.key>=?6 AND (?7 IS NULL OR i.key<?7) AND (?8 IS NULL OR (i.key,i.position){comparison}(?8,?9)) {latest} ORDER BY i.key {direction},i.position {direction} LIMIT {}",
        read.limits.rows.get().saturating_add(1)
    );
    let prefix_end = prefix_end(&read.prefix);
    let seek_key = read.after.as_ref().map(|after| after.key.as_slice());
    let seek_position = read.after.as_ref().map(|after| integer(after.position)).transpose()?;
    let mut query = tx.prepare(&sql).map_err(db)?;
    let mut rows = query
        .query(params![
            read.name.as_str(),
            integer(revision)?,
            integer(snapshot.at.sequence)?,
            read.lower.as_deref(),
            read.upper.as_deref(),
            read.prefix,
            prefix_end,
            seek_key,
            seek_position
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
