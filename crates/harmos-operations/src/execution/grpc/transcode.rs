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
use std::collections::{BTreeMap, HashMap};
use std::mem::size_of;

// Third Party Imports
use prost::encoding::{WireType, encode_varint};
use prost_reflect::{MapKey, MessageDescriptor};

// Local Imports
use super::input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
use harmos_domain::__private::{PostcardReader, PostcardValue, decoding_add, decoding_multiply};
use harmos_domain::MessageError;
use harmos_domain::schema::Cardinality;
use harmos_domain::schema::{
    declaration::Definition,
    graph::{Field, Kind, Schema},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Descriptor-Backed View of a Canonical Stream Schema.
#[derive(Clone)]
pub struct Dynamic<'a> {
    descriptor: MessageDescriptor,
    schema: &'a Schema,
    root: usize,
    depth: usize,
}

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
    input: PostcardReader<'payload>,
}

/* === Enums === */

enum Number {
    Varint(u64),
    Bits32(u32),
    Bits64(u64),
}

/* === Traits === */

pub(super) trait Shape: Clone {
    type Reference;

    fn fields(&self) -> &[Field<Self::Reference>];
    fn child(&self, field: &Field<Self::Reference>) -> Result<Self, MessageError>;
    fn default_variant(&self) -> Option<usize>;
    fn format(&self, _tag: u32) -> Option<prost_reflect::Kind> {
        None
    }
    fn map(&self, _tag: u32) -> bool {
        false
    }
    fn map_value(&self, _tag: u32) -> bool {
        false
    }
    fn check_depth(&self) -> Result<(), MessageError> {
        Ok(())
    }
    fn present(&self, tag: u32, input: DecodeInput<'_>) -> Result<bool, MessageError> {
        Ok(last(input, tag)?.is_some())
    }
    fn input<'a>(&self, tag: u32, input: &'a DecodeInput<'_>) -> Result<DecodeInput<'a>, MessageError> {
        input.field(tag)
    }
}

/* === Implementations === */

impl Shape for Definition {
    type Reference = Definition;

    fn fields(&self) -> &[Field<Definition>] {
        (self.fields)()
    }
    fn child(&self, field: &Field<Definition>) -> Result<Self, MessageError> {
        match field.kind {
            Kind::Message { definition } => Ok(definition),
            _ => Err(MessageError::borrowed("a nested field requires a message payload")),
        }
    }
    fn default_variant(&self) -> Option<usize> {
        self.default_variant
    }
}

impl Shape for Dynamic<'_> {
    type Reference = usize;

    fn fields(&self) -> &[Field] {
        &self.schema.definitions[self.root]
    }
    fn child(&self, field: &Field) -> Result<Self, MessageError> {
        let Kind::Message { definition } = field.kind else {
            return Err(MessageError::borrowed("a nested field requires a message payload"));
        };
        let Some(prost_reflect::Kind::Message(descriptor)) = self.format(field.tag) else {
            return Err(MessageError::borrowed("refresh the descriptor to match the canonical schema"));
        };
        Ok(Self {
            descriptor,
            schema: self.schema,
            root: definition,
            depth: self.depth + 1,
        })
    }
    fn default_variant(&self) -> Option<usize> {
        None
    }
    fn format(&self, tag: u32) -> Option<prost_reflect::Kind> {
        self.descriptor.get_field(tag).map(|field| field.kind())
    }
    fn map(&self, tag: u32) -> bool {
        self.descriptor.get_field(tag).is_some_and(|field| field.is_map())
    }
    fn map_value(&self, tag: u32) -> bool {
        self.descriptor.is_map_entry() && tag == 2
    }
    fn check_depth(&self) -> Result<(), MessageError> {
        if self.depth >= harmos_domain::schema::MAX_DEPTH {
            return Err(MessageError::borrowed("Reduce Stream Row Nesting to the Structural Limit"));
        }
        Ok(())
    }
    fn present(&self, tag: u32, input: DecodeInput<'_>) -> Result<bool, MessageError> {
        if self.map_value(tag) {
            return Ok(true);
        }
        let field = self.descriptor.get_field(tag).expect("schema field descriptor");
        let Some(oneof) = field.containing_oneof() else {
            return Ok(last(input, tag)?.is_some());
        };
        let selected = input.select(|tag| oneof.fields().any(|field| field.number() == tag), |_| Ok(()))?;
        Ok(selected.is_some_and(|(selected, _)| selected == tag))
    }
    fn input<'a>(&self, tag: u32, input: &'a DecodeInput<'_>) -> Result<DecodeInput<'a>, MessageError> {
        let field = self.descriptor.get_field(tag).expect("schema field descriptor");
        let Some(oneof) = field.containing_oneof() else {
            return input.field(tag);
        };
        let selected = input.select(|tag| oneof.fields().any(|field| field.number() == tag), |_| Ok(()))?;
        match selected {
            Some((selected, input)) if selected == tag => Ok(input),
            _ => input.child(&[]),
        }
    }
}

