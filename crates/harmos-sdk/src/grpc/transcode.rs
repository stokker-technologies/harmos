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
use prost::encoding::{WireType, encode_varint};
// Local Imports
use super::input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
use harmos_domain::__private::{PostcardReader as Reader, PostcardValue, decoding_add, decoding_multiply};
use harmos_domain::MessageError;
use harmos_domain::schema::Cardinality;
use harmos_domain::schema::declaration::{Definition, Field, Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Output<'a, 'payload> {
    bytes: Option<&'a mut Vec<u8>>,
    length: usize,
    retained: usize,
    admit: Option<&'a mut dyn FnMut(usize) -> Result<(), MessageError>>,
    spans: Vec<FieldSpan<'payload>>,
    parameter: Option<BytesParameter<'a>>,
}

#[derive(Clone, Copy)]
struct BytesParameter<'a> {
    path: &'a [&'a str],
    cursor: Option<usize>,
    length: usize,
    matched: usize,
}

struct FieldSpan<'payload> {
    index: usize,
    input: Reader<'payload>,
}

/* === Enums === */

enum Number {
    Varint(u64),
    Bits32(u32),
    Bits64(u64),
}

/* === Traits === */

/* === Implementations === */

impl<'a, 'payload> Output<'a, 'payload> {
    fn counting() -> Self {
        Self {
            bytes: None,
            length: 0,
            retained: 0,
            admit: None,
            spans: Vec::new(),
            parameter: None,
        }
    }

    fn child_counter(&self) -> Self {
        let parameter = self.parameter.map(|mut parameter| {
            parameter.matched = 0;
            parameter
        });
        Self { parameter, ..Self::counting() }
    }

    fn enter(&mut self, field: &Field) -> Result<Option<usize>, MessageError> {
        let Some(parameter) = &mut self.parameter else {
            return Ok(None);
        };
        let previous = parameter.cursor;
        let selected = previous.is_some_and(|index| parameter.path.get(index).copied() == Some(field.name.as_ref()));
        parameter.cursor = previous.filter(|_| selected).map(|index| index + 1);
        if selected && field.cardinality == Cardinality::Repeated {
            return Err(MessageError::borrowed("a bytes parameter cannot select a repeated field"));
        }
        if parameter.cursor == Some(parameter.path.len()) && !matches!(field.kind, Kind::Bytes) {
            return Err(MessageError::borrowed("a bytes parameter must select a bytes field"));
        }
        Ok(previous)
    }

    fn leave(&mut self, previous: Option<usize>) {
        if let Some(parameter) = &mut self.parameter {
            parameter.cursor = previous;
        }
    }

    fn bytes_length(&mut self, actual: usize) -> Result<usize, MessageError> {
        let Some(parameter) = &mut self.parameter else {
            return Ok(actual);
        };
        if parameter.cursor != Some(parameter.path.len()) {
            return Ok(actual);
        }
        parameter.matched = decoding_add(parameter.matched, 1)?;
        Ok(parameter.length)
    }

