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

// Third Party Imports
use bytes::Buf;
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};
use prost_reflect::{Kind, MessageDescriptor};
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
            return Ok(32);
        }
        ancestor = node.parent;
    }
    let node = Layout { message, parent };
    let mut bytes = 32;
    for field in message.fields() {
        let slot = if field.is_map() {
            64
        } else if field.is_list() {
            32
        } else {
            element(&field.kind(), Some(&node))?
        };
        // Includes Optional Presence, Oneof Discriminants and Alignment Padding.
        bytes = add(bytes, add(slot, 16)?)?;
    }
    Ok(bytes)
}

fn element(kind: &Kind, parent: Option<&Layout<'_>>) -> Result<usize, Status> {
    match kind {
        Kind::Message(message) => layout(message, parent),
        Kind::String | Kind::Bytes => Ok(32),
        _ => Ok(16),
    }
}

fn allocations(message: &MessageDescriptor, mut encoded: &[u8], depth: usize) -> Result<usize, Status> {
    if depth == 0 {
        return Err(Status::data_loss("protobuf nesting limit exceeded"));
    }
    let mut bytes = 0;
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
                    bytes = add(bytes, memory_at(nested, payload, depth - 1)?)?;
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
            bytes = add(bytes, memory_at(nested, body, depth - 1)?)?;
        } else {
            skip_field(wire, tag, &mut encoded, DecodeContext::default()).map_err(invalid)?;
        }
        if field.is_map() {
            // Covers Hash Tables and Generated BTreeMaps, Including Their First Node.
            bytes = add(bytes, multiply(add(element(&kind, None)?, 64)?, 16)?)?;
        } else if field.is_list() {
            // Covers Geometric Growth and the Minimum Capacity for Short Lists.
            bytes = add(bytes, multiply(multiply(element(&kind, None)?.max(8), count)?, 4)?)?;
        }
    }
    Ok(bytes)
}

fn memory_at(message: &MessageDescriptor, encoded: &[u8], depth: usize) -> Result<usize, Status> {
    add(layout(message, None)?, allocations(message, encoded, depth)?)
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
