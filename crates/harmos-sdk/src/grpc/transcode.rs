/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protobuf From And To The Canonical Postcard Value, Driven By The Described Shape.
//
// Every message crosses process boundaries as its Postcard value; the protobuf
// form packaging publishes in `.proto` files is a projection of the same
// described fields. Fields are written in tag order, required scalars equal to
// their default are omitted, numeric repeated fields are packed, and a message
// whose fields are all variants is a oneof whose Postcard form is the variant
// index followed by that variant's fields. Floats are default only at positive
// zero, so negative zero keeps its sign across the wire.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use prost::encoding::{WireType, encode_key, encode_varint};
// Local Imports
use super::input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
use harmos_domain::__private::{PostcardReader as Reader, PostcardValue};
use harmos_domain::MessageError;
use harmos_domain::schema::Cardinality;
use harmos_domain::schema::declaration::{Definition, Field, Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
/// Append the protobuf form of one Postcard-encoded `definition` value.
pub fn encode(definition: &Definition, postcard: &[u8], bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let mut input = Reader::new(postcard);
    message(definition, &mut input, bytes)?;
    input.finish()
}

/// The Postcard form of one protobuf `definition` message.
pub fn decode(definition: &Definition, input: DecodeInput<'_>) -> Result<Vec<u8>, MessageError> {
    let mut bytes = Vec::new();
    decode_message(definition, input, &mut bytes)?;
    Ok(bytes)
}

/* === Private Functions === */
fn oneof(fields: &[Field]) -> bool {
    !fields.is_empty() && fields.iter().all(|field| field.cardinality == Cardinality::Variant)
}

fn payload(field: &Field) -> Result<&Definition, MessageError> {
    match &field.kind {
        Kind::Message { definition } => Ok(definition),
        _ => Err(MessageError::borrowed("a oneof variant carries a message payload")),
    }
}

/* --- Postcard To Protobuf --- */

fn message(definition: &Definition, input: &mut Reader<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    if oneof(fields) {
        let index: u32 = input.value()?;
        let field = usize::try_from(index)
            .ok()
            .and_then(|index| fields.get(index))
            .ok_or_else(|| MessageError::borrowed("unknown Postcard enum variant"))?;
        let mut body = Vec::new();
        members(payload(field)?, input, &mut body)?;
        delimited(field.tag, &body, bytes);
        return Ok(());
    }
    members(definition, input, bytes)
}

// Postcard Holds Fields In Declaration Order; Protobuf Writes Them By Tag.
fn members(definition: &Definition, input: &mut Reader<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    let mut chunks = Vec::with_capacity(fields.len());
    for field in fields {
        let mut chunk = Vec::new();
        member(field, input, &mut chunk)?;
        if !chunk.is_empty() {
            chunks.push((field.tag, chunk));
        }
    }
    chunks.sort_by_key(|(tag, _)| *tag);
    for (_, chunk) in chunks {
        bytes.extend_from_slice(&chunk);
    }
    Ok(())
}

fn member(field: &Field, input: &mut Reader<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    match (field.cardinality, &field.kind) {
        (Cardinality::Required, Kind::Message { definition }) => nested(field.tag, definition, input, bytes),
        (Cardinality::Required, kind) => scalar(field.tag, kind, input, bytes, true),
        (Cardinality::Optional, kind) => {
            if !input.value::<bool>()? {
                return Ok(());
            }
            match kind {
                Kind::Message { definition } => nested(field.tag, definition, input, bytes),
                kind => scalar(field.tag, kind, input, bytes, false),
            }
        }
        (Cardinality::Repeated, kind) => {
            let count: usize = input.value()?;
            match kind {
                Kind::Message { definition } => {
                    for _ in 0..count {
                        nested(field.tag, definition, input, bytes)?;
                    }
                }
                Kind::String | Kind::Bytes => {
                    for _ in 0..count {
                        scalar(field.tag, kind, input, bytes, false)?;
                    }
                }
                kind if count > 0 => {
                    let mut packed = Vec::new();
                    for _ in 0..count {
                        number(kind, input, &mut packed)?;
                    }
                    delimited(field.tag, &packed, bytes);
                }
                _ => {}
            }
            Ok(())
        }
        (Cardinality::Variant, _) => Err(MessageError::borrowed("a variant field belongs to a oneof")),
    }
}

fn nested(tag: u32, definition: &Definition, input: &mut Reader<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let mut body = Vec::new();
    message(definition, input, &mut body)?;
    delimited(tag, &body, bytes);
    Ok(())
}

fn scalar(tag: u32, kind: &Kind, input: &mut Reader<'_>, bytes: &mut Vec<u8>, omit_default: bool) -> Result<(), MessageError> {
    match kind {
        Kind::String => {
            let text: &str = input.value()?;
            if !(omit_default && text.is_empty()) {
                delimited(tag, text.as_bytes(), bytes);
            }
        }
        Kind::Bytes => {
            let length: usize = input.value()?;
            let value = input.take(length)?;
            if !(omit_default && value.is_empty()) {
                delimited(tag, value, bytes);
            }
        }
        kind => {
            let mut value = Vec::new();
            if number(kind, input, &mut value)? || !omit_default {
                encode_key(tag, wire(kind), bytes);
                bytes.extend_from_slice(&value);
            }
        }
    }
    Ok(())
}

// Write One Numeric Value Without Its Key; True When It Differs From Default.
fn number(kind: &Kind, input: &mut Reader<'_>, bytes: &mut Vec<u8>) -> Result<bool, MessageError> {
    Ok(match kind {
        Kind::Bool => {
            let value: bool = input.value()?;
            encode_varint(u64::from(value), bytes);
            value
        }
        Kind::U32 => {
            let value: u32 = input.value()?;
            encode_varint(u64::from(value), bytes);
            value != 0
        }
        Kind::U64 => {
            let value: u64 = input.value()?;
            encode_varint(value, bytes);
            value != 0
        }
        Kind::I32 => {
            let value: i32 = input.value()?;
            encode_varint(i64::from(value) as u64, bytes);
            value != 0
        }
        Kind::I64 => {
            let value: i64 = input.value()?;
            encode_varint(value as u64, bytes);
            value != 0
        }
        Kind::F32 => {
            let value: f32 = input.value()?;
            bytes.extend_from_slice(&value.to_le_bytes());
            value.to_bits() != 0
        }
        Kind::F64 => {
            let value: f64 = input.value()?;
            bytes.extend_from_slice(&value.to_le_bytes());
            value.to_bits() != 0
        }
        Kind::String | Kind::Bytes | Kind::Message { .. } => {
            return Err(MessageError::borrowed("expected a numeric protobuf field"));
        }
    })
}

fn wire(kind: &Kind) -> WireType {
    match kind {
        Kind::F32 => WireType::ThirtyTwoBit,
        Kind::F64 => WireType::SixtyFourBit,
        Kind::String | Kind::Bytes | Kind::Message { .. } => WireType::LengthDelimited,
        _ => WireType::Varint,
    }
}

fn delimited(tag: u32, value: &[u8], bytes: &mut Vec<u8>) {
    encode_key(tag, WireType::LengthDelimited, bytes);
    encode_varint(value.len() as u64, bytes);
    bytes.extend_from_slice(value);
}

/* --- Protobuf To Postcard --- */

fn decode_message(definition: &Definition, input: DecodeInput<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    if !oneof(fields) {
        return decode_members(definition, input, bytes);
    }
    let tags = fields.iter().map(|field| field.tag).collect::<Vec<_>>();
    let (index, selected) = match input.variant(&tags)? {
        Some((tag, selected)) => (tags.iter().position(|known| *known == tag).expect("selected declared variant"), selected),
        None => (definition.default_variant.ok_or_else(|| MessageError::borrowed("missing oneof variant"))?, input.child(&[])?),
    };
    let field = fields.get(index).ok_or_else(|| MessageError::borrowed("default variant lies outside the oneof"))?;
    u32::try_from(index).map_err(|_| MessageError::borrowed("oneof variant index overflows"))?.write(bytes);
    decode_members(payload(field)?, selected, bytes)
}

fn decode_members(definition: &Definition, input: DecodeInput<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    // Check Every Occurrence Of A Known Field Before Any Is Selected
    input.fields(&mut |occurrence| {
        if let Some(field) = fields.iter().find(|field| field.tag == occurrence.tag) {
            check(field, occurrence)?;
        }
        Ok(())
    })?;
    for field in fields {
        decode_member(field, input, bytes)?;
    }
    Ok(())
}

fn check(field: &Field, occurrence: DecodeField<'_>) -> Result<(), MessageError> {
    match (&field.kind, field.cardinality) {
        (Kind::String, _) => occurrence.text().map(drop),
        (Kind::Bytes | Kind::Message { .. }, _) => occurrence.bytes().map(drop),
        (kind, Cardinality::Repeated) => occurrence.scalars(decode_scalar(kind), &mut |_| Ok(())),
        (kind, _) => occurrence.scalar(decode_scalar(kind)).map(drop),
    }
}

fn decode_member(field: &Field, input: DecodeInput<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let tag = field.tag;
    match (field.cardinality, &field.kind) {
        (Cardinality::Required, Kind::Message { definition }) => decode_message(definition, input.field(tag)?, bytes),
        (Cardinality::Optional, Kind::Message { definition }) => {
            let present = last(input, tag)?.is_some();
            present.write(bytes);
            if present {
                decode_message(definition, input.field(tag)?, bytes)?;
            }
            Ok(())
        }
        (Cardinality::Repeated, Kind::Message { definition }) => {
            count(input, tag)?.write(bytes);
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    decode_message(definition, input.child(occurrence.bytes()?)?, bytes)?;
                }
                Ok(())
            })
        }
        (Cardinality::Required, kind) => match last(input, tag)? {
            Some(occurrence) => decode_scalar_value(kind, occurrence, bytes),
            None => default(kind, bytes),
        },
        (Cardinality::Optional, kind) => {
            let occurrence = last(input, tag)?;
            occurrence.is_some().write(bytes);
            occurrence.map_or(Ok(()), |occurrence| decode_scalar_value(kind, occurrence, bytes))
        }
        (Cardinality::Repeated, kind @ (Kind::String | Kind::Bytes)) => {
            count(input, tag)?.write(bytes);
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    decode_scalar_value(kind, occurrence, bytes)?;
                }
                Ok(())
            })
        }
        (Cardinality::Repeated, kind) => {
            let scalar = decode_scalar(kind);
            let mut values = 0usize;
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    occurrence.scalars(scalar, &mut |_| {
                        values += 1;
                        Ok(())
                    })?;
                }
                Ok(())
            })?;
            values.write(bytes);
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    occurrence.scalars(scalar, &mut |value| write_number(kind, value, bytes))?;
                }
                Ok(())
            })
        }
        (Cardinality::Variant, _) => Err(MessageError::borrowed("a variant field belongs to a oneof")),
    }
}

