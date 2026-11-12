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
use std::{collections::BTreeSet, io::Cursor, sync::Arc};

// Third Party Imports
use arrow_array::{Array, Float64Array, Int64Array, RecordBatch, TimestampNanosecondArray, UInt64Array};
use arrow_ipc::{reader::StreamReader, writer::StreamWriter};
use arrow_schema::Schema;
use rusqlite::{Connection, OptionalExtension, params};

// Local Imports
use super::*;
use crate::query::columnar::{self, ColumnKind, ColumnSource};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn store(connection: &Connection, source: &ColumnSource, batch: &RecordBatch, settings: SqliteSettings) -> Result<()> {
    if batch.num_rows() == 0 {
        return Ok(());
    }
    if batch.num_rows() > settings.chunk_rows.get() || batch.get_array_memory_size() > settings.chunk_bytes.get() {
        return Err(columnar::exhausted());
    }
    let positions = positions(batch)?;
    if positions.values().windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid("native chunk positions must increase"));
    }
    connection
        .execute(
            "INSERT INTO arrow_chunks(source,first,last,rows) VALUES(?1,?2,?3,?4)",
            params![text(source)?, integer(positions.value(0))?, integer(positions.value(batch.num_rows() - 1))?, batch.num_rows() as i64],
        )
        .map_err(db)?;
    let chunk = connection.last_insert_rowid();
    let mut bytes = 0usize;
    for (index, field) in batch.schema().fields().iter().enumerate() {
        let column = batch.column(index);
        let single = RecordBatch::try_new(Arc::new(Schema::new(vec![field.clone()])), vec![column.clone()])
            .map_err(|_| invalid("invalid native column"))?;
        let data = ipc(&single)?;
        bytes = bytes.saturating_add(data.len());
        if bytes > settings.chunk_bytes.get() {
            return Err(columnar::exhausted());
        }
        let range = column
            .as_any()
            .downcast_ref::<Float64Array>()
            .map(|column| {
                let finite = column.iter().flatten().filter(|value| value.is_finite()).collect::<Vec<_>>();
                (finite.iter().copied().reduce(f64::min), finite.iter().copied().reduce(f64::max))
            })
            .unwrap_or((None, None));
        let time = if let Some(array) = column.as_any().downcast_ref::<Int64Array>() {
            (array.iter().flatten().min(), array.iter().flatten().max())
        } else if let Some(array) = column.as_any().downcast_ref::<TimestampNanosecondArray>() {
            (array.iter().flatten().min(), array.iter().flatten().max())
        } else {
            (None, None)
        };
        connection
            .execute(
                "INSERT INTO arrow_columns VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![chunk, field.name(), data, range.0, range.1, time.0, time.1],
            )
            .map_err(db)?;
    }
    Ok(())
}

