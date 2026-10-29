/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

use std::io::Cursor;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use arrow_ipc::{MessageHeader, Type, root_as_message_with_opts};
use arrow_schema::Schema;

use super::{MAX_DEPTH, MAX_FIELDS, invalid, validate_schema};
use crate::Result;

pub(super) fn encode(schema: &Schema, batch: Option<&RecordBatch>) -> Result<Vec<u8>> {
    let options = IpcWriteOptions::default();
    let mut encoded = Vec::new();
    let mut writer =
        StreamWriter::try_new_with_options(&mut encoded, schema, options).map_err(|_| invalid("Arrow IPC schema encoding failed"))?;
    if let Some(batch) = batch {
        writer.write(batch).map_err(|_| invalid("Arrow IPC batch encoding failed"))?;
    }
    writer.finish().map_err(|_| invalid("Arrow IPC termination encoding failed"))?;
    drop(writer);
    Ok(encoded)
}

pub(super) fn decode(bytes: &[u8], batch: bool) -> Result<(Arc<Schema>, Option<RecordBatch>)> {
    // The caller performs the allocation preflight before reaching Arrow's decoder.
    let mut reader = StreamReader::try_new(Cursor::new(bytes), None).map_err(|_| invalid("invalid Arrow IPC schema"))?;
    let schema = reader.schema();
    validate_schema(&schema)?;
    let value = reader.next().transpose().map_err(|_| invalid("invalid Arrow IPC batch"))?;
    if value.is_some() != batch || reader.next().is_some() {
        return Err(invalid("Arrow IPC requires exactly the declared batch count"));
    }
    Ok((schema, value))
}

pub(super) fn preflight(bytes: &[u8], batch: bool) -> Result<()> {
    let mut offset = 0usize;
    let mut schemas = 0usize;
    let mut batches = 0usize;
    let mut fields = 0usize;
    let mut terminated = false;
    while offset < bytes.len() {
        let first = read_u32(bytes, &mut offset)?;
        if first != u32::MAX {
            return Err(invalid("Arrow IPC requires modern continuation framing"));
        }
        let length = read_u32(bytes, &mut offset)? as usize;
        if length == 0 {
            terminated = true;
            break;
        }
        let metadata_end = offset.checked_add(length).ok_or_else(|| invalid("IPC length overflow"))?;
        let metadata = bytes.get(offset..metadata_end).ok_or_else(|| invalid("truncated IPC metadata"))?;
        offset = metadata_end;
        let options = flatbuffers::VerifierOptions {
            max_depth: MAX_DEPTH * 4,
            max_tables: MAX_FIELDS * 16,
            max_apparent_size: metadata.len().checked_mul(8).ok_or_else(|| invalid("IPC verifier bound overflow"))?,
            ignore_missing_null_terminator: false,
        };
        let message =
            root_as_message_with_opts(&options, metadata).map_err(|_| invalid("invalid or structurally excessive IPC metadata"))?;
        let body = usize::try_from(message.bodyLength()).map_err(|_| invalid("negative IPC body length"))?;
        offset = offset.checked_add(body).ok_or_else(|| invalid("IPC body length overflow"))?;
        if offset > bytes.len() {
            return Err(invalid("truncated IPC body"));
        }
        match message.header_type() {
            MessageHeader::Schema if schemas == 0 && batches == 0 && body == 0 => {
                let schema = message.header_as_schema().ok_or_else(|| invalid("missing IPC schema"))?;
                if let Some(values) = schema.fields() {
                    for field in values {
                        inspect_field(field, 0, &mut fields)?;
                    }
                }
                schemas += 1;
            }
            MessageHeader::RecordBatch if schemas == 1 && batches == 0 && batch => {
                let record = message.header_as_record_batch().ok_or_else(|| invalid("missing IPC record batch"))?;
                if record.compression().is_some() || record.variadicBufferCounts().is_some() {
                    return Err(invalid("compressed or variadic IPC batches are unsupported"));
                }
                let _rows = usize::try_from(record.length()).map_err(|_| invalid("negative IPC row count"))?;

                let nodes = record.nodes().ok_or_else(|| invalid("missing IPC array nodes"))?;
                if nodes.len() != fields {
                    return Err(invalid("IPC node count differs from schema"));
                }
                for node in nodes {
                    let length = usize::try_from(node.length()).map_err(|_| invalid("negative IPC array length"))?;
                    let nulls = usize::try_from(node.null_count()).map_err(|_| invalid("negative IPC null count"))?;
                    if nulls > length {
                        return Err(invalid("IPC null count exceeds array length"));
                    }
                }
                let buffers = record.buffers().ok_or_else(|| invalid("missing IPC buffers"))?;
                if buffers.len() > fields.saturating_mul(3) {
                    return Err(invalid("IPC buffer count exceeds supported layout"));
                }
                for buffer in buffers {
                    let start = usize::try_from(buffer.offset()).map_err(|_| invalid("negative IPC buffer offset"))?;
                    let length = usize::try_from(buffer.length()).map_err(|_| invalid("negative IPC buffer length"))?;
                    if start.checked_add(length).is_none_or(|end| end > body) {
                        return Err(invalid("IPC buffer is outside its body"));
                    }
                }
                batches += 1;
            }
            _ => {
                return Err(invalid("unexpected IPC message or unsupported dictionary encoding"));
            }
        }
    }
    if !terminated || offset != bytes.len() || schemas != 1 || batches != usize::from(batch) {
        return Err(invalid("invalid IPC schema/batch/termination sequence"));
    }
    Ok(())
}