    fn reserve_spans(&mut self, additional: usize) -> Result<(), MessageError> {
        let required = decoding_add(self.spans.len(), additional)?;
        if required <= self.spans.capacity() {
            return Ok(());
        }
        let retained = decoding_multiply(self.spans.capacity(), std::mem::size_of::<FieldSpan<'_>>())?;
        let replacement = decoding_multiply(required, std::mem::size_of::<FieldSpan<'_>>())?;
        let peak = decoding_add(self.retained, decoding_add(retained, replacement)?)?;
        if let Some(admit) = &mut self.admit {
            admit(peak)?;
        }
        self.spans.try_reserve_exact(additional).map_err(|_| MessageError::borrowed("protobuf field index allocation failed"))?;
        let retained = decoding_multiply(self.spans.capacity(), std::mem::size_of::<FieldSpan<'_>>())?;
        if let Some(admit) = &mut self.admit {
            admit(decoding_add(self.retained, retained)?)?;
        }
        Ok(())
    }

    fn append(&mut self, value: &[u8]) -> Result<(), MessageError> {
        self.advance(value.len())?;
        if let Some(bytes) = &mut self.bytes {
            bytes.extend_from_slice(value);
        }
        Ok(())
    }

    fn advance(&mut self, length: usize) -> Result<(), MessageError> {
        self.length = self.length.checked_add(length).ok_or_else(|| MessageError::borrowed("protobuf encoding size overflows"))?;
        Ok(())
    }

    fn varint(&mut self, value: u64) -> Result<(), MessageError> {
        let mut bytes = [0; 10];
        let mut remaining = &mut bytes[..];
        encode_varint(value, &mut remaining);
        let length = 10 - remaining.len();
        self.append(&bytes[..length])
    }

    fn key(&mut self, tag: u32, wire: WireType) -> Result<(), MessageError> {
        self.varint((u64::from(tag) << 3) | wire as u64)
    }

    fn header(&mut self, tag: u32, length: usize) -> Result<(), MessageError> {
        self.key(tag, WireType::LengthDelimited)?;
        self.varint(length as u64)
    }
}

impl Number {
    fn is_default(&self) -> bool {
        match self {
            Self::Varint(value) | Self::Bits64(value) => *value == 0,
            Self::Bits32(value) => *value == 0,
        }
    }

    fn write(&self, output: &mut Output<'_, '_>) -> Result<(), MessageError> {
        match self {
            Self::Varint(value) => output.varint(*value),
            Self::Bits32(value) => output.append(&value.to_le_bytes()),
            Self::Bits64(value) => output.append(&value.to_le_bytes()),
        }
    }
}

/* === Public Functions === */

pub fn encode_admitted(
    definition: &Definition, postcard: &[u8], bytes: &mut Vec<u8>, retained: usize,
    admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    let mut input = Reader::new(postcard);
    let mut output = Output {
        bytes: Some(bytes),
        length: 0,
        retained,
        admit: Some(admit),
        spans: Vec::new(),
        parameter: None,
    };
    message(definition, &mut input, &mut output)?;
    input.finish()
}

pub fn encoded_len(definition: &Definition, postcard: &[u8]) -> Result<usize, MessageError> {
    let mut input = Reader::new(postcard);
    let mut output = Output::counting();
    message(definition, &mut input, &mut output)?;
    input.finish()?;
    Ok(output.length)
}

pub fn parameterized_len(definition: &Definition, postcard: &[u8], path: &[&str], length: usize) -> Result<usize, MessageError> {
    if path.is_empty() {
        return Err(MessageError::borrowed("a bytes parameter requires a field path"));
    }
    let mut input = Reader::new(postcard);
    let parameter = BytesParameter {
        path,
        cursor: Some(0),
        length,
        matched: 0,
    };
    let mut output = Output {
        parameter: Some(parameter),
        ..Output::counting()
    };
    message(definition, &mut input, &mut output)?;
    input.finish()?;
    if output.parameter.is_none_or(|parameter| parameter.matched != 1) {
        return Err(MessageError::borrowed("bytes parameter does not select one present field"));
    }
    Ok(output.length)
}

pub fn decode(definition: &Definition, input: DecodeInput<'_>) -> Result<Vec<u8>, MessageError> {
    // Postcard Rarely Outgrows Its Protobuf Source, So The Bound Stays Near The Payload.
    let mut bytes = Vec::with_capacity(input.encoded_len()?);
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

fn message<'payload>(definition: &Definition, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    if oneof(fields) {
        let index: u32 = input.value()?;
        let field = usize::try_from(index)
            .ok()
            .and_then(|index| fields.get(index))
            .ok_or_else(|| MessageError::borrowed("unknown Postcard enum variant"))?;
        let definition = payload(field)?;
        let previous = bytes.enter(field)?;
        let result = delimited(field.tag, input, bytes, |input, bytes| members(definition, input, bytes));
        bytes.leave(previous);
        return result;
    }
    members(definition, input, bytes)
}

// Postcard Holds Fields In Declaration Order; Protobuf Writes Them By Tag.
fn members<'payload>(definition: &Definition, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>) -> Result<(), MessageError> {
    let fields = (definition.fields)();
    if bytes.bytes.is_none() || fields.windows(2).all(|pair| pair[0].tag < pair[1].tag) {
        for field in fields {
            member(field, input, bytes)?;
        }
        return Ok(());
    }
    // One Admitted Index Retains Parent Spans And Reuses Child Workspace.
    bytes.reserve_spans(fields.len())?;
    let start = bytes.spans.len();
    for (index, field) in fields.iter().enumerate() {
        bytes.spans.push(FieldSpan { index, input: input.clone() });
        member(field, input, &mut Output::counting())?;
    }
    let end = bytes.spans.len();
    bytes.spans[start..end].sort_unstable_by_key(|span| (fields[span.index].tag, span.index));
    for position in start..end {
        let span = &bytes.spans[position];
        let field = &fields[span.index];
        let mut cursor = span.input.clone();
        member(field, &mut cursor, bytes)?;
    }
    bytes.spans.truncate(start);
    Ok(())
}

fn member<'payload>(field: &Field, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>) -> Result<(), MessageError> {
    let previous = bytes.enter(field)?;
    let result = member_value(field, input, bytes);
    bytes.leave(previous);
    result
}

