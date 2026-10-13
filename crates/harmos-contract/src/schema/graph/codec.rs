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
use std::borrow::Cow;
use std::mem::size_of;
// Third Party Imports

// Local Imports
use super::{Field, Kind, Schema};
use crate::config::SchemaSettings;
use crate::message::{MessageError, PostcardReader, PostcardValue, decoding_vector};
use crate::schema::declaration::{self, Definition};
use crate::schema::{Cardinality, Described, MAX_NAME_BYTES};

/* === Constants === */

/* === Macros === */
macro_rules! shape {
    ($ty:ty, $name:literal, $fields:expr) => {
        impl Described for $ty {
            const NAME: &'static str = $name;
            const FIELDS: &'static [Field<Definition>] = $fields;
            const MARKER: &'static [u8] = &declaration::encode::<
                { declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
    };
}

shape!(
    Schema,
    "harmos::schema::Graph",
    &[
        described("root", 1, declaration::Kind::U64, Cardinality::Required),
        described(
            "definitions",
            2,
            declaration::Kind::Message {
                definition: DefinitionShape::DEFINITION
            },
            Cardinality::Repeated
        ),
    ]
);
shape!(
    DefinitionShape,
    "harmos::schema::Definition",
    &[described(
        "fields",
        1,
        declaration::Kind::Message {
            definition: FieldShape::DEFINITION
        },
        Cardinality::Repeated
    ),]
);
shape!(
    FieldShape,
    "harmos::schema::Field",
    &[
        described("name", 1, declaration::Kind::String, Cardinality::Required),
        described("tag", 2, declaration::Kind::U32, Cardinality::Required),
        described("kind", 3, declaration::Kind::U32, Cardinality::Required),
        described(
            "cardinality",
            4,
            declaration::Kind::U32,
            Cardinality::Required
        ),
        described(
            "definition",
            5,
            declaration::Kind::U64,
            Cardinality::Optional
        ),
    ]
);

/* === Type Aliases === */
type Result<T> = std::result::Result<T, MessageError>;

/* === Structs === */
struct DefinitionShape;
struct FieldShape;

struct Node<'a> {
    name: &'a str,
    tag: u32,
    kind: u32,
    cardinality: u32,
    definition: Option<usize>,
}

struct Budget {
    bytes: usize,
    work: usize,
    settings: SchemaSettings,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Budget {
    fn new() -> Self {
        Self {
            bytes: size_of::<Schema>(),
            work: 0,
            settings: SchemaSettings::default(),
        }
    }

    fn charge(&mut self, bytes: usize, work: usize) -> Result<()> {
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(invalid)?;
        self.work = self.work.checked_add(work).ok_or_else(invalid)?;
        if self.bytes > self.settings.preparation_bytes.get()
            || self.work > self.settings.preparation_work.get()
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn definition(&mut self, fields: usize) -> Result<()> {
        let bits = usize::BITS as usize - fields.leading_zeros() as usize;
        let work = fields.checked_mul(2 * bits + 3).ok_or_else(invalid)?;
        self.charge(
            4 * size_of::<Vec<Field>>() + size_of::<usize>() + size_of::<bool>(),
            work + 1,
        )
    }

    fn field(&mut self, node: &Node<'_>) -> Result<()> {
        node.check()?;
        self.charge(
            4 * size_of::<Field>() + 2 * node.name.len() + size_of::<&str>() + size_of::<u32>(),
            node.name.len() + 1,
        )
    }
}

impl Node<'_> {
    fn check(&self) -> Result<()> {
        if self.name.is_empty()
            || self.name.len() > MAX_NAME_BYTES
            || self.name.chars().any(char::is_control)
            || self.tag == 0
            || self.tag > 536_870_911
            || (19_000..=19_999).contains(&self.tag)
            || self.kind > 9
            || self.cardinality > 3
            || (self.kind == 9) != self.definition.is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn owned(self) -> Result<Field> {
        self.check()?;
        let kind = match self.kind {
            0 => Kind::Bool,
            1 => Kind::U32,
            2 => Kind::U64,
            3 => Kind::I32,
            4 => Kind::I64,
            5 => Kind::F32,
            6 => Kind::F64,
            7 => Kind::String,
            8 => Kind::Bytes,
            9 => Kind::Message {
                definition: self.definition.ok_or_else(invalid)?,
            },
            _ => return Err(invalid()),
        };
        let cardinality = match self.cardinality {
            0 => Cardinality::Required,
            1 => Cardinality::Optional,
            2 => Cardinality::Repeated,
            3 => Cardinality::Variant,
            _ => return Err(invalid()),
        };
        Ok(Field {
            name: self.name.to_owned().into(),
            tag: self.tag,
            kind,
            cardinality,
        })
    }
}

impl<'a> From<&'a Field> for Node<'a> {
    fn from(field: &'a Field) -> Self {
        let (kind, definition) = match field.kind {
            Kind::Bool => (0, None),
            Kind::U32 => (1, None),
            Kind::U64 => (2, None),
            Kind::I32 => (3, None),
            Kind::I64 => (4, None),
            Kind::F32 => (5, None),
            Kind::F64 => (6, None),
            Kind::String => (7, None),
            Kind::Bytes => (8, None),
            Kind::Message { definition } => (9, Some(definition)),
        };
        let cardinality = match field.cardinality {
            Cardinality::Required => 0,
            Cardinality::Optional => 1,
            Cardinality::Repeated => 2,
            Cardinality::Variant => 3,
        };
        Self {
            name: &field.name,
            tag: field.tag,
            kind,
            cardinality,
            definition,
        }
    }
}

impl PostcardValue for Schema {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.root.write(bytes);
        self.definitions.len().write(bytes);
        for fields in &self.definitions {
            fields.len().write(bytes);
            for field in fields {
                let node = Node::from(field);
                node.name.len().write(bytes);
                bytes.extend_from_slice(node.name.as_bytes());
                node.tag.write(bytes);
                node.kind.write(bytes);
                node.cardinality.write(bytes);
                match node.definition {
                    None => 0u32.write(bytes),
                    Some(definition) => {
                        1u32.write(bytes);
                        definition.write(bytes);
                    }
                }
            }
        }
    }

    fn size(&self) -> usize {
        let mut size = self
            .root
            .size()
            .saturating_add(self.definitions.len().size());
        for fields in &self.definitions {
            size = size.saturating_add(fields.len().size());
            for field in fields {
                let node = Node::from(field);
                let reference = match node.definition {
                    None => 1,
                    Some(index) => 1usize.saturating_add(index.size()),
                };
                size = size
                    .saturating_add(node.name.len().size())
                    .saturating_add(node.name.len())
                    .saturating_add(node.tag.size())
                    .saturating_add(node.kind.size())
                    .saturating_add(node.cardinality.size())
                    .saturating_add(reference);
            }
        }
        size
    }

    fn memory(input: &mut PostcardReader<'_>) -> Result<usize> {
        input.nested(|input| {
            let root = input.value::<usize>()?;
            let count = input.value::<usize>()?;
            if root >= count {
                return Err(invalid());
            }
            input.claim(count)?;
            let mut budget = Budget::new();
            for _ in 0..count {
                let fields = input.value::<usize>()?;
                input.claim(fields)?;
                budget.definition(fields)?;
                for _ in 0..fields {
                    let node = postcard_node(input)?;
                    if node.definition.is_some_and(|reference| reference >= count) {
                        return Err(invalid());
                    }
                    budget.field(&node)?;
                }
            }
            Ok(budget.bytes)
        })
    }

    fn read(input: &mut PostcardReader<'_>) -> Result<Self> {
        Self::memory(&mut input.clone())?;
        input.nested(|input| {
            let root = input.value()?;
            let count = input.value::<usize>()?;
            input.claim(count)?;
            let mut definitions = decoding_vector(count)?;
            for _ in 0..count {
                let count = input.value::<usize>()?;
                input.claim(count)?;
                let mut fields = decoding_vector(count)?;
                for _ in 0..count {
                    fields.push(postcard_node(input)?.owned()?);
                }
                definitions.push(fields);
            }
            let schema = Self { root, definitions };
            schema
                .prepare(&SchemaSettings::default())
                .map_err(|_| invalid())?;
            Ok(schema)
        })
    }
}

#[cfg(feature = "grpc")]
impl crate::Protobuf for Schema {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(protobuf_size(self));
        <Self as crate::Protobuf>::encode_into(self, &mut bytes);
        bytes
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        use prost::encoding::uint64;
        uint64::encode(1, &(self.root as u64), bytes);
        for fields in &self.definitions {
            framed(2, definition_size(fields), bytes);
            for field in fields {
                let node = Node::from(field);
                framed(1, node_size(&node), bytes);
                framed(1, node.name.len(), bytes);
                bytes.extend_from_slice(node.name.as_bytes());
                prost::encoding::uint32::encode(2, &node.tag, bytes);
                prost::encoding::uint32::encode(3, &node.kind, bytes);
                prost::encoding::uint32::encode(4, &node.cardinality, bytes);
                if let Some(definition) = node.definition {
                    uint64::encode(5, &(definition as u64), bytes);
                }
            }
        }
    }

    fn encoded_len(&self) -> usize {
        protobuf_size(self)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize> {
        Self::decoding_memory_input(crate::message::DecodeInput::new(bytes))
    }
    fn decoding_memory_input(input: crate::message::DecodeInput<'_>) -> Result<usize> {
        let mut root = 0usize;
        let mut definitions = 0usize;
        input.fields(&mut |field| {
            if field.tag == 1 {
                root = usize::try_from(unsigned(field)?).map_err(|_| invalid())?;
            }
            if field.tag == 2 {
                definitions = definitions.checked_add(1).ok_or_else(invalid)?;
            }
            Ok(())
        })?;
        if root >= definitions {
            return Err(invalid());
        }
        let mut budget = Budget::new();
        input.fields(&mut |field| {
            if field.tag == 1 {
                let _ = usize::try_from(unsigned(field)?).map_err(|_| invalid())?;
            }
            if field.tag != 2 {
                return Ok(());
            }
            let definition = input.child(field.bytes()?)?;
            let mut count = 0usize;
            definition.fields(&mut |field| {
                if field.tag == 1 {
                    count = count.checked_add(1).ok_or_else(invalid)?;
                    let node = protobuf_node(definition.child(field.bytes()?)?)?;
                    if node
                        .definition
                        .is_some_and(|reference| reference >= definitions)
                    {
                        return Err(invalid());
                    }
                    budget.field(&node)?;
                }
                Ok(())
            })?;
            budget.definition(count)
        })?;
        Ok(budget.bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        Self::decode_input(crate::message::DecodeInput::new(bytes))
    }
    fn decode_input(input: crate::message::DecodeInput<'_>) -> Result<Self> {
        Self::decoding_memory_input(input)?;
        let mut root = 0;
        let mut count = 0;
        input.fields(&mut |field| {
            if field.tag == 2 {
                count += 1;
            }
            Ok(())
        })?;
        let mut definitions = decoding_vector(count)?;
        input.fields(&mut |field| {
            match field.tag {
                1 => root = usize::try_from(unsigned(field)?).map_err(|_| invalid())?,
                2 => {
                    let definition = input.child(field.bytes()?)?;
                    let mut count = 0;
                    definition.fields(&mut |field| {
                        if field.tag == 1 {
                            count += 1;
                        }
                        Ok(())
                    })?;
                    let mut fields = decoding_vector(count)?;
                    definition.fields(&mut |field| {
                        if field.tag == 1 {
                            fields.push(protobuf_node(definition.child(field.bytes()?)?)?.owned()?);
                        }
                        Ok(())
                    })?;
                    definitions.push(fields);
                }
                _ => {}
            }
            Ok(())
        })?;
        let schema = Self { root, definitions };
        schema
            .prepare(&SchemaSettings::default())
            .map_err(|_| invalid())?;
        Ok(schema)
    }
}

/* === Public Functions === */

/* === Private Functions === */
const fn described(
    name: &'static str,
    tag: u32,
    kind: declaration::Kind,
    cardinality: Cardinality,
) -> Field<Definition> {
    Field {
        name: Cow::Borrowed(name),
        tag,
        kind,
        cardinality,
    }
}

fn postcard_node<'a>(input: &mut PostcardReader<'a>) -> Result<Node<'a>> {
    let length = input.value::<usize>()?;
    if length > MAX_NAME_BYTES {
        return Err(invalid());
    }
    let name = std::str::from_utf8(input.take(length)?).map_err(|_| invalid())?;
    let tag = input.value()?;
    let kind = input.value()?;
    let cardinality = input.value()?;
    let definition = match input.value::<u32>()? {
        0 => None,
        1 => Some(input.value()?),
        _ => return Err(invalid()),
    };
    Ok(Node {
        name,
        tag,
        kind,
        cardinality,
        definition,
    })
}