pub(super) fn scan(state: &State, snapshot: &BackendSnapshot, read: &ArrowScan, cancel: &Cancellation) -> Result<ArrowPage> {
    cancel.check()?;
    state.snapshot(snapshot)?;
    let after = read.position(snapshot)?;
    let schema = rows::schema(&state.connection, &read.source.schema, snapshot.at.sequence)?;
    let empty = columnar::empty(&schema, read.source.kind)?;
    let required = read.required_columns();
    for name in &required {
        if empty.column_by_name(name).is_none() {
            return Err(invalid("requested column is unavailable"));
        }
    }
    let source = text(&read.source)?;
    let chunk: Option<(i64, u64, u64, usize)> = state
        .connection
        .query_row(
            "SELECT id,first,last,rows FROM arrow_chunks WHERE source=?1 AND last>?2 AND first<=?3 ORDER BY first LIMIT 1",
            params![source, integer(after)?, integer(snapshot.at.sequence)?],
            |row| Ok((row.get(0)?, unsigned(row, 1)?, unsigned(row, 2)?, size(row, 3)?)),
        )
        .optional()
        .map_err(db)?;
    let Some((chunk, _first, last, count)) = chunk else {
        return columnar::finish_page(snapshot, read, &empty, true, 0, Vec::new(), 0, cancel);
    };
    let complete = last >= snapshot.at.sequence
        || !state
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM arrow_chunks WHERE source=?1 AND last>?2 AND first<=?3)",
                params![source, integer(last)?, integer(snapshot.at.sequence)?],
                |row| row.get::<_, bool>(0),
            )
            .map_err(db)?;
    let mut pruned = false;
    if let Some(predicate) = &read.predicate {
        let (minimum, maximum): (Option<f64>, Option<f64>) = state
            .connection
            .query_row("SELECT minimum,maximum FROM arrow_columns WHERE chunk=?1 AND name=?2", params![chunk, predicate.column], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(db)?;
        pruned |= predicate.lower.zip(maximum).is_some_and(|(lower, maximum)| maximum < lower)
            || predicate.upper.zip(minimum).is_some_and(|(upper, minimum)| minimum >= upper);
    }
    if let Some(time) = &read.time {
        let (minimum, maximum): (Option<i64>, Option<i64>) = state
            .connection
            .query_row("SELECT time_min,time_max FROM arrow_columns WHERE chunk=?1 AND name=?2", params![chunk, time.column], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(db)?;
        pruned |= maximum.is_some_and(|maximum| maximum < time.lower_ns) || minimum.is_some_and(|minimum| minimum >= time.upper_ns);
    }
    if pruned {
        let mut page = columnar::finish_page(snapshot, read, &empty, complete, 0, Vec::new(), 0, cancel)?;
        if let Some(next) = &mut page.next {
            next.position = last.min(snapshot.at.sequence);
        }
        return Ok(page);
    }
    if count > read.work.rows.get() {
        return Err(columnar::exhausted());
    }
    let mut bytes_read = 0usize;
    for name in &required {
        let length: usize = state
            .connection
            .query_row("SELECT length(data) FROM arrow_columns WHERE chunk=?1 AND name=?2", params![chunk, name], |row| size(row, 0))
            .map_err(db)?;
        bytes_read = bytes_read.saturating_add(length);
    }
    if bytes_read.saturating_mul(4) > read.work.bytes.get() {
        return Err(columnar::exhausted());
    }
    let mut fields = Vec::new();
    let mut arrays = Vec::new();
    for name in &required {
        cancel.check()?;
        let data: Vec<u8> = state
            .connection
            .query_row("SELECT data FROM arrow_columns WHERE chunk=?1 AND name=?2", params![chunk, name], |row| row.get(0))
            .map_err(db)?;
        let batch = from_ipc(&data)?;
        if batch.num_rows() != count || batch.num_columns() != 1 || batch.schema().field(0).name() != name {
            return Err(incompatible());
        }
        fields.push(batch.schema().field(0).clone());
        arrays.push(batch.column(0).clone());
    }
    let mut batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).map_err(|_| incompatible())?;
    if bytes_read.saturating_add(batch.get_array_memory_size().saturating_mul(3)) > read.work.bytes.get() {
        return Err(columnar::exhausted());
    }
    if read.source.kind == ColumnKind::Objects {
        let key = text(&read.source.schema)?;
        let mut query = state.connection.prepare("SELECT position FROM storage_rows WHERE kind=1 AND schema_key=?1 AND position>=?3 AND position<=?4 AND position<=?2 AND position=(SELECT max(o.position) FROM storage_rows o WHERE o.kind=1 AND o.schema_key=?1 AND o.item=storage_rows.item AND o.position<=?2)").map_err(db)?;
        let retained = query
            .query_map(params![key, integer(snapshot.at.sequence)?, integer(_first)?, integer(last)?], |row| unsigned(row, 0))
            .map_err(db)?
            .collect::<std::result::Result<BTreeSet<_>, _>>()
            .map_err(db)?;
        let indices = positions(&batch)?
            .values()
            .iter()
            .enumerate()
            .filter_map(|(index, position)| retained.contains(position).then_some(index as u64))
            .collect::<Vec<_>>();
        batch = columnar::take(&batch, &indices)?;
    }
    let final_position = positions(&batch)?.values().iter().copied().filter(|position| *position <= snapshot.at.sequence).max();
    let mut page = columnar::finish_page(snapshot, read, &batch, complete, bytes_read, required, 0, cancel)?;
    if let Some(next) = &mut page.next {
        if final_position.is_none_or(|position| next.position >= position) {
            next.position = last.min(snapshot.at.sequence);
        }
    }
    Ok(page)
}

pub(super) fn ipc(batch: &RecordBatch) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut writer = StreamWriter::try_new(&mut data, &batch.schema()).map_err(|_| invalid("Arrow IPC schema encoding failed"))?;
    writer.write(batch).map_err(|_| invalid("Arrow IPC encoding failed"))?;
    writer.finish().map_err(|_| invalid("Arrow IPC finish failed"))?;
    drop(writer);
    Ok(data)
}

pub(super) fn from_ipc(data: &[u8]) -> Result<RecordBatch> {
    let mut reader = StreamReader::try_new(Cursor::new(data), None).map_err(|_| incompatible())?;
    let batch = reader.next().transpose().map_err(|_| incompatible())?.ok_or_else(incompatible)?;
    if reader.next().is_some() {
        return Err(incompatible());
    }
    Ok(batch)
}

/* === Private Functions === */

fn positions(batch: &RecordBatch) -> Result<&UInt64Array> {
    batch
        .column_by_name("_storage.position")
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .ok_or_else(|| invalid("native source has no positions"))
}

/* === Tests === */