fn member_value<'payload>(field: &Field, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>) -> Result<(), MessageError> {
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
                    delimited(field.tag, input, bytes, |input, bytes| {
                        for _ in 0..count {
                            number(kind, input)?.write(bytes)?;
                        }
                        Ok(())
                    })?;
                }
                _ => {}
            }
            Ok(())
        }
        (Cardinality::Variant, _) => Err(MessageError::borrowed("a variant field belongs to a oneof")),
    }
}

fn nested<'payload>(
    tag: u32, definition: &Definition, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    delimited(tag, input, bytes, |input, bytes| message(definition, input, bytes))
}

fn scalar<'payload>(
    tag: u32, kind: &Kind, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>, omit_default: bool,
) -> Result<(), MessageError> {
    match kind {
        Kind::String => {
            let text: &str = input.value()?;
            if !(omit_default && text.is_empty()) {
                bytes.header(tag, text.len())?;
                bytes.append(text.as_bytes())?;
            }
        }
        Kind::Bytes => {
            let length: usize = input.value()?;
            let value = input.take(length)?;
            let length = bytes.bytes_length(value.len())?;
            if !(omit_default && length == 0) {
                bytes.header(tag, length)?;
                if bytes.bytes.is_none() {
                    bytes.advance(length)?;
                } else {
                    bytes.append(value)?;
                }
            }
        }
        kind => {
            let value = number(kind, input)?;
            if !value.is_default() || !omit_default {
                bytes.key(tag, wire(kind))?;
                value.write(bytes)?;
            }
        }
    }
    Ok(())
}