#[cfg(feature = "grpc")]
fn protobuf_node<'a>(input: crate::message::DecodeInput<'a>) -> Result<Node<'a>> {
    let mut node = Node {
        name: "",
        tag: 0,
        kind: 0,
        cardinality: 0,
        definition: None,
    };
    input.fields(&mut |field| {
        match field.tag {
            1 => node.name = field.text()?,
            2 => node.tag = u32::try_from(unsigned(field)?).map_err(|_| invalid())?,
            3 => node.kind = u32::try_from(unsigned(field)?).map_err(|_| invalid())?,
            4 => node.cardinality = u32::try_from(unsigned(field)?).map_err(|_| invalid())?,
            5 => node.definition = Some(usize::try_from(unsigned(field)?).map_err(|_| invalid())?),
            _ => {}
        }
        Ok(())
    })?;
    node.check()?;
    Ok(node)
}

#[cfg(feature = "grpc")]
fn unsigned(field: crate::message::DecodeField<'_>) -> Result<u64> {
    let crate::message::DecodeValue::Varint(value) =
        field.scalar(crate::message::DecodeScalar::Varint)?
    else {
        return Err(invalid());
    };
    Ok(value)
}

#[cfg(feature = "grpc")]
fn framed(tag: u32, length: usize, bytes: &mut Vec<u8>) {
    prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, bytes);
    prost::encoding::encode_varint(length as u64, bytes);
}
#[cfg(feature = "grpc")]
fn frame_size(tag: u32, length: usize) -> usize {
    prost::encoding::key_len(tag)
        .saturating_add(prost::encoding::encoded_len_varint(length as u64))
        .saturating_add(length)
}
#[cfg(feature = "grpc")]
fn node_size(node: &Node<'_>) -> usize {
    let mut size = frame_size(1, node.name.len())
        .saturating_add(prost::encoding::uint32::encoded_len(2, &node.tag))
        .saturating_add(prost::encoding::uint32::encoded_len(3, &node.kind))
        .saturating_add(prost::encoding::uint32::encoded_len(4, &node.cardinality));
    if let Some(definition) = node.definition {
        size = size.saturating_add(prost::encoding::uint64::encoded_len(
            5,
            &(definition as u64),
        ));
    }
    size
}
#[cfg(feature = "grpc")]
fn definition_size(fields: &[Field]) -> usize {
    fields.iter().fold(0usize, |bytes, field| {
        bytes.saturating_add(frame_size(1, node_size(&Node::from(field))))
    })
}
#[cfg(feature = "grpc")]
fn protobuf_size(schema: &Schema) -> usize {
    let root = prost::encoding::uint64::encoded_len(1, &(schema.root as u64));
    schema.definitions.iter().fold(root, |bytes, fields| {
        bytes.saturating_add(frame_size(2, definition_size(fields)))
    })
}
fn invalid() -> MessageError {
    MessageError::borrowed("invalid finite schema graph or preparation allowance")
}

/* === Tests === */
