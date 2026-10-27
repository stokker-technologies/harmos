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
use std::sync::Arc;
// Third Party Imports
// Local Imports
use super::{Cardinality, graph};
use crate::MessageError;
use crate::config::SchemaSettings;
use crate::message::PostcardReader;

/* === Constants === */

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
    name: Box<str>,
    tag: u32,
    cardinality: Cardinality,
    kind: Wire,
}

// Each Message Contributes At Most A Variant And Payload Field; Repeated
// Indexes Consume Their Own Reader Nesting Level.
struct Path<'schema> {
    entries: [PathElement<'schema>; 2 * crate::message::postcard::MAX_DEPTH],
    len: usize,
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
    Message(usize),
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(transparent)]
pub struct PayloadError(#[from] MessageError);

/* === Traits === */

/* === Implementations === */

impl<'schema> Path<'schema> {
    fn new() -> Self {
        Self { entries: [PathElement::Index(0); 2 * crate::message::postcard::MAX_DEPTH], len: 0 }
    }

    fn push(&mut self, element: PathElement<'schema>) {
        self.entries[self.len] = element;
        self.len += 1;
    }

    fn pop(&mut self) {
        self.len -= 1;
    }
}

impl<'schema> std::ops::Deref for Path<'schema> {
    type Target = [PathElement<'schema>];

    fn deref(&self) -> &Self::Target {
        &self.entries[..self.len]
    }
}

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
            .checked_add(graph_compilation_bytes(schema)?)
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
        match self.visit(bytes, |_, _| Ok::<(), std::convert::Infallible>(())) {
            Ok(()) => Ok(()),
            Err(VisitError::Payload(error)) => Err(error),
            Err(VisitError::Consumer(error)) => match error {},
        }
    }

    pub fn visit<'schema, 'payload, E>(
        &'schema self, bytes: &'payload [u8], mut visitor: impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), E>,
    ) -> Result<(), VisitError<E>> {
        let mut input = PostcardReader::new(bytes);
        let mut path = Path::new();
        let mut failure = None;
        let result = self.read(self.root, &mut input, &mut path, &mut |path, event| {
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
    pub fn from_schema(schema: &graph::Schema, settings: &SchemaSettings) -> Result<Self, graph::Error> {
        schema.prepare(settings)?;
        if Self::construction_memory_bytes(schema, settings)? > settings.preparation_bytes.get() {
            return Err(graph::Error::Budget);
        }
        let definitions = schema
            .definitions
            .iter()
            .map(|fields| {
                fields
                    .iter()
                    .map(|field| Checked {
                        name: field.name.as_ref().into(),
                        tag: field.tag,
                        cardinality: field.cardinality,
                        kind: match field.kind {
                            graph::Kind::Bool => Wire::Bool,
                            graph::Kind::U32 => Wire::U32,
                            graph::Kind::U64 => Wire::U64,
                            graph::Kind::I32 => Wire::I32,
                            graph::Kind::I64 => Wire::I64,
                            graph::Kind::F32 => Wire::F32,
                            graph::Kind::F64 => Wire::F64,
                            graph::Kind::String => Wire::String,
                            graph::Kind::Bytes => Wire::Bytes,
                            graph::Kind::Message { definition } => Wire::Message(definition),
                        },
                    })
                    .collect::<Box<[_]>>()
            })
            .collect::<Box<[_]>>();
        Ok(Self { root: schema.root, definitions: definitions.into() })
    }
    fn read<'schema, 'payload>(
        &'schema self, definition: usize, input: &mut PostcardReader<'payload>, path: &mut Path<'schema>,
        visitor: &mut impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        input.nested(|input| {
            visitor(path, Event::BeginMessage)?;
            if self.definitions[definition].first().is_some_and(|field| field.cardinality == Cardinality::Variant) {
                let index: u32 = input.value()?;
                let field = self.definitions[definition]
                    .get(index as usize)
                    .ok_or_else(|| MessageError::borrowed("unknown Postcard enum variant"))?;
                let Wire::Message(payload) = &field.kind else { unreachable!("validated enum schema") };
                path.push(PathElement::Field { name: &field.name, tag: field.tag });
                visitor(path, Event::Variant { index })?;
                visitor(path, Event::BeginMessage)?;
                self.read_fields(*payload, input, path, visitor)?;
                visitor(path, Event::EndMessage)?;
                path.pop();
            } else {
                self.read_fields(definition, input, path, visitor)?;
            }
            visitor(path, Event::EndMessage)
        })
    }
    fn read_fields<'schema, 'payload>(
        &'schema self, definition: usize, input: &mut PostcardReader<'payload>, path: &mut Path<'schema>,
        visitor: &mut impl FnMut(&[PathElement<'schema>], Event<'payload>) -> Result<(), MessageError>,
    ) -> Result<(), MessageError> {
        for field in &self.definitions[definition] {
            path.push(PathElement::Field { name: &field.name, tag: field.tag });
            match field.cardinality {
                Cardinality::Required | Cardinality::Variant => field.kind.read(self, input, path, visitor)?,
                Cardinality::Optional => input.nested(|input| {
                    let present = input.value::<bool>()?;
                    visitor(path, Event::Optional { present })?;
                    if present {
                        field.kind.read(self, input, path, visitor)?;
                    }
                    Ok(())
                })?,
                Cardinality::Repeated => input.nested(|input| {
                    let count: usize = input.value()?;
                    input.claim(count)?;
                    visitor(path, Event::BeginRepeated { length: count })?;
                    for index in 0..count {
                        path.push(PathElement::Index(index));
                        field.kind.read(self, input, path, visitor)?;
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
        &'schema self, schema: &'schema Validator, input: &mut PostcardReader<'payload>, path: &mut Path<'schema>,
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
            Self::Message(definition) => return schema.read(*definition, input, path, visitor),
        };
        visitor(path, Event::Scalar(scalar))
    }
}
impl crate::MemorySize for Validator {
    fn memory_size(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>()
            .saturating_add(2 * std::mem::size_of::<usize>())
            .saturating_add(self.definitions.len().saturating_mul(std::mem::size_of::<Box<[Checked]>>()));
        for fields in self.definitions.iter() {
            bytes = bytes.saturating_add(fields.len().saturating_mul(std::mem::size_of::<Checked>()));
            for field in fields {
                bytes = bytes.saturating_add(field.name.len());
            }
        }
        bytes
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn graph_compilation_bytes(schema: &graph::Schema) -> Result<usize, graph::Error> {
    let mut bytes = std::mem::size_of::<Validator>()
        .checked_add(schema.definitions.len().checked_mul(std::mem::size_of::<Box<[Checked]>>()).ok_or(graph::Error::Budget)?)
        .ok_or(graph::Error::Budget)?;
    for fields in &schema.definitions {
        bytes = bytes
            .checked_add(fields.len().checked_mul(std::mem::size_of::<Checked>()).ok_or(graph::Error::Budget)?)
            .ok_or(graph::Error::Budget)?;
        for field in fields {
            bytes = bytes.checked_add(field.name.len()).ok_or(graph::Error::Budget)?;
        }
    }
    Ok(bytes)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    fn enum_chain(depth: usize) -> (Validator, Vec<u8>) {
        let mut definitions = Vec::new();
        for level in 0..depth {
            let variant = Checked { name: "variant".into(), tag: 1, cardinality: Cardinality::Variant, kind: Wire::Message(2 * level + 1) };
            let kind = if level + 1 == depth { Wire::Bool } else { Wire::Message(2 * level + 2) };
            let field = Checked { name: "value".into(), tag: 1, cardinality: Cardinality::Required, kind };
            definitions.push(vec![variant].into_boxed_slice());
            definitions.push(vec![field].into_boxed_slice());
        }
        let validator = Validator { root: 0, definitions: definitions.into() };
        let mut bytes = vec![0; depth];
        bytes.push(1);
        (validator, bytes)
    }

    #[test]
    fn deepest_enum_path_preserves_events_and_consumer_errors() {
        let depth = crate::message::postcard::MAX_DEPTH;
        let (validator, bytes) = enum_chain(depth);
        let mut longest = 0;
        validator
            .visit(&bytes, |path, event| {
                longest = longest.max(path.len());
                if let Event::Scalar(value) = event {
                    assert_eq!(value, Scalar::Bool(true));
                    assert_eq!(path.len(), 2 * depth);
                }
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(longest, 2 * depth);
        let failure = validator.visit(&bytes, |_, event| if matches!(event, Event::Scalar(_)) { Err(7) } else { Ok(()) });
        assert!(matches!(failure, Err(VisitError::Consumer(7))));
    }

    #[test]
    fn excessive_enum_depth_returns_payload_error() {
        let (validator, bytes) = enum_chain(crate::message::postcard::MAX_DEPTH + 1);
        let result = validator.visit(&bytes, |_, _| Ok::<_, ()>(()));
        assert!(matches!(result, Err(VisitError::Payload(_))));
    }

    #[test]
    fn repeated_index_uses_its_existing_nesting_level() {
        let depth = crate::message::postcard::MAX_DEPTH - 1;
        let (mut validator, mut bytes) = enum_chain(depth);
        let definitions = Arc::get_mut(&mut validator.definitions).unwrap();
        definitions.last_mut().unwrap()[0].cardinality = Cardinality::Repeated;
        bytes.insert(depth, 1);
        let mut scalars = 0;
        validator
            .visit(&bytes, |path, event| {
                if matches!(event, Event::Scalar(Scalar::Bool(true))) {
                    assert_eq!(path.len(), 2 * depth + 1);
                    assert_eq!(path.last(), Some(&PathElement::Index(0)));
                    scalars += 1;
                }
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(scalars, 1);
    }
}
