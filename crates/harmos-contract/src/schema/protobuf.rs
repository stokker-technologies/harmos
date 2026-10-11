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
// Third Party Imports
use prost::encoding::{WireType, decode_key, decode_varint};
// Local Imports
use super::{Cardinality, Field, Kind};

/* === Constants === */
const DEPTH: usize = 32;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug)]
pub struct Validator {
    fields: Box<[Checked]>,
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
    Message(Validator),
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
    pub fn new(fields: &[Field]) -> Result<Self, super::Error> {
        super::validate(fields)?;
        Ok(Self::compile(fields))
    }
    pub fn construction_memory_bytes(fields: &[Field]) -> Result<usize, super::Error> {
        let validation = super::validation_memory_bytes(fields)?;
        let compiled = compilation_bytes(fields)?;
        validation.checked_add(compiled).ok_or(super::Error::Budget)
    }
    pub fn validate(&self, bytes: &[u8]) -> Result<(), PayloadError> {
        self.read(bytes, 0)
    }
    fn compile(fields: &[Field]) -> Self {
        let mut compiled = Vec::with_capacity(fields.len());
        for field in fields {
            compiled.push(Checked {
                tag: field.tag,
                repeated: field.cardinality == Cardinality::Repeated,
                kind: match &field.kind {
                    Kind::Bool | Kind::U32 | Kind::U64 | Kind::I32 | Kind::I64 => Wire::Varint,
                    Kind::F32 => Wire::Bits32,
                    Kind::F64 => Wire::Bits64,
                    Kind::String => Wire::String,
                    Kind::Bytes => Wire::Bytes,
                    Kind::Message { fields } => Wire::Message(Self::compile(fields)),
                },
            });
        }
        compiled.sort_unstable_by_key(|field| field.tag);
        Self {
            fields: compiled.into_boxed_slice(),
        }
    }
    fn read(&self, mut bytes: &[u8], depth: usize) -> Result<(), PayloadError> {
        if depth > DEPTH {
            return Err(PayloadError::Depth);
        }
        while !bytes.is_empty() {
            let (tag, wire) = decode_key(&mut bytes)?;
            let Ok(index) = self.fields.binary_search_by_key(&tag, |field| field.tag) else {
                skip(tag, wire, &mut bytes, depth)?;
                continue;
            };
            let field = &self.fields[index];
            if field.repeated
                && wire == WireType::LengthDelimited
                && matches!(field.kind, Wire::Varint | Wire::Bits32 | Wire::Bits64)
            {
                let mut packed = delimited(&mut bytes)?;
                while !packed.is_empty() {
                    field.kind.scalar(&mut packed)?;
                }
                continue;
            }
            field.kind.read(wire, &mut bytes, depth)?;
        }
        Ok(())
    }
}

impl crate::MemorySize for Validator {
    fn memory_size(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>().saturating_add(
            self.fields
                .len()
                .saturating_mul(std::mem::size_of::<Checked>()),
        );
        for field in &self.fields {
            if let Wire::Message(schema) = &field.kind {
                let owned = schema.memory_size();
                if owned == usize::MAX {
                    return usize::MAX;
                }
                bytes =
                    bytes.saturating_add(owned.saturating_sub(std::mem::size_of::<Validator>()));
            }
        }
        bytes
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
    fn read(&self, wire: WireType, bytes: &mut &[u8], depth: usize) -> Result<(), PayloadError> {
        let expected = match self {
            Self::Varint => WireType::Varint,
            Self::Bits32 => WireType::ThirtyTwoBit,
            Self::Bits64 => WireType::SixtyFourBit,
            _ => WireType::LengthDelimited,
        };
        if wire != expected {
            return Err(malformed(
                "field wire type differs from its admitted schema",
            ));
        }
        match self {
            Self::Varint | Self::Bits32 | Self::Bits64 => self.scalar(bytes)?,
            Self::String => {
                std::str::from_utf8(delimited(bytes)?)?;
            }
            Self::Bytes => {
                delimited(bytes)?;
            }
            Self::Message(schema) => schema.read(delimited(bytes)?, depth + 1)?,
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
    let length = usize::try_from(length)
        .map_err(|_| malformed("protobuf length exceeds the host address space"))?;
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
fn compilation_bytes(fields: &[Field]) -> Result<usize, super::Error> {
    let mut bytes = fields
        .len()
        .checked_mul(std::mem::size_of::<Checked>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Validator>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<Checked>>()))
        .ok_or(super::Error::Budget)?;
    for field in fields {
        if let Kind::Message { fields } = &field.kind {
            let nested = compilation_bytes(fields)?;
            // The nested Validator pointer is already inline in Checked.
            bytes = bytes
                .checked_add(nested - std::mem::size_of::<Validator>())
                .ok_or(super::Error::Budget)?;
        }
    }
    Ok(bytes)
}

fn malformed(message: &'static str) -> PayloadError {
    PayloadError::Malformed(message)
}

/* === Tests === */