fn last<'a>(input: DecodeInput<'a>, tag: u32) -> Result<Option<DecodeField<'a>>, MessageError> {
    let mut selected = None;
    input.fields(&mut |occurrence| {
        if occurrence.tag == tag {
            selected = Some(occurrence);
        }
        Ok(())
    })?;
    Ok(selected)
}

fn count(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError> {
    let mut count = 0usize;
    input.fields(&mut |occurrence| {
        if occurrence.tag == tag {
            count += 1;
        }
        Ok(())
    })?;
    Ok(count)
}

fn decode_scalar(kind: &Kind) -> DecodeScalar {
    match kind {
        Kind::F32 => DecodeScalar::Bits32,
        Kind::F64 => DecodeScalar::Bits64,
        _ => DecodeScalar::Varint,
    }
}

fn decode_scalar_value(kind: &Kind, occurrence: DecodeField<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    match kind {
        Kind::String => {
            let text = occurrence.text()?;
            text.len().write(bytes);
            bytes.extend_from_slice(text.as_bytes());
            Ok(())
        }
        Kind::Bytes => {
            let value = occurrence.bytes()?;
            value.len().write(bytes);
            bytes.extend_from_slice(value);
            Ok(())
        }
        kind => write_number(kind, occurrence.scalar(decode_scalar(kind))?, bytes),
    }
}

fn write_number(kind: &Kind, value: DecodeValue<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    match (kind, value) {
        (Kind::Bool, DecodeValue::Varint(value)) => (value != 0).write(bytes),
        (Kind::U32, DecodeValue::Varint(value)) => (value as u32).write(bytes),
        (Kind::U64, DecodeValue::Varint(value)) => value.write(bytes),
        (Kind::I32, DecodeValue::Varint(value)) => (value as i32).write(bytes),
        (Kind::I64, DecodeValue::Varint(value)) => (value as i64).write(bytes),
        (Kind::F32, DecodeValue::Bits32(value)) => f32::from_bits(value).write(bytes),
        (Kind::F64, DecodeValue::Bits64(value)) => f64::from_bits(value).write(bytes),
        _ => return Err(MessageError::borrowed("protobuf scalar wire type mismatch")),
    }
    Ok(())
}

fn default(kind: &Kind, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    match kind {
        Kind::Bool => false.write(bytes),
        Kind::U32 => 0u32.write(bytes),
        Kind::U64 => 0u64.write(bytes),
        Kind::I32 => 0i32.write(bytes),
        Kind::I64 => 0i64.write(bytes),
        Kind::F32 => 0f32.write(bytes),
        Kind::F64 => 0f64.write(bytes),
        Kind::String | Kind::Bytes => 0usize.write(bytes),
        Kind::Message { .. } => {
            return Err(MessageError::borrowed("expected a scalar protobuf field"));
        }
    }
    Ok(())
}

/* === Tests === */