fn number(kind: &Kind, input: &mut Reader<'_>) -> Result<Number, MessageError> {
    Ok(match kind {
        Kind::Bool => {
            let value: bool = input.value()?;
            Number::Varint(u64::from(value))
        }
        Kind::U32 => {
            let value: u32 = input.value()?;
            Number::Varint(u64::from(value))
        }
        Kind::U64 => {
            let value: u64 = input.value()?;
            Number::Varint(value)
        }
        Kind::I32 => {
            let value: i32 = input.value()?;
            Number::Varint(i64::from(value) as u64)
        }
        Kind::I64 => {
            let value: i64 = input.value()?;
            Number::Varint(value as u64)
        }
        Kind::F32 => {
            let value: f32 = input.value()?;
            Number::Bits32(value.to_bits())
        }
        Kind::F64 => {
            let value: f64 = input.value()?;
            Number::Bits64(value.to_bits())
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

fn delimited<'payload>(
    tag: u32, input: &mut Reader<'payload>, bytes: &mut Output<'_, 'payload>,
    write: impl Fn(&mut Reader<'payload>, &mut Output<'_, 'payload>) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    let mut end = input.clone();
    let mut count = bytes.child_counter();
    write(&mut end, &mut count)?;
    if let Some(parameter) = &mut bytes.parameter {
        parameter.matched = decoding_add(parameter.matched, count.parameter.map_or(0, |parameter| parameter.matched))?;
    }
    bytes.header(tag, count.length)?;
    if bytes.bytes.is_none() {
        bytes.advance(count.length)?;
        *input = end;
        return Ok(());
    }
    write(input, bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Leaf {
        value: i64,
        negative_zero: f64,
    }

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Row {
        leaf: Leaf,
        values: Vec<u64>,
        optional: Option<u32>,
    }

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    enum Choice {
        Empty,
        Row(Row),
    }

    #[harmos_domain::message(harmos_path = ::harmos_domain)]
    struct Wide {
        alpha: u32,
        bravo: Row,
        charlie: u64,
        delta: Choice,
        echo: Vec<Row>,
        foxtrot: Option<Row>,
        golf: String,
        hotel: Vec<u64>,
        india: Vec<u8>,
        juliet: Leaf,
        kilo: bool,
        lima: i64,
    }

    #[test]
    fn counted_nested_packed_and_variant_encoding() {
        use harmos_domain::{Message, schema::Described};
        let value = Choice::Row(Row {
            leaf: Leaf { value: -1, negative_zero: -0.0 },
            values: vec![0, 127, 128, u64::MAX],
            optional: Some(0),
        });
        let postcard = value.encode();
        let expected = encoded_len(&Choice::DEFINITION, &postcard).unwrap();
        let mut bytes = Vec::with_capacity(expected);
        encode_admitted(&Choice::DEFINITION, &postcard, &mut bytes, 0, &mut |_| Ok(())).unwrap();
        assert_eq!(bytes.len(), expected);
        assert_eq!(decode(&Choice::DEFINITION, DecodeInput::new(&bytes)).unwrap(), postcard);
        let mut truncated = postcard.clone();
        truncated.pop();
        assert!(encoded_len(&Choice::DEFINITION, &truncated).is_err());
        let mut trailing = postcard;
        trailing.push(0);
        assert!(encoded_len(&Choice::DEFINITION, &trailing).is_err());
    }

    #[test]
    fn declaration_order_is_projected_in_tag_order() {
        use harmos_domain::schema::Described;
        let definition = Definition { fields: || FIELDS, ..Leaf::DEFINITION };
        const FIELDS: &[Field] = &[
            Field {
                name: Cow::Borrowed("later"),
                tag: 3,
                kind: Kind::U32,
                cardinality: Cardinality::Required,
            },
            Field {
                name: Cow::Borrowed("first"),
                tag: 1,
                kind: Kind::U32,
                cardinality: Cardinality::Optional,
            },
            Field {
                name: Cow::Borrowed("middle"),
                tag: 2,
                kind: Kind::U32,
                cardinality: Cardinality::Repeated,
            },
        ];
        let postcard = postcard::to_allocvec(&(7u32, Some(0u32), vec![127u32, 128])).unwrap();
        let mut bytes = vec![0xaa];
        encode_admitted(&definition, &postcard, &mut bytes, 0, &mut |_| Ok(())).unwrap();
        assert_eq!(bytes, [0xaa, 0x08, 0, 0x12, 3, 127, 128, 1, 0x18, 7]);
        assert_eq!(encoded_len(&definition, &postcard).unwrap(), bytes.len() - 1);
        assert_eq!(decode(&definition, DecodeInput::new(&bytes[1..])).unwrap(), postcard);
    }

    #[test]
    fn hashed_field_index_is_admitted_and_preserves_nested_spans() {
        use harmos_domain::{Message, schema::Described};
        let row = |seed| Row {
            leaf: Leaf {
                value: -(seed as i64),
                negative_zero: -0.0,
            },
            values: vec![seed; 64],
            optional: Some(0),
        };
        let value = Wide {
            alpha: 17,
            bravo: row(1),
            charlie: u64::MAX,
            delta: Choice::Row(row(2)),
            echo: vec![row(3), row(4), row(5)],
            foxtrot: Some(row(6)),
            golf: "field spans".into(),
            hotel: vec![0, 127, 128, u64::MAX],
            india: vec![0xff; 4096],
            juliet: Leaf { value: -1, negative_zero: -0.0 },
            kilo: true,
            lima: i64::MIN,
        };
        assert!(Wide::FIELDS.windows(2).any(|pair| pair[0].tag > pair[1].tag));
        let postcard = value.encode();
        let length = encoded_len(&Wide::DEFINITION, &postcard).unwrap();
        let mut bytes = Vec::with_capacity(length);
        let baseline = postcard.capacity() + bytes.capacity();

        let refused = crate::grpc::encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
            if required > baseline { Err(MessageError::borrowed("fixture index refusal")) } else { Ok(()) }
        });
        assert!(refused.is_err());
        assert!(bytes.is_empty());

        let mut claims = Vec::new();
        crate::grpc::encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
            claims.push(required);
            Ok(())
        })
        .unwrap();
        assert_eq!(bytes.len(), length);
        assert!(claims.iter().any(|required| *required > baseline));
        assert_eq!(claims.last(), Some(&bytes.capacity()));
        assert_eq!(decode(&Wide::DEFINITION, DecodeInput::new(&bytes)).unwrap(), postcard);

        let mut tags = Vec::new();
        DecodeInput::new(&bytes)
            .fields(&mut |field| {
                tags.push(field.tag);
                Ok(())
            })
            .unwrap();
        assert_eq!(tags.len(), Wide::FIELDS.len());
        assert!(tags.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn index_growth_refusal_retains_existing_backing() {
        use std::cell::Cell;
        let ceiling = Cell::new(usize::MAX);
        let retained = Cell::new(0);
        let mut admit = |required| {
            if required > ceiling.get() {
                return Err(MessageError::borrowed("fixture arena growth refusal"));
            }
            retained.set(required);
            Ok(())
        };
        let mut output = Output::counting();
        output.retained = 23;
        output.admit = Some(&mut admit);
        output.reserve_spans(4).unwrap();
        let capacity = output.spans.capacity();
        let pointer = output.spans.as_ptr();
        let claimed = retained.get();

        // The Final Arena Fits; Replacing Its Still-Live Backing Does Not.
        let requested = capacity + 1;
        ceiling.set(23 + requested * std::mem::size_of::<FieldSpan<'_>>());
        assert!(output.reserve_spans(requested).is_err());
        assert_eq!(output.spans.capacity(), capacity);
        assert_eq!(output.spans.as_ptr(), pointer);
        assert_eq!(retained.get(), claimed);

        ceiling.set(usize::MAX);
        output.reserve_spans(requested).unwrap();
        assert!(output.spans.capacity() >= requested);
        assert_eq!(retained.get(), 23 + output.spans.capacity() * std::mem::size_of::<FieldSpan<'_>>());
    }
}
