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
// Third Party Imports
// Local Imports
use super::{Cardinality, Field, Kind};
use crate::MessageError;
use crate::message::PostcardReader;

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
#[derive(Debug)]
pub struct Validator {
    fields: Box<[Checked]>,
}
#[derive(Debug)]
struct Checked {
    name: Box<str>,
    tag: u32,
    cardinality: Cardinality,
    kind: Wire,
}

/* === Enums === */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathElement<'schema> {
    Field { name: &'schema str, tag: u32 },
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scalar<'payload> {
    Bool(bool),
    U32(u32),
    U64(u64),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(&'payload str),
    Bytes(&'payload [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event<'payload> {
    BeginMessage,
    EndMessage,
    Optional { present: bool },
    BeginRepeated { length: usize },
    EndRepeated,
    Variant { index: u32 },
    Scalar(Scalar<'payload>),
}

#[derive(Debug, thiserror::Error)]
pub enum VisitError<E> {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("Postcard visitor failed: {0}")]
    Consumer(#[source] E),
}

#[derive(Debug)]
enum Wire {
    Bool,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    String,
    Bytes,
    Message(Validator),
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(transparent)]
pub struct PayloadError(#[from] MessageError);

/* === Traits === */
/* === Implementations === */
impl Validator {
    pub fn new(fields: &[Field]) -> Result<Self, super::Error> {
        super::validate(fields)?;
        Ok(Self::compile(fields))
    }
    pub fn construction_memory_bytes(fields: &[Field]) -> Result<usize, super::Error> {
        super::validation_memory_bytes(fields)?
            .checked_add(compilation_bytes(fields)?)
            .ok_or(super::Error::Budget)
    }
    pub fn validate(&self, bytes: &[u8]) -> Result<(), PayloadError> {
        match self.visit(bytes, |_, _| Ok::<(), std::convert::Infallible>(())) {
            Ok(()) => Ok(()),
            Err(VisitError::Payload(error)) => Err(error),
            Err(VisitError::Consumer(error)) => match error {},
        }
    }

    pub fn visit<'schema, 'payload, E>(
        &'schema self,
        bytes: &'payload [u8],
        mut visitor: impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), E>,
    ) -> Result<(), VisitError<E>> {
        let mut input = PostcardReader::new(bytes);
        let mut path = Vec::new();
        let mut failure = None;
        let result = self.read(&mut input, &mut path, &mut |path, event| {
            visitor(path, event).map_err(|error| {
                failure = Some(error);
                MessageError::borrowed("Postcard consumer failed")
            })
        });
        if let Some(error) = failure {
            return Err(VisitError::Consumer(error));
        }
        result.map_err(PayloadError::from)?;
        input.finish().map_err(PayloadError::from)?;
        Ok(())
    }
    fn compile(fields: &[Field]) -> Self {
        let fields = fields
            .iter()
            .map(|field| Checked {
                name: field.name.as_ref().into(),
                tag: field.tag,
                cardinality: field.cardinality,
                kind: match &field.kind {
                    Kind::Bool => Wire::Bool,
                    Kind::U32 => Wire::U32,
                    Kind::U64 => Wire::U64,
                    Kind::I32 => Wire::I32,
                    Kind::I64 => Wire::I64,
                    Kind::F32 => Wire::F32,
                    Kind::F64 => Wire::F64,
                    Kind::String => Wire::String,
                    Kind::Bytes => Wire::Bytes,
                    Kind::Message { fields } => Wire::Message(Self::compile(fields)),
                },
            })
            .collect();
        Self { fields }
    }
    fn read<'schema, 'payload>(
        &'schema self,
        input: &mut PostcardReader<'payload>,
        path: &mut Vec<PathElement<'schema>>,
        visitor: &mut impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        input.nested(|input| {
            visitor(path, Event::BeginMessage)?;
            if self
                .fields
                .first()
                .is_some_and(|field| field.cardinality == Cardinality::Variant)
            {
                let index: u32 = input.value()?;
                let field = self
                    .fields
                    .get(index as usize)
                    .ok_or_else(|| MessageError::borrowed("unknown Postcard enum variant"))?;
                let Wire::Message(payload) = &field.kind else {
                    unreachable!("validated enum schema")
                };
                path.push(PathElement::Field {
                    name: &field.name,
                    tag: field.tag,
                });
                visitor(path, Event::Variant { index })?;
                visitor(path, Event::BeginMessage)?;
                payload.read_fields(input, path, visitor)?;
                visitor(path, Event::EndMessage)?;
                path.pop();
            } else {
                self.read_fields(input, path, visitor)?;
            }
            visitor(path, Event::EndMessage)
        })
    }
    fn read_fields<'schema, 'payload>(
        &'schema self,
        input: &mut PostcardReader<'payload>,
        path: &mut Vec<PathElement<'schema>>,
        visitor: &mut impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        for field in &self.fields {
            path.push(PathElement::Field {
                name: &field.name,
                tag: field.tag,
            });
            match field.cardinality {
                Cardinality::Required | Cardinality::Variant => {
                    field.kind.read(input, path, visitor)?
                }
                Cardinality::Optional => input.nested(|input| {
                    let present = input.value::<bool>()?;
                    visitor(path, Event::Optional { present })?;
                    if present {
                        field.kind.read(input, path, visitor)?;
                    }
                    Ok(())
                })?,
                Cardinality::Repeated => input.nested(|input| {
                    let count: usize = input.value()?;
                    input.claim(count)?;
                    visitor(path, Event::BeginRepeated { length: count })?;
                    for index in 0..count {
                        path.push(PathElement::Index(index));
                        field.kind.read(input, path, visitor)?;
                        path.pop();
                    }
                    visitor(path, Event::EndRepeated)
                })?,
            }
            path.pop();
        }
        Ok(())
    }
}
impl Wire {
    fn read<'schema, 'payload>(
        &'schema self,
        input: &mut PostcardReader<'payload>,
        path: &mut Vec<PathElement<'schema>>,
        visitor: &mut impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        let scalar = match self {
            Self::Bool => Scalar::Bool(input.value()?),
            Self::U32 => Scalar::U32(input.value()?),
            Self::U64 => Scalar::U64(input.value()?),
            Self::I32 => Scalar::I32(input.value()?),
            Self::I64 => Scalar::I64(input.value()?),
            Self::F32 => Scalar::F32(input.value()?),
            Self::F64 => Scalar::F64(input.value()?),
            Self::String => Scalar::String(input.value()?),
            Self::Bytes => Scalar::Bytes(input.nested(|input| {
                let count = input.value()?;
                input.take(count)
            })?),
            Self::Message(message) => return message.read(input, path, visitor),
        };
        visitor(path, Event::Scalar(scalar))
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
            bytes = bytes.saturating_add(field.name.len());
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

/* === Public Functions === */
/* === Private Functions === */
fn compilation_bytes(fields: &[Field]) -> Result<usize, super::Error> {
    let mut bytes = fields
        .len()
        .checked_mul(std::mem::size_of::<Checked>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Validator>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<Checked>>()))
        .ok_or(super::Error::Budget)?;
    for field in fields {
        bytes = bytes
            .checked_add(field.name.len())
            .ok_or(super::Error::Budget)?;
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

/* === Tests === */