impl<'a> Dynamic<'a> {
    pub fn new(descriptor: MessageDescriptor, schema: &'a Schema) -> Self {
        Self {
            descriptor,
            schema,
            root: schema.root,
            depth: 0,
        }
    }

    pub fn schema(descriptor: &MessageDescriptor) -> Result<Schema, MessageError> {
        let mut messages = vec![descriptor.clone()];
        let mut indices = BTreeMap::from([(descriptor.full_name().to_owned(), 0)]);
        let mut definitions = Vec::new();
        while definitions.len() < messages.len() {
            let message = messages[definitions.len()].clone();
            let mut fields = Vec::new();
            for field in message.fields() {
                let kind = match field.kind() {
                    prost_reflect::Kind::Bool => Kind::Bool,
                    prost_reflect::Kind::Uint32 | prost_reflect::Kind::Fixed32 => Kind::U32,
                    prost_reflect::Kind::Uint64 | prost_reflect::Kind::Fixed64 => Kind::U64,
                    prost_reflect::Kind::Int32
                    | prost_reflect::Kind::Sint32
                    | prost_reflect::Kind::Sfixed32
                    | prost_reflect::Kind::Enum(_) => Kind::I32,
                    prost_reflect::Kind::Int64 | prost_reflect::Kind::Sint64 | prost_reflect::Kind::Sfixed64 => Kind::I64,
                    prost_reflect::Kind::Float => Kind::F32,
                    prost_reflect::Kind::Double => Kind::F64,
                    prost_reflect::Kind::String => Kind::String,
                    prost_reflect::Kind::Bytes => Kind::Bytes,
                    prost_reflect::Kind::Message(message) => {
                        let next = messages.len();
                        let definition = *indices.entry(message.full_name().to_owned()).or_insert_with(|| {
                            messages.push(message);
                            next
                        });
                        Kind::Message { definition }
                    }
                };
                let cardinality = if field.is_list() || field.is_map() {
                    Cardinality::Repeated
                } else if field.supports_presence() {
                    Cardinality::Optional
                } else {
                    Cardinality::Required
                };
                fields.push(Field {
                    name: field.name().to_owned().into(),
                    tag: field.number(),
                    kind,
                    cardinality,
                });
            }
            definitions.push(fields);
        }
        let schema = Schema {
            root: 0,
            definitions,
            representations: BTreeMap::new(),
        };
        schema.prepare(&Default::default()).map_err(|error| MessageError::new(error.to_string()))?;
        Ok(schema)
    }

    pub fn encode(&self, postcard: &[u8], bytes: &mut Vec<u8>) -> Result<(), MessageError> {
        encode_admitted(self, postcard, bytes, 0, &mut |_| Ok(()))
    }