fn inspect_field(field: arrow_ipc::Field<'_>, depth: usize, fields: &mut usize) -> Result<()> {
    *fields = fields.checked_add(1).ok_or_else(|| invalid("IPC field count overflow"))?;
    if depth > MAX_DEPTH || *fields > MAX_FIELDS || field.dictionary().is_some() {
        return Err(invalid("IPC field depth/count or dictionary encoding unsupported"));
    }
    let children = field.children();
    match field.type_type() {
        Type::Null | Type::Bool | Type::Utf8 | Type::Binary => {
            if children.is_some_and(|children| !children.is_empty()) {
                return Err(invalid("scalar IPC field has children"));
            }
        }
        Type::Int => {
            let integer = field.type_as_int().ok_or_else(|| invalid("invalid IPC integer"))?;
            if !matches!(integer.bitWidth(), 8 | 16 | 32 | 64) {
                return Err(invalid("unsupported IPC integer width"));
            }
            if children.is_some_and(|children| !children.is_empty()) {
                return Err(invalid("scalar IPC field has children"));
            }
        }
        Type::FloatingPoint => {
            let float = field.type_as_floating_point().ok_or_else(|| invalid("invalid IPC float"))?;
            if !matches!(float.precision(), arrow_ipc::Precision::SINGLE | arrow_ipc::Precision::DOUBLE) {
                return Err(invalid("unsupported IPC float width"));
            }
        }
        Type::Timestamp => {
            let timestamp = field.type_as_timestamp().ok_or_else(|| invalid("invalid IPC timestamp"))?;
            if timestamp.unit() != arrow_ipc::TimeUnit::NANOSECOND || timestamp.timezone() != Some("UTC") {
                return Err(invalid("IPC timestamps must be UTC nanoseconds"));
            }
        }
        Type::List if children.is_some_and(|children| children.len() == 1) => {}
        Type::Struct_ => {}
        _ => return Err(invalid("unsupported IPC field type")),
    }
    if let Some(children) = children {
        for child in children {
            inspect_field(child, depth + 1, fields)?;
        }
    }
    if let Some(metadata) = field.custom_metadata()
        && metadata.len() > MAX_FIELDS
    {
        return Err(invalid("excessive IPC field metadata"));
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or_else(|| invalid("IPC prefix overflow"))?;
    let value = bytes.get(*offset..end).ok_or_else(|| invalid("truncated IPC prefix"))?;
    *offset = end;
    Ok(u32::from_le_bytes(value.try_into().expect("four-byte IPC prefix")))
}
