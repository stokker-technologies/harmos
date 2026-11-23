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
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::mem::{align_of, size_of};

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

struct Ancestor<'a> {
    message: &'a MessageDescriptor,
    parent: Option<&'a Ancestor<'a>>,
}

#[derive(Clone, Copy)]
struct Inline {
    size: usize,
    align: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Inline {
    const EMPTY: Self = Self { size: 0, align: 1 };

    const fn of<T>() -> Self {
        Self {
            size: size_of::<T>(),
            align: align_of::<T>(),
        }
    }

    fn padded(self, tag: usize) -> Self {
        Self {
            size: self.size.next_multiple_of(self.align).saturating_add(tag),
            align: self.align,
        }
    }
}

/* === Public Functions === */

/// Upper Bound of a Generated Prost Value's Retained Bytes, Without Decoding Values.
///
/// Inline storage follows the generated layout: fields keep their width, rustc orders them by
/// alignment, `Option` adds one aligned tag unless its value has a niche, and a oneof adds one tag.
/// Heap storage follows prost's decode path: strings and bytes reserve their length, repeated
/// fields push from Vec's first capacity, and maps take the larger of a hashbrown table and a
/// B-tree with half-full nodes. A singular message encoded twice, or a `merged` value that other
/// occurrences extend, counts its collections double. Bytes and map slots assume the larger
/// `bytes::Bytes` and `HashMap`.
pub(super) fn memory(message: &MessageDescriptor, encoded: &[u8], root: usize, merged: bool) -> Result<usize, Status> {
    add(layout(message, None).size.max(root), allocations(message, encoded, merged, 100)?)
}

pub(super) fn overflow() -> Status {
    Status::resource_exhausted("client retained_bytes overflow; raise retained_bytes or request smaller pages")
}

/* === Private Functions === */

fn layout(message: &MessageDescriptor, parent: Option<&Ancestor<'_>>) -> Inline {
    if let Some(inline) = google(message) {
        return inline;
    }
    let mut ancestor = parent;
    while let Some(node) = ancestor {
        if node.message == message {
            // Recursive Generated Fields Are Boxed or Live in a Collection.
            return Inline::of::<usize>();
        }
        ancestor = node.parent;
    }
    let node = Ancestor { message, parent };
    let mut total = Inline::EMPTY;
    let mut oneofs = BTreeSet::new();
    for field in message.fields() {
        let inline = match field.containing_oneof().filter(|oneof| !oneof.is_synthetic()) {
            Some(oneof) if !oneofs.insert(oneof.full_name().to_owned()) => continue,
            // One Enum Tag After the Largest Variant; Option Uses the Tag's Spare Values.
            Some(oneof) => {
                let largest = oneof.fields().map(|variant| slot(&variant, Some(&node))).fold(Inline::EMPTY, |left, right| Inline {
                    size: left.size.max(right.size),
                    align: left.align.max(right.align),
                });
                largest.padded(largest.align)
            }
            None if field.supports_presence() && !field.is_list() && !field.is_map() && !niche(&field) => {
                let inline = slot(&field, Some(&node));
                inline.padded(inline.align)
            }
            None => slot(&field, Some(&node)),
        };
        total = Inline {
            size: total.size.saturating_add(inline.size),
            align: total.align.max(inline.align),
        };
    }
    total.padded(0)
}

fn slot(field: &FieldDescriptor, parent: Option<&Ancestor<'_>>) -> Inline {
    if field.is_map() {
        return Inline::of::<HashMap<(), ()>>();
    }
    if field.is_list() {
        return Inline::of::<Vec<()>>();
    }
    if boxed(field) {
        return Inline::of::<usize>();
    }
    element(&field.kind(), parent)
}

// Option Needs No Tag Beside Spare Bit Patterns: Bools, Strings, Bytes, Boxes, and Messages Holding
// a List, One of Those, or an Optional Field Outside a Oneof. Maps May Be B-Trees Without One.
fn niche(field: &FieldDescriptor) -> bool {
    match field.kind() {
        Kind::Bool | Kind::String | Kind::Bytes => true,
        Kind::Message(nested) => {
            boxed(field)
                || nested.fields().any(|inner| {
                    inner.is_list()
                        || matches!(inner.kind(), Kind::Bool | Kind::String | Kind::Bytes)
                        || (inner.supports_presence()
                            && !inner.is_map()
                            && !boxed(&inner)
                            && inner.containing_oneof().is_none_or(|oneof| oneof.is_synthetic()))
                })
        }
        _ => false,
    }
}

// Prost-Types Fixes the Descriptor and Struct Shapes Even When Their Descriptors Are Newer; Other
// Google Messages Follow Their Descriptors, Whose Added Fields Only Raise the Bound.
fn google(message: &MessageDescriptor) -> Option<Inline> {
    Some(match message.full_name().strip_prefix("google.protobuf.")? {
        "FileDescriptorProto" => Inline::of::<prost_types::FileDescriptorProto>(),
        "DescriptorProto" => Inline::of::<prost_types::DescriptorProto>(),
        "DescriptorProto.ExtensionRange" => Inline::of::<prost_types::descriptor_proto::ExtensionRange>(),
        "FieldDescriptorProto" => Inline::of::<prost_types::FieldDescriptorProto>(),
        "OneofDescriptorProto" => Inline::of::<prost_types::OneofDescriptorProto>(),
        "EnumDescriptorProto" => Inline::of::<prost_types::EnumDescriptorProto>(),
        "EnumValueDescriptorProto" => Inline::of::<prost_types::EnumValueDescriptorProto>(),
        "ServiceDescriptorProto" => Inline::of::<prost_types::ServiceDescriptorProto>(),
        "MethodDescriptorProto" => Inline::of::<prost_types::MethodDescriptorProto>(),
        "Struct" => Inline::of::<prost_types::Struct>(),
        _ => return None,
    })
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

fn element(kind: &Kind, parent: Option<&Ancestor<'_>>) -> Inline {
    match kind {
        Kind::Message(message) => layout(message, parent),
        Kind::String => Inline::of::<String>(),
        // Bytes Fields Generate as Vec<u8> or the Larger bytes::Bytes.
        Kind::Bytes => Inline::of::<bytes::Bytes>(),
        Kind::Bool => Inline::of::<bool>(),
        Kind::Double | Kind::Int64 | Kind::Uint64 | Kind::Sint64 | Kind::Fixed64 | Kind::Sfixed64 => Inline::of::<u64>(),
        // Remaining Scalars and Enumerations Generate as 32-Bit Values.
        _ => Inline::of::<u32>(),
    }
}

fn allocations(message: &MessageDescriptor, encoded: &[u8], merged: bool, depth: usize) -> Result<usize, Status> {
    if depth == 0 {
        return Err(Status::data_loss("protobuf nesting limit exceeded"));
    }
    let mut bytes = 0;
    let mut counts = BTreeMap::new();
    // Singular Messages Seen Twice, or Numbered Past the Mask, Merge Their Occurrences.
    let (mut seen, mut twice, mut singular) = (0u64, 0u64, false);
    fields(message, encoded, |field, payload| {
        let mut count = 1;
        match (field.kind(), payload) {
            (Kind::Message(_), Some(_)) if !field.is_list() && !field.is_map() => {
                let bit = 1u64.checked_shl(field.number()).unwrap_or(0);
                twice |= seen & bit;
                seen |= bit;
                singular = true;
            }
            // Repeated Elements and Map Entries Are Fresh Values, Even in a Merged Parent.
            (Kind::Message(nested), Some(payload)) => bytes = add(bytes, allocations(&nested, payload, false, depth - 1)?)?,
            // Vec Reserves the Exact Length From Its Eight-Byte First Allocation.
            (Kind::String | Kind::Bytes, Some(payload)) => bytes = add(bytes, payload.len().max(8))?,
            (kind, Some(payload)) if field.is_list() => count = packed_count(&kind, payload)?,
            (_, Some(_)) => return Err(Status::data_loss("invalid length-delimited scalar")),
            (_, None) => {}
        }
        if field.is_list() || field.is_map() {
            let total = counts.entry(field.number()).or_insert(0usize);
            *total = add(*total, count)?;
        }
        Ok(())
    })?;
    if singular {
        fields(message, encoded, |field, payload| {
            if let (Kind::Message(nested), Some(payload)) = (field.kind(), payload)
                && !field.is_list()
                && !field.is_map()
            {
                let repeated = 1u64.checked_shl(field.number()).is_none_or(|bit| twice & bit != 0);
                bytes = add(bytes, allocations(&nested, payload, merged || repeated, depth - 1)?)?;
                if boxed(&field) {
                    bytes = add(bytes, layout(&nested, None).size)?;
                }
            }
            Ok(())
        })?;
    }
    for (tag, count) in counts {
        let field = message.get_field(tag).expect("scanned repeated field");
        let entry = element(&field.kind(), None).size;
        let count = if merged {
            multiply(count, 2)?
        } else {
            count
        };
        bytes = add(
            bytes,
            if field.is_map() {
                table(count, entry)?
            } else {
                vector(count, entry)?
            },
        )?;
    }
    Ok(bytes)
}

fn fields<'a>(
    message: &MessageDescriptor, mut encoded: &'a [u8], mut visit: impl FnMut(FieldDescriptor, Option<&'a [u8]>) -> Result<(), Status>,
) -> Result<(), Status> {
    while encoded.has_remaining() {
        let (tag, wire) = decode_key(&mut encoded).map_err(invalid)?;
        let Some(field) = message.get_field(tag) else {
            skip_field(wire, tag, &mut encoded, DecodeContext::default()).map_err(invalid)?;
            continue;
        };
        let payload = match wire {
            WireType::LengthDelimited => {
                let length = usize::try_from(decode_varint(&mut encoded).map_err(invalid)?).map_err(invalid)?;
                if length > encoded.len() {
                    return Err(Status::data_loss("truncated protobuf field"));
                }
                let (payload, remaining) = encoded.split_at(length);
                encoded = remaining;
                Some(payload)
            }
            WireType::StartGroup => {
                if !matches!(field.kind(), Kind::Message(_)) {
                    return Err(Status::data_loss("invalid protobuf group field"));
                }
                let (body, remaining) = group(encoded, tag)?;
                encoded = remaining;
                Some(body)
            }
            _ => {
                skip_field(wire, tag, &mut encoded, DecodeContext::default()).map_err(invalid)?;
                None
            }
        };
        visit(field, payload)?;
    }
    Ok(())
}