    pub fn decode(&self, input: DecodeInput<'_>) -> Result<Vec<u8>, MessageError> {
        decode(self, input)
    }
}

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

    fn enter<R>(&mut self, field: &Field<R>) -> Result<Option<usize>, MessageError> {
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
        let retained = decoding_multiply(self.spans.capacity(), size_of::<FieldSpan<'_>>())?;
        let replacement = decoding_multiply(required, size_of::<FieldSpan<'_>>())?;
        let peak = decoding_add(self.retained, decoding_add(retained, replacement)?)?;
        if let Some(admit) = &mut self.admit {
            admit(peak)?;
        }
        self.spans.try_reserve_exact(additional).map_err(|_| MessageError::borrowed("protobuf field index allocation failed"))?;
        let retained = decoding_multiply(self.spans.capacity(), size_of::<FieldSpan<'_>>())?;
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

pub(super) fn encode_admitted(
    definition: &impl Shape, postcard: &[u8], bytes: &mut Vec<u8>, retained: usize,
    admit: &mut dyn FnMut(usize) -> Result<(), MessageError>,
) -> Result<(), MessageError> {
    let mut input = PostcardReader::new(postcard);
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

pub(super) fn encoded_len(definition: &impl Shape, postcard: &[u8]) -> Result<usize, MessageError> {
    let mut input = PostcardReader::new(postcard);
    let mut output = Output::counting();
    message(definition, &mut input, &mut output)?;
    input.finish()?;
    Ok(output.length)
}

pub(super) fn parameterized_len(definition: &impl Shape, postcard: &[u8], path: &[&str], length: usize) -> Result<usize, MessageError> {
    if path.is_empty() {
        return Err(MessageError::borrowed("a bytes parameter requires a field path"));
    }
    let mut input = PostcardReader::new(postcard);
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

pub(super) fn decode(definition: &impl Shape, input: DecodeInput<'_>) -> Result<Vec<u8>, MessageError> {
    // Postcard Rarely Outgrows Its Protobuf Source, So The Bound Stays Near The Payload.
    let mut bytes = Vec::with_capacity(input.encoded_len()?);
    decode_message(definition, input, &mut bytes)?;
    Ok(bytes)
}

/* === Private Functions === */

fn oneof<R>(fields: &[Field<R>]) -> bool {
    !fields.is_empty() && fields.iter().all(|field| field.cardinality == Cardinality::Variant)
}

/* --- Postcard To Protobuf --- */

fn message<'payload>(
    definition: &impl Shape, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    definition.check_depth()?;
    let fields = definition.fields();
    if oneof(fields) {
        let index: u32 = input.value()?;
        let field = usize::try_from(index)
            .ok()
            .and_then(|index| fields.get(index))
            .ok_or_else(|| MessageError::borrowed("unknown Postcard enum variant"))?;
        let payload = definition.child(field)?;
        let previous = bytes.enter(field)?;
        let result = delimited(field.tag, input, bytes, |input, bytes| members(&payload, input, bytes));
        bytes.leave(previous);
        return result;
    }
    members(definition, input, bytes)
}

// Postcard Holds Fields In Declaration Order; Protobuf Writes Them By Tag.
fn members<'payload>(
    definition: &impl Shape, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    let fields = definition.fields();
    if bytes.bytes.is_none() || fields.windows(2).all(|pair| pair[0].tag < pair[1].tag) {
        for field in fields {
            member(definition, field, input, bytes)?;
        }
        return Ok(());
    }
    // One Admitted Index Retains Parent Spans And Reuses Child Workspace.
    bytes.reserve_spans(fields.len())?;
    let start = bytes.spans.len();
    for (index, field) in fields.iter().enumerate() {
        bytes.spans.push(FieldSpan { index, input: input.clone() });
        member(definition, field, input, &mut Output::counting())?;
    }
    let end = bytes.spans.len();
    bytes.spans[start..end].sort_unstable_by_key(|span| (fields[span.index].tag, span.index));
    for position in start..end {
        let span = &bytes.spans[position];
        let field = &fields[span.index];
        let mut cursor = span.input.clone();
        member(definition, field, &mut cursor, bytes)?;
    }
    bytes.spans.truncate(start);
    Ok(())
}

fn member<'payload, S: Shape>(
    definition: &S, field: &Field<S::Reference>, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    let previous = bytes.enter(field)?;
    let result = member_value(definition, field, input, bytes);
    bytes.leave(previous);
    result
}

fn member_value<'payload, S: Shape>(
    definition: &S, field: &Field<S::Reference>, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    match (field.cardinality, &field.kind) {
        (Cardinality::Required, Kind::Message { .. }) => nested(field.tag, &definition.child(field)?, input, bytes),
        (Cardinality::Required, kind) => scalar(field.tag, kind, &definition.format(field.tag), input, bytes, true),
        (Cardinality::Optional, kind) => {
            if !input.value::<bool>()? {
                if definition.map_value(field.tag) {
                    return Err(MessageError::borrowed("Read Stream Row: Supply Present Map Values"));
                }
                return Ok(());
            }
            match kind {
                Kind::Message { .. } => nested(field.tag, &definition.child(field)?, input, bytes),
                kind => scalar(field.tag, kind, &definition.format(field.tag), input, bytes, false),
            }
        }
        (Cardinality::Repeated, kind) => {
            let count: usize = input.value()?;
            let format = definition.format(field.tag);
            match kind {
                Kind::Message { .. } => {
                    let child = definition.child(field)?;
                    for _ in 0..count {
                        nested(field.tag, &child, input, bytes)?;
                    }
                }
                Kind::String | Kind::Bytes => {
                    for _ in 0..count {
                        scalar(field.tag, kind, &definition.format(field.tag), input, bytes, false)?;
                    }
                }
                kind if count > 0 => {
                    delimited(field.tag, input, bytes, |input, bytes| {
                        for _ in 0..count {
                            number(kind, &format, input)?.write(bytes)?;
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
    tag: u32, definition: &impl Shape, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
) -> Result<(), MessageError> {
    delimited(tag, input, bytes, |input, bytes| input.nested(|input| message(definition, input, bytes)))
}

fn scalar<'payload, R>(
    tag: u32, kind: &Kind<R>, format: &Option<prost_reflect::Kind>, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
    omit_default: bool,
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
            let value = number(kind, format, input)?;
            if !value.is_default() || !omit_default {
                bytes.key(tag, wire(kind, format))?;
                value.write(bytes)?;
            }
        }
    }
    Ok(())
}

fn number<R>(kind: &Kind<R>, format: &Option<prost_reflect::Kind>, input: &mut PostcardReader<'_>) -> Result<Number, MessageError> {
    let value = match kind {
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
    };
    Ok(match (format, value) {
        (Some(prost_reflect::Kind::Sint32), Number::Varint(value)) => {
            let value = value as i32;
            Number::Varint(u64::from(((value << 1) ^ (value >> 31)) as u32))
        }
        (Some(prost_reflect::Kind::Sint64), Number::Varint(value)) => {
            let value = value as i64;
            Number::Varint(((value << 1) ^ (value >> 63)) as u64)
        }
        (Some(prost_reflect::Kind::Fixed32 | prost_reflect::Kind::Sfixed32), Number::Varint(value)) => Number::Bits32(value as u32),
        (Some(prost_reflect::Kind::Fixed64 | prost_reflect::Kind::Sfixed64), Number::Varint(value)) => Number::Bits64(value),
        (_, value) => value,
    })
}

fn wire<R>(kind: &Kind<R>, format: &Option<prost_reflect::Kind>) -> WireType {
    match format {
        Some(prost_reflect::Kind::Fixed32 | prost_reflect::Kind::Sfixed32) => return WireType::ThirtyTwoBit,
        Some(prost_reflect::Kind::Fixed64 | prost_reflect::Kind::Sfixed64) => return WireType::SixtyFourBit,
        _ => {}
    }
    match kind {
        Kind::F32 => WireType::ThirtyTwoBit,
        Kind::F64 => WireType::SixtyFourBit,
        Kind::String | Kind::Bytes | Kind::Message { .. } => WireType::LengthDelimited,
        _ => WireType::Varint,
    }
}

fn delimited<'payload>(
    tag: u32, input: &mut PostcardReader<'payload>, bytes: &mut Output<'_, 'payload>,
    write: impl Fn(&mut PostcardReader<'payload>, &mut Output<'_, 'payload>) -> Result<(), MessageError>,
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

fn decode_message(definition: &impl Shape, input: DecodeInput<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    definition.check_depth()?;
    let fields = definition.fields();
    if !oneof(fields) {
        return decode_members(definition, input, bytes);
    }
    let tags = fields.iter().map(|field| field.tag).collect::<Vec<_>>();
    let (index, selected) = match input.variant(&tags)? {
        Some((tag, selected)) => (tags.iter().position(|known| *known == tag).expect("selected declared variant"), selected),
        None => (definition.default_variant().ok_or_else(|| MessageError::borrowed("missing oneof variant"))?, input.child(&[])?),
    };
    let field = fields.get(index).ok_or_else(|| MessageError::borrowed("default variant lies outside the oneof"))?;
    u32::try_from(index).map_err(|_| MessageError::borrowed("oneof variant index overflows"))?.write(bytes);
    decode_members(&definition.child(field)?, selected, bytes)
}

fn decode_members(definition: &impl Shape, input: DecodeInput<'_>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let fields = definition.fields();
    // Check Every Occurrence of a Known Field Before Any Is Selected
    input.fields(&mut |occurrence| {
        if let Some(field) = fields.iter().find(|field| field.tag == occurrence.tag) {
            check(definition, field, input, occurrence)?;
        }
        Ok(())
    })?;
    for field in fields {
        decode_member(definition, field, input, bytes)?;
    }
    Ok(())
}

fn check<'a, S: Shape>(
    definition: &S, field: &Field<S::Reference>, input: DecodeInput<'a>, occurrence: DecodeField<'a>,
) -> Result<(), MessageError> {
    let format = definition.format(field.tag);
    match (&field.kind, field.cardinality) {
        (Kind::String, _) => occurrence.text().map(drop),
        (Kind::Bytes, _) => occurrence.bytes().map(drop),
        (Kind::Message { .. }, _) => {
            let definition = definition.child(field)?;
            definition.check_depth()?;
            let input = input.child(occurrence.bytes()?)?;
            input.fields(&mut |occurrence| {
                if let Some(field) = definition.fields().iter().find(|field| field.tag == occurrence.tag) {
                    check(&definition, field, input, occurrence)?;
                }
                Ok(())
            })
        }
        (kind, Cardinality::Repeated) => occurrence.scalars(decode_scalar(kind, &format), &mut |_| Ok(())),
        (kind, _) => occurrence.scalar(decode_scalar(kind, &format)).map(drop),
    }
}

fn decode_member<S: Shape>(
    definition: &S, field: &Field<S::Reference>, input: DecodeInput<'_>, bytes: &mut Vec<u8>,
) -> Result<(), MessageError> {
    let tag = field.tag;
    match (field.cardinality, &field.kind) {
        (Cardinality::Required, Kind::Message { .. }) => decode_message(&definition.child(field)?, definition.input(tag, &input)?, bytes),
        (Cardinality::Optional, Kind::Message { .. }) => {
            let present = definition.present(tag, input)?;
            present.write(bytes);
            if present {
                decode_message(&definition.child(field)?, definition.input(tag, &input)?, bytes)?;
            }
            Ok(())
        }
        (Cardinality::Repeated, Kind::Message { .. }) => {
            let child = definition.child(field)?;
            if definition.map(tag) {
                return decode_map(&child, input, tag, bytes);
            }
            count(input, tag)?.write(bytes);
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    decode_message(&child, input.child(occurrence.bytes()?)?, bytes)?;
                }
                Ok(())
            })
        }
        (Cardinality::Required, kind) => match last(input, tag)? {
            Some(occurrence) => decode_scalar_value(kind, &definition.format(tag), occurrence, bytes),
            None => default(kind, bytes),
        },
        (Cardinality::Optional, kind) => {
            let occurrence = if definition.present(tag, input)? { last(input, tag)? } else { None };
            occurrence.is_some().write(bytes);
            occurrence.map_or(Ok(()), |occurrence| decode_scalar_value(kind, &definition.format(tag), occurrence, bytes))
        }
        (Cardinality::Repeated, kind @ (Kind::String | Kind::Bytes)) => {
            count(input, tag)?.write(bytes);
            input.fields(&mut |occurrence| {
                if occurrence.tag == tag {
                    decode_scalar_value(kind, &definition.format(tag), occurrence, bytes)?;
                }
                Ok(())
            })
        }
        (Cardinality::Repeated, kind) => {
            let format = definition.format(tag);
            let scalar = decode_scalar(kind, &format);
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
                    occurrence.scalars(scalar, &mut |value| write_number(kind, &format, value, bytes))?;
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

fn decode_scalar<R>(kind: &Kind<R>, format: &Option<prost_reflect::Kind>) -> DecodeScalar {
    match format {
        Some(prost_reflect::Kind::Fixed32 | prost_reflect::Kind::Sfixed32) => return DecodeScalar::Bits32,
        Some(prost_reflect::Kind::Fixed64 | prost_reflect::Kind::Sfixed64) => return DecodeScalar::Bits64,
        _ => {}
    }
    match kind {
        Kind::F32 => DecodeScalar::Bits32,
        Kind::F64 => DecodeScalar::Bits64,
        _ => DecodeScalar::Varint,
    }
}

fn decode_scalar_value<R>(
    kind: &Kind<R>, format: &Option<prost_reflect::Kind>, occurrence: DecodeField<'_>, bytes: &mut Vec<u8>,
) -> Result<(), MessageError> {
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
        kind => write_number(kind, format, occurrence.scalar(decode_scalar(kind, format))?, bytes),
    }
}

fn write_number<R>(
    kind: &Kind<R>, format: &Option<prost_reflect::Kind>, value: DecodeValue<'_>, bytes: &mut Vec<u8>,
) -> Result<(), MessageError> {
    let value = decoded_number(format, value);
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

fn decoded_number<'a>(format: &Option<prost_reflect::Kind>, value: DecodeValue<'a>) -> DecodeValue<'a> {
    match (format, value) {
        (Some(prost_reflect::Kind::Sint32), DecodeValue::Varint(value)) => {
            let value = value as u32;
            DecodeValue::Varint(i64::from(((value >> 1) as i32) ^ -((value & 1) as i32)) as u64)
        }
        (Some(prost_reflect::Kind::Sint64), DecodeValue::Varint(value)) => {
            DecodeValue::Varint((((value >> 1) as i64) ^ -((value & 1) as i64)) as u64)
        }
        (Some(prost_reflect::Kind::Fixed32 | prost_reflect::Kind::Sfixed32), DecodeValue::Bits32(value)) => {
            DecodeValue::Varint(u64::from(value))
        }
        (Some(prost_reflect::Kind::Fixed64 | prost_reflect::Kind::Sfixed64), DecodeValue::Bits64(value)) => DecodeValue::Varint(value),
        (_, value) => value,
    }
}

// Maps Retain Their Last Value Per Key and the Existing Canonical Key Order.
fn decode_map(definition: &impl Shape, input: DecodeInput<'_>, tag: u32, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    let key = definition.fields().iter().find(|field| field.tag == 1).expect("map key");
    let mut entries = HashMap::new();
    input.fields(&mut |occurrence| {
        if occurrence.tag != tag {
            return Ok(());
        }
        let payload = occurrence.bytes()?;
        let entry = input.child(payload)?;
        let occurrence = last(entry, key.tag)?;
        let value =
            occurrence.map(|occurrence| decoded_number(&definition.format(key.tag), occurrence.value)).unwrap_or(DecodeValue::Varint(0));
        let key = match (&key.kind, value) {
            (Kind::Bool, DecodeValue::Varint(value)) => MapKey::Bool(value != 0),
            (Kind::I32, DecodeValue::Varint(value)) => MapKey::I32(value as i32),
            (Kind::I64, DecodeValue::Varint(value)) => MapKey::I64(value as i64),
            (Kind::U32, DecodeValue::Varint(value)) => MapKey::U32(value as u32),
            (Kind::U64, DecodeValue::Varint(value)) => MapKey::U64(value),
            (Kind::String, _) => MapKey::String(occurrence.map(|occurrence| occurrence.text()).transpose()?.unwrap_or("").to_owned()),
            _ => return Err(MessageError::borrowed("supply a valid protobuf map key")),
        };
        entries.insert(key, payload);
        Ok(())
    })?;
    entries.len().write(bytes);
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by_cached_key(|(key, _)| format!("{key:?}"));
    for (_, payload) in entries {
        decode_message(definition, input.child(payload)?, bytes)?;
    }
    Ok(())
}

fn default<R>(kind: &Kind<R>, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
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
    use harmos_domain::{Message, schema::Described};
    use prost_reflect::{DescriptorPool, DynamicMessage};
    use prost_types::field_descriptor_proto::{Label, Type};
    use prost_types::{
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, MessageOptions, OneofDescriptorProto,
    };
    use std::borrow::Cow;
    use std::cell::Cell;

    fn descriptor() -> MessageDescriptor {
        let field = |name: &str, tag, kind: Type| FieldDescriptorProto {
            name: Some(name.into()),
            number: Some(tag),
            r#type: Some(kind as i32),
            label: Some(Label::Optional as i32),
            ..Default::default()
        };
        let fields = [
            ("signed32", Type::Sint32),
            ("signed64", Type::Sint64),
            ("fixed32", Type::Fixed32),
            ("fixed64", Type::Fixed64),
            ("sfixed32", Type::Sfixed32),
            ("sfixed64", Type::Sfixed64),
            ("float", Type::Float),
            ("double", Type::Double),
            ("text", Type::String),
            ("bytes", Type::Bytes),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, kind))| field(name, index as i32 + 1, kind));
        let mut fields = fields.collect::<Vec<_>>();
        fields.push(FieldDescriptorProto {
            oneof_index: Some(0),
            ..field("selected", 11, Type::Uint32)
        });
        fields.push(FieldDescriptorProto {
            oneof_index: Some(0),
            ..field("other", 12, Type::String)
        });
        fields.push(FieldDescriptorProto {
            type_name: Some(".codec.Row.EntriesEntry".into()),
            label: Some(Label::Repeated as i32),
            ..field("entries", 13, Type::Message)
        });
        fields.push(FieldDescriptorProto {
            oneof_index: Some(1),
            proto3_optional: Some(true),
            ..field("optional", 14, Type::Uint32)
        });
        fields.push(FieldDescriptorProto {
            label: Some(Label::Repeated as i32),
            ..field("values", 15, Type::Sint32)
        });
        fields.push(FieldDescriptorProto {
            oneof_index: Some(0),
            type_name: Some(".codec.Leaf".into()),
            ..field("leaf", 16, Type::Message)
        });
        let file = FileDescriptorProto {
            name: Some("codec.proto".into()),
            package: Some("codec".into()),
            syntax: Some("proto3".into()),
            message_type: vec![
                DescriptorProto {
                    name: Some("Row".into()),
                    field: fields,
                    oneof_decl: vec![
                        OneofDescriptorProto {
                            name: Some("choice".into()),
                            ..Default::default()
                        },
                        OneofDescriptorProto {
                            name: Some("_optional".into()),
                            ..Default::default()
                        },
                    ],
                    nested_type: vec![DescriptorProto {
                        name: Some("EntriesEntry".into()),
                        field: vec![
                            field("key", 1, Type::Sint32),
                            FieldDescriptorProto {
                                type_name: Some(".codec.Leaf".into()),
                                ..field("value", 2, Type::Message)
                            },
                        ],
                        options: Some(MessageOptions {
                            map_entry: Some(true),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                DescriptorProto {
                    name: Some("Leaf".into()),
                    field: vec![field("value", 1, Type::Uint32), field("other", 2, Type::Uint32)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        DescriptorPool::from_file_descriptor_set(FileDescriptorSet { file: vec![file] }).unwrap().get_message_by_name("codec.Row").unwrap()
    }

    #[test]
    fn dynamic_wire_formats_and_optional_presence_round_trip() {
        let descriptor = descriptor();
        let schema = Dynamic::schema(&descriptor).unwrap();
        let codec = Dynamic::new(descriptor.clone(), &schema);
        for optional in [None, Some(0), Some(17)] {
            let mut message = DynamicMessage::new(descriptor.clone());
            for (tag, value) in [
                (1, prost_reflect::Value::I32(i32::MIN)),
                (2, prost_reflect::Value::I64(i64::MIN)),
                (3, prost_reflect::Value::U32(u32::MAX)),
                (4, prost_reflect::Value::U64(u64::MAX)),
                (5, prost_reflect::Value::I32(-37)),
                (6, prost_reflect::Value::I64(-91)),
                (7, prost_reflect::Value::F32(-0.0)),
                (8, prost_reflect::Value::F64(-0.0)),
                (9, prost_reflect::Value::String("canonical".into())),
                (10, prost_reflect::Value::Bytes(vec![0, 255].into())),
                (11, prost_reflect::Value::U32(0)),
                (15, prost_reflect::Value::List(vec![prost_reflect::Value::I32(-1), prost_reflect::Value::I32(i32::MAX)])),
            ] {
                message.set_field_by_number(tag, value);
            }
            if let Some(value) = optional {
                message.set_field_by_number(14, prost_reflect::Value::U32(value));
            }
            let mut wire = prost::Message::encode_to_vec(&message);
            wire.extend_from_slice(&[0x3d, 0, 0, 0, 128, 0x41, 0, 0, 0, 0, 0, 0, 0, 128]);
            let canonical = codec.decode(DecodeInput::new(&wire)).unwrap();
            let mut output = Vec::with_capacity(wire.len());
            let pointer = output.as_ptr();
            codec.encode(&canonical, &mut output).unwrap();
            assert_eq!(output.as_ptr(), pointer);
            let decoded = DynamicMessage::decode(descriptor.clone(), output.as_slice()).unwrap();
            assert_eq!(decoded, message);
            assert_eq!(decoded.get_field_by_number(7).unwrap().as_f32().unwrap().to_bits(), (-0.0_f32).to_bits());
            assert_eq!(decoded.get_field_by_number(8).unwrap().as_f64().unwrap().to_bits(), (-0.0_f64).to_bits());
            let mut trailing = canonical;
            trailing.push(0);
            assert!(codec.encode(&trailing, &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn dynamic_oneof_merging_and_mixed_packing_match_protobuf() {
        let descriptor = descriptor();
        let schema = Dynamic::schema(&descriptor).unwrap();
        let codec = Dynamic::new(descriptor.clone(), &schema);
        // The First Leaf Is Replaced; the Two Leaves After the Scalar Merge.
        let wire = [0x82, 1, 2, 8, 9, 0x58, 0, 0x82, 1, 2, 16, 7, 0x82, 1, 2, 8, 3, 0x78, 1, 0x7a, 2, 0, 2];
        let canonical = codec.decode(DecodeInput::new(&wire)).unwrap();
        let mut output = Vec::new();
        codec.encode(&canonical, &mut output).unwrap();
        assert_eq!(
            DynamicMessage::decode(descriptor.clone(), output.as_slice()).unwrap(),
            DynamicMessage::decode(descriptor, wire.as_slice()).unwrap()
        );
    }

    #[test]
    fn dynamic_maps_keep_last_values_defaults_and_canonical_order() {
        let descriptor = descriptor();
        let schema = Dynamic::schema(&descriptor).unwrap();
        let codec = Dynamic::new(descriptor.clone(), &schema);
        // Zigzag Keys -1 and 1; the Duplicate -1 Replaces Its Value, and Key 0 Defaults Its Message.
        let wire = [0x6a, 6, 8, 1, 18, 2, 8, 7, 0x6a, 6, 8, 2, 18, 2, 8, 9, 0x6a, 6, 8, 1, 18, 2, 8, 3, 0x6a, 0];
        let canonical = codec.decode(DecodeInput::new(&wire)).unwrap();
        let mut output = Vec::new();
        codec.encode(&canonical, &mut output).unwrap();
        let decoded = DynamicMessage::decode(descriptor.clone(), output.as_slice()).unwrap();
        let expected = DynamicMessage::decode(descriptor, wire.as_slice()).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(codec.decode(DecodeInput::new(&output)).unwrap(), canonical);
        let mut reversed = Vec::new();
        DecodeInput::new(&output)
            .fields(&mut |field| {
                let mut entry = vec![0x6a];
                let value = field.bytes()?;
                encode_varint(value.len() as u64, &mut entry);
                entry.extend_from_slice(value);
                reversed.splice(0..0, entry);
                Ok(())
            })
            .unwrap();
        assert_eq!(codec.decode(DecodeInput::new(&reversed)).unwrap(), canonical);
    }

    #[test]
    fn dynamic_stored_maps_and_nesting_keep_structural_refusals() {
        let descriptor = descriptor();
        let prost_reflect::Kind::Message(entry) = descriptor.get_field(13).unwrap().kind() else { unreachable!() };
        let schema = Dynamic::schema(&entry).unwrap();
        let codec = Dynamic::new(entry, &schema);
        let absent = postcard::to_allocvec(&(-1_i32, Option::<()>::None)).unwrap();
        assert!(codec.encode(&absent, &mut Vec::new()).is_err());

        let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("nested.proto".into()),
                package: Some("codec".into()),
                syntax: Some("proto3".into()),
                message_type: vec![DescriptorProto {
                    name: Some("Node".into()),
                    field: vec![FieldDescriptorProto {
                        name: Some("next".into()),
                        number: Some(3),
                        r#type: Some(Type::Message as i32),
                        type_name: Some(".codec.Node".into()),
                        label: Some(Label::Optional as i32),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        })
        .unwrap();
        let descriptor = pool.get_message_by_name("codec.Node").unwrap();
        let schema = Dynamic::schema(&descriptor).unwrap();
        let codec = Dynamic::new(descriptor, &schema);
        for nested in [harmos_domain::schema::MAX_DEPTH - 1, harmos_domain::schema::MAX_DEPTH] {
            let mut wire = Vec::new();
            for _ in 0..nested {
                let mut parent = vec![0x1a];
                encode_varint(wire.len() as u64, &mut parent);
                parent.extend_from_slice(&wire);
                wire = parent;
            }
            let mut canonical = vec![1; nested];
            canonical.push(0);
            let decoded = codec.decode(DecodeInput::new(&wire));
            let mut output = Vec::new();
            let encoded = codec.encode(&canonical, &mut output);
            if nested == harmos_domain::schema::MAX_DEPTH {
                assert!(decoded.is_err());
                assert!(encoded.is_err());
            } else {
                assert_eq!(decoded.unwrap(), canonical);
                encoded.unwrap();
                assert_eq!(output, wire);
            }
        }
    }

    #[test]
    fn dynamic_decode_rejects_malformed_overwritten_values() {
        let descriptor = descriptor();
        let schema = Dynamic::schema(&descriptor).unwrap();
        let codec = Dynamic::new(descriptor, &schema);
        for wire in [
            vec![0x0d, 0, 0, 0, 0, 8, 0],
            vec![0x4a, 1, 255],
            vec![0x7a, 1, 128],
            vec![255],
            vec![0x82, 1, 3, 10, 1, 0, 0x58, 0],
            vec![0x6a, 7, 8, 1, 18, 3, 10, 1, 0, 0x6a, 2, 8, 1],
        ] {
            assert!(codec.decode(DecodeInput::new(&wire)).is_err());
        }
    }

    #[derive(harmos_domain::Message, Clone, Debug, Default)]
    #[harmos_path(::harmos_domain)]
    struct Leaf {
        value: i64,
        negative_zero: f64,
    }

    #[derive(harmos_domain::Message, Clone, Debug)]
    #[harmos_path(::harmos_domain)]
    struct Row {
        leaf: Leaf,
        values: Vec<u64>,
        optional: Option<u32>,
    }

    #[derive(harmos_domain::Message, Clone, Debug)]
    #[harmos_path(::harmos_domain)]
    #[derive(Default)]
    enum Choice {
        #[default]
        Empty,
        Row(Row),
    }

    #[derive(harmos_domain::Message, Clone, Debug)]
    #[harmos_path(::harmos_domain)]
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
        let definition = Definition { fields: || FIELDS, ..Leaf::DEFINITION };
        const FIELDS: &[Field<Definition>] = &[
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

        let refused = crate::execution::grpc::encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
            if required > baseline { Err(MessageError::borrowed("fixture index refusal")) } else { Ok(()) }
        });
        assert!(refused.is_err());
        assert!(bytes.is_empty());

        let mut claims = Vec::new();
        crate::execution::grpc::encode_admitted(&value, &mut bytes, usize::MAX, &mut |required| {
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
        let mut expected = Wide::FIELDS
            .iter()
            .flat_map(|field| std::iter::repeat_n(field.tag, if field.name == "echo" { value.echo.len() } else { 1 }))
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(tags, expected);
    }

    #[test]
    fn index_growth_refusal_retains_existing_backing() {
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
        ceiling.set(23 + requested * size_of::<FieldSpan<'_>>());
        assert!(output.reserve_spans(requested).is_err());
        assert_eq!(output.spans.capacity(), capacity);
        assert_eq!(output.spans.as_ptr(), pointer);
        assert_eq!(retained.get(), claimed);

        ceiling.set(usize::MAX);
        output.reserve_spans(requested).unwrap();
        assert!(output.spans.capacity() >= requested);
        assert_eq!(retained.get(), 23 + output.spans.capacity() * size_of::<FieldSpan<'_>>());
    }
}
