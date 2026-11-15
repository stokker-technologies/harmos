/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Descriptor-Backed Generated Message Admission

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

// Third Party Imports
use bytes::Buf;
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};
use prost_reflect::{FieldDescriptor, Kind, MessageDescriptor};
use tonic::Status;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Layout<'a> {
    message: &'a MessageDescriptor,
    parent: Option<&'a Layout<'a>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Conservative Generated Rust Layout and Allocation Bound, Without Decoding Values.
pub(super) fn memory(message: &MessageDescriptor, encoded: &[u8], root: usize) -> Result<usize, Status> {
    add(layout(message, None)?.max(root), allocations(message, encoded, 100)?)
}

/* === Private Functions === */

fn layout(message: &MessageDescriptor, parent: Option<&Layout<'_>>) -> Result<usize, Status> {
    let mut ancestor = parent;
    while let Some(node) = ancestor {
        if node.message == message {
            // Recursive Generated Fields Are Boxed or Live in a Collection.
            return Ok(size_of::<usize>());
        }
        ancestor = node.parent;
    }
    let node = Layout { message, parent };
    let mut bytes = 0;
    let mut oneofs = BTreeSet::new();
    for field in message.fields() {
        if let Some(oneof) = field.containing_oneof().filter(|oneof| !oneof.is_synthetic()) {
            if !oneofs.insert(oneof.full_name().to_owned()) {
                continue;
            }
            let mut largest = 0;
            for variant in oneof.fields() {
                largest = largest.max(slot(&variant, Some(&node))?);
            }
            // One Enum Payload Plus Its Discriminant, Including Option Presence.
            bytes = add(bytes, add(largest, size_of::<usize>())?)?;
            continue;
        }
        let mut storage = slot(&field, Some(&node))?;
        if field.supports_presence() && !field.is_list() && !field.is_map() {
            storage = add(storage, size_of::<usize>())?;
        }
        bytes = add(bytes, storage)?;
    }
    Ok(bytes)
}

fn slot(field: &FieldDescriptor, parent: Option<&Layout<'_>>) -> Result<usize, Status> {
    if field.is_map() {
        return Ok(size_of::<std::collections::HashMap<(), ()>>());
    }
    if field.is_list() {
        return Ok(size_of::<Vec<()>>());
    }
    if boxed(field) {
        return Ok(size_of::<usize>());
    }
    element(&field.kind(), parent)
}

// Prost Boxes Singular Edges That Close a Message Cycle; Collection Edges Already Indirect.
fn boxed(field: &FieldDescriptor) -> bool {
    let Kind::Message(nested) = field.kind() else {
        return false;
    };
    !field.is_list() && !field.is_map() && reaches(&nested, field.parent_message(), &mut BTreeSet::new())
}

fn reaches(message: &MessageDescriptor, target: &MessageDescriptor, seen: &mut BTreeSet<String>) -> bool {
    if message == target {
        return true;
    }
    if !seen.insert(message.full_name().to_owned()) {
        return false;
    }
    message
        .fields()
        .filter(|field| !field.is_list() && !field.is_map())
        .any(|field| matches!(field.kind(), Kind::Message(nested) if reaches(&nested, target, seen)))
}

fn element(kind: &Kind, parent: Option<&Layout<'_>>) -> Result<usize, Status> {
    match kind {
        Kind::Message(message) => layout(message, parent),
        Kind::String => Ok(size_of::<String>()),
        Kind::Bytes => Ok(size_of::<Vec<u8>>().max(size_of::<bytes::Bytes>())),
        // Rounding Each Scalar to Pointer Alignment Bounds Rust Field Reordering.
        _ => Ok(8),
    }
}