// Vec Doubles From Its First Capacity: Eight Bytes, Four Elements, or One Element Above 1 KiB.
fn vector(count: usize, element: usize) -> Result<usize, Status> {
    if count == 0 || element == 0 {
        return Ok(0);
    }
    let first = match element {
        1 => 8,
        2..=1024 => 4,
        _ => 1,
    };
    // An Element Estimated Past 1 KiB May Still Be Smaller and Start With Four.
    let floor = if element > 1024 {
        4096
    } else {
        0
    };
    Ok(multiply(element, count.checked_next_power_of_two().ok_or_else(overflow)?.max(first))?.max(floor))
}

// Hashbrown Keeps an Eighth of Its Buckets Free Plus a Control Byte Each and One Trailing Group;
// B-Tree Nodes Hold Eleven Entries and Stay At Least Half Full, With a Parent Link and Twelve Edges.
fn table(count: usize, entry: usize) -> Result<usize, Status> {
    if count == 0 {
        return Ok(0);
    }
    let buckets = match count {
        1..4 => 4,
        4..8 => 8,
        8..15 => 16,
        _ => (multiply(count, 8)? / 7).checked_next_power_of_two().ok_or_else(overflow)?,
    };
    let hashed = add(multiply(buckets, add(entry, 1)?)?.max(64), 32)?;
    let node = add(
        multiply(entry, 11)?,
        if count <= 11 {
            16
        } else {
            112
        },
    )?;
    let tree = multiply(
        if count <= 11 {
            1
        } else {
            (count - 1) / 5 + 1
        },
        node,
    )?;
    Ok(hashed.max(tree))
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
    left.checked_add(right).ok_or_else(overflow)
}

fn multiply(left: usize, right: usize) -> Result<usize, Status> {
    left.checked_mul(right).ok_or_else(overflow)
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
        let required = memory(&value.descriptor(), &encoded, std::mem::size_of_val(&value), false).unwrap();
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
        let required = memory(&value.descriptor(), &encoded, std::mem::size_of_val(&value), false).unwrap();
        assert!(required >= 5 * "payload".len() * 128);
        let truncated = &encoded[..encoded.len() - 1];
        assert_eq!(memory(&value.descriptor(), truncated, 0, false).unwrap_err().code(), tonic::Code::DataLoss);
    }
}
