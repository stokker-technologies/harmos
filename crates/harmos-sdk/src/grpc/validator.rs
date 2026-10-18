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
use std::str::Utf8Error;
use std::sync::Arc;
// Third Party Imports
use prost::encoding::{WireType, decode_key, decode_varint};
// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::{Cardinality, graph};

/* === Constants === */
const DEPTH: usize = 32;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Validator {
    root: usize,
    definitions: Arc<[Box<[Checked]>]>,
}

#[derive(Debug)]
struct Checked {
    tag: u32,
    repeated: bool,
    kind: Wire,
}

/* === Enums === */
#[derive(Debug)]
enum Wire {
    Varint,
    Bits32,
    Bits64,
    String,
    Bytes,
    Message(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayloadError {
    #[error(transparent)]
    Codec(#[from] prost::DecodeError),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error("protobuf payload exceeds 32 nested message or group levels")]
    Depth,
    #[error("invalid protobuf payload: {0}")]
    Malformed(&'static str),
}

/* === Traits === */

/* === Implementations === */
impl Validator {
    // Reuse The Checked Definitions For Another Root Without Recompiling Them.
    pub fn at(&self, root: usize) -> Result<Self, graph::Error> {
        if root >= self.definitions.len() {
            return Err(graph::Error::Reference { definition: root });
        }
        Ok(Self { root, definitions: Arc::clone(&self.definitions) })
    }
    pub fn construction_memory_bytes(schema: &graph::Schema, settings: &SchemaSettings) -> Result<usize, graph::Error> {
        schema
            .preparation(settings)?
            .bytes
            .checked_add(schema.definitions.iter().try_fold(std::mem::size_of::<Self>(), |bytes, fields| {
                fields
                    .len()
                    .checked_mul(std::mem::size_of::<Checked>())
                    .and_then(|size| size.checked_add(std::mem::size_of::<Box<[Checked]>>()))
                    .and_then(|size| bytes.checked_add(size))
                    .ok_or(graph::Error::Budget)
            })?)
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .and_then(|bytes| {
                schema
                    .definitions
                    .len()
                    .checked_mul(std::mem::size_of::<Box<[Checked]>>())
                    .and_then(|temporary| bytes.checked_add(temporary))
            })
            .ok_or(graph::Error::Budget)
    }
    pub fn validate(&self, bytes: &[u8]) -> Result<(), PayloadError> {
        self.read(self.root, bytes, 0)
    }
    pub fn from_schema(schema: &graph::Schema, settings: &SchemaSettings) -> Result<Self, graph::Error> {
        schema.prepare(settings)?;
        if Self::construction_memory_bytes(schema, settings)? > settings.preparation_bytes.get() {
            return Err(graph::Error::Budget);
        }
        let definitions = schema
            .definitions
            .iter()
            .map(|fields| {
                let mut compiled = fields
                    .iter()
                    .map(|field| Checked {
                        tag: field.tag,
                        repeated: field.cardinality == Cardinality::Repeated,
                        kind: match field.kind {
                            graph::Kind::Bool | graph::Kind::U32 | graph::Kind::U64 | graph::Kind::I32 | graph::Kind::I64 => Wire::Varint,
                            graph::Kind::F32 => Wire::Bits32,
                            graph::Kind::F64 => Wire::Bits64,
                            graph::Kind::String => Wire::String,
                            graph::Kind::Bytes => Wire::Bytes,
                            graph::Kind::Message { definition } => Wire::Message(definition),
                        },
                    })
                    .collect::<Vec<_>>();
                compiled.sort_unstable_by_key(|field| field.tag);
                compiled.into_boxed_slice()
            })
            .collect::<Box<[_]>>();
        Ok(Self { root: schema.root, definitions: definitions.into() })
    }
    fn read(&self, definition: usize, mut bytes: &[u8], depth: usize) -> Result<(), PayloadError> {
        if depth > DEPTH {
            return Err(PayloadError::Depth);
        }
        while !bytes.is_empty() {
            let (tag, wire) = decode_key(&mut bytes)?;
            let Ok(index) = self.definitions[definition].binary_search_by_key(&tag, |field| field.tag) else {
                skip(tag, wire, &mut bytes, depth)?;
                continue;
            };
            let field = &self.definitions[definition][index];
            if field.repeated && wire == WireType::LengthDelimited && matches!(field.kind, Wire::Varint | Wire::Bits32 | Wire::Bits64) {
                let mut packed = delimited(&mut bytes)?;
                while !packed.is_empty() {
                    field.kind.scalar(&mut packed)?;
                }
                continue;
            }
            field.kind.read(self, wire, &mut bytes, depth)?;
        }
        Ok(())
    }
}

impl harmos_domain::MemorySize for Validator {
    fn memory_size(&self) -> usize {
        self.definitions.iter().fold(std::mem::size_of::<Self>().saturating_add(2 * std::mem::size_of::<usize>()), |bytes, fields| {
            bytes
                .saturating_add(std::mem::size_of::<Box<[Checked]>>())
                .saturating_add(fields.len().saturating_mul(std::mem::size_of::<Checked>()))
        })
    }
}

impl Wire {
    fn scalar(&self, bytes: &mut &[u8]) -> Result<(), PayloadError> {
        match self {
            Self::Varint => {
                decode_varint(bytes)?;
            }
            Self::Bits32 => {
                take(bytes, 4)?;
            }
            Self::Bits64 => {
                take(bytes, 8)?;
            }
            _ => unreachable!("only numeric scalar fields are packed"),
        }
        Ok(())
    }
    fn read(&self, schema: &Validator, wire: WireType, bytes: &mut &[u8], depth: usize) -> Result<(), PayloadError> {
        let expected = match self {
            Self::Varint => WireType::Varint,
            Self::Bits32 => WireType::ThirtyTwoBit,
            Self::Bits64 => WireType::SixtyFourBit,
            _ => WireType::LengthDelimited,
        };
        if wire != expected {
            return Err(malformed("field wire type differs from its admitted schema"));
        }
        match self {
            Self::Varint | Self::Bits32 | Self::Bits64 => self.scalar(bytes)?,
            Self::String => {
                std::str::from_utf8(delimited(bytes)?)?;
            }
            Self::Bytes => {
                delimited(bytes)?;
            }
            Self::Message(definition) => schema.read(*definition, delimited(bytes)?, depth + 1)?,
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8], PayloadError> {
    if length > bytes.len() {
        return Err(malformed("truncated protobuf field"));
    }
    let (payload, remaining) = bytes.split_at(length);
    *bytes = remaining;
    Ok(payload)
}

fn delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], PayloadError> {
    let length = decode_varint(bytes)?;
    let length = usize::try_from(length).map_err(|_| malformed("protobuf length exceeds the host address space"))?;
    take(bytes, length)
}

fn skip(tag: u32, wire: WireType, bytes: &mut &[u8], depth: usize) -> Result<(), PayloadError> {
    match wire {
        WireType::Varint => {
            decode_varint(bytes)?;
        }
        WireType::ThirtyTwoBit => {
            take(bytes, 4)?;
        }
        WireType::SixtyFourBit => {
            take(bytes, 8)?;
        }
        WireType::LengthDelimited => {
            delimited(bytes)?;
        }
        WireType::StartGroup => {
            if depth >= DEPTH {
                return Err(PayloadError::Depth);
            }
            loop {
                let (child, wire) = decode_key(bytes)?;
                if wire == WireType::EndGroup {
                    if child != tag {
                        return Err(malformed("protobuf group end does not match its start"));
                    }
                    break;
                }
                skip(child, wire, bytes, depth + 1)?;
            }
        }
        WireType::EndGroup => return Err(malformed("unexpected protobuf group end")),
    }
    Ok(())
}

// Called Only After The Shared Structural Budget Has Passed

fn malformed(message: &'static str) -> PayloadError {
    PayloadError::Malformed(message)
}

/* === Tests === */