fn allocations(message: &MessageDescriptor, mut encoded: &[u8], depth: usize) -> Result<usize, Status> {
    if depth == 0 {
        return Err(Status::data_loss("protobuf nesting limit exceeded"));
    }
    let mut bytes = 0;
    let mut repeated = BTreeMap::new();
    while encoded.has_remaining() {
        let (tag, wire) = decode_key(&mut encoded).map_err(invalid)?;
        let Some(field) = message.get_field(tag) else {
            skip_field(wire, tag, &mut encoded, DecodeContext::default()).map_err(invalid)?;
            continue;
        };
        let kind = field.kind();
        let mut count = 1;
        if wire == WireType::LengthDelimited {
            let length = usize::try_from(decode_varint(&mut encoded).map_err(invalid)?).map_err(invalid)?;
            if length > encoded.len() {
                return Err(Status::data_loss("truncated protobuf field"));
            }
            let (payload, remaining) = encoded.split_at(length);
            encoded = remaining;
            match &kind {
                Kind::Message(nested) => {
                    bytes = add(bytes, allocations(nested, payload, depth - 1)?)?;
                    if boxed(&field) {
                        bytes = add(bytes, layout(nested, None)?)?;
                    }
                }
                Kind::String | Kind::Bytes => {
                    // Covers Growth, Replacement and the Smallest Vec Allocation.
                    bytes = add(bytes, add(multiply(length, 2)?, 8)?)?;
                }
                _ if field.is_list() => count = packed_count(&kind, payload)?,
                _ => return Err(Status::data_loss("invalid length-delimited scalar")),
            }
        } else if wire == WireType::StartGroup {
            let (body, remaining) = group(encoded, tag)?;
            encoded = remaining;
            let Kind::Message(nested) = &kind else {
                return Err(Status::data_loss("invalid protobuf group field"));
            };
            bytes = add(bytes, allocations(nested, body, depth - 1)?)?;
            if boxed(&field) {
                bytes = add(bytes, layout(nested, None)?)?;
            }
        } else {
            skip_field(wire, tag, &mut encoded, DecodeContext::default()).map_err(invalid)?;
        }
        if field.is_map() {
            // Covers Hash Tables and Generated BTreeMaps, Including Their First Node.
            bytes = add(bytes, multiply(add(element(&kind, None)?, 64)?, 16)?)?;
        } else if field.is_list() {
            let total = repeated.entry(field.number()).or_insert(0usize);
            *total = add(*total, count)?;
        }
    }
    for (tag, count) in repeated {
        let field = message.get_field(tag).expect("scanned repeated field");
        let storage = element(&field.kind(), None)?.max(1);
        let minimum = if storage == 1 {
            8
        } else if storage <= 1024 {
            4
        } else {
            1
        };
        // Vec's Minimum Capacity and At Most Geometric Growth for Present Elements.
        bytes = add(bytes, multiply(storage, multiply(count.max(minimum), 2)?)?)?;
    }
    Ok(bytes)
}

fn group(encoded: &[u8], tag: u32) -> Result<(&[u8], &[u8]), Status> {
    let mut remaining = encoded;
    while remaining.has_remaining() {
        let body_length = encoded.len() - remaining.len();
        let (field, wire) = decode_key(&mut remaining).map_err(invalid)?;
        if wire == WireType::EndGroup {
            if field != tag {
                return Err(Status::data_loss("mismatched protobuf end group"));
            }
            return Ok((&encoded[..body_length], remaining));
        }
        skip_field(wire, field, &mut remaining, DecodeContext::default()).map_err(invalid)?;
    }
    Err(Status::data_loss("unterminated protobuf group"))
}

fn packed_count(kind: &Kind, mut payload: &[u8]) -> Result<usize, Status> {
    let width = match kind {
        Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => Some(8),
        Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => Some(4),
        _ => None,
    };
    if let Some(width) = width {
        if !payload.len().is_multiple_of(width) {
            return Err(Status::data_loss("invalid packed scalar length"));
        }
        return Ok(payload.len() / width);
    }
    let mut count = 0;
    while payload.has_remaining() {
        decode_varint(&mut payload).map_err(invalid)?;
        count = add(count, 1)?;
    }
    Ok(count)
}

fn add(left: usize, right: usize) -> Result<usize, Status> {
    left.checked_add(right)
        .ok_or_else(|| Status::resource_exhausted("client retained_bytes overflow; raise retained_bytes or request smaller pages"))
}

fn multiply(left: usize, right: usize) -> Result<usize, Status> {
    left.checked_mul(right)
        .ok_or_else(|| Status::resource_exhausted("client retained_bytes overflow; raise retained_bytes or request smaller pages"))
}

fn invalid(error: impl std::fmt::Display) -> Status {
    Status::data_loss(error.to_string())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use prost_reflect::ReflectMessage;

    #[test]
    fn compact_repeated_messages_reserve_generated_storage() {
        let value = prost_types::ListValue {
            values: vec![prost_types::Value::default(); 128],
        };
        let encoded = value.encode_to_vec();
        let required = memory(&value.descriptor(), &encoded, std::mem::size_of_val(&value)).unwrap();
        let storage = std::mem::size_of_val(&value) + value.values.capacity() * std::mem::size_of::<prost_types::Value>();
        assert!(required >= storage);
        assert!(storage > encoded.len());
    }

    #[test]
    fn map_entries_and_nested_strings_reserve_storage() {
        let value = prost_types::Struct {
            fields: [(
                "record".into(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::ListValue(prost_types::ListValue {
                        values: vec![
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::StringValue("payload".repeat(128)))
                            };
                            5
                        ],
                    })),
                },
            )]
            .into(),
        };
        let encoded = value.encode_to_vec();
        let required = memory(&value.descriptor(), &encoded, std::mem::size_of_val(&value)).unwrap();
        assert!(required >= 5 * "payload".len() * 128);
        let truncated = &encoded[..encoded.len() - 1];
        assert_eq!(memory(&value.descriptor(), truncated, 0).unwrap_err().code(), tonic::Code::DataLoss);
    }
}
