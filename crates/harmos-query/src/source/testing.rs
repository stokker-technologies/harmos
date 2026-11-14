/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Test-Only Payload Decoders over One Message Declaration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroU32;

// Third Party Imports
use arrow_array::{BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, ListArray};
use arrow_buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};

// Local Imports
use super::*;
use crate::schema::Described;
use crate::{Identifier, PayloadEncoding, SchemaKey, StoreHistory};

/* === Constants === */

/* === Macros === */

macro_rules! scalar_array {
    ($values:expr, $variant:ident, $array:ty) => {{
        let values = $values
            .iter()
            .map(|value| match value {
                Some(Node::Scalar(Scalar::$variant(value))) => Ok(Some(*value)),
                None | Some(Node::Null) => Ok(None),
                _ => Err(invalid("payload value differs from declared schema")),
            })
            .collect::<Result<Vec<_>>>()?;
        Arc::new(<$array>::from(values)) as ArrayRef
    }};
}

/* === Type Aliases === */

/* === Structs === */

// Decodes postcard payloads of one message declaration into one Arrow column.
pub struct PayloadDecoder(pub(crate) StorageSchema);

/* === Enums === */

enum Node<'a> {
    Null,
    Scalar(Scalar<'a>),
    Message(Vec<(String, Node<'a>)>),
    List(Vec<Node<'a>>),
}

/* === Traits === */

/* === Implementations === */

impl PayloadDecoder {
    pub fn new<T: Described>() -> Self {
        let settings = harmos_domain::config::SchemaSettings::default();
        let schema = graph::Schema::from_declaration(T::DEFINITION, &settings).expect("valid message declaration");
        let history = StoreHistory {
            store: Identifier::new("test").expect("valid identifier"),
            epoch: Identifier::new("epoch").expect("valid identifier"),
        };
        Self(StorageSchema {
            position: StorageCursor { history, sequence: 1 },
            key: SchemaKey {
                id: Identifier::new("sample").expect("valid identifier"),
                version: NonZeroU32::MIN,
                payload_encoding: PayloadEncoding::PostcardV1,
            },
            schema: Arc::new(schema),
        })
    }

    // Decodes through the production columnar decoder.
    pub fn decode(&self, payloads: &[&[u8]]) -> Result<ArrayRef> {
        payload(&self.0, payloads)
    }

    // Decodes through the reference tree decoder that the columnar decoder must match.
    pub fn reference(&self, payloads: &[&[u8]]) -> Result<ArrayRef> {
        let settings = harmos_domain::config::SchemaSettings::default();
        let validator = schema::Validator::from_schema(&self.0.schema, &settings).map_err(|_| invalid("invalid payload schema"))?;
        let mut roots = Vec::with_capacity(payloads.len());
        for bytes in payloads {
            let mut root = Node::Null;
            validator
                .visit(bytes, |path, event| {
                    match event {
                        Event::BeginMessage => *node_at(&mut root, path)? = Node::Message(Vec::new()),
                        Event::BeginRepeated { length } => {
                            *node_at(&mut root, path)? = Node::List((0..length).map(|_| Node::Null).collect())
                        }
                        Event::Scalar(value) => *node_at(&mut root, path)? = Node::Scalar(value),
                        Event::Variant { index } => {
                            let parent = path.get(..path.len().saturating_sub(1)).ok_or_else(|| invalid("invalid enum path"))?;
                            let Node::Message(fields) = node_at(&mut root, parent)? else {
                                return Err(invalid("invalid enum parent"));
                            };
                            fields.push(("__variant".into(), Node::Scalar(Scalar::U32(index))));
                        }
                        Event::Optional { present: false } => *node_at(&mut root, path)? = Node::Null,
                        _ => {}
                    }
                    Ok::<_, Error>(())
                })
                .map_err(|_| invalid("payload decoding failed"))?;
            roots.push(root);
        }
        let refs = roots.iter().map(Some).collect::<Vec<_>>();
        array(&DataType::Struct(payload_fields(&self.0.schema)?), &refs)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn node_at<'n, 'a>(root: &'n mut Node<'a>, path: &[PathElement<'_>]) -> Result<&'n mut Node<'a>> {
    let Some((part, rest)) = path.split_first() else {
        return Ok(root);
    };
    match (root, part) {
        (Node::Message(fields), PathElement::Field { name, .. }) => {
            let index = match fields.iter().position(|(field, _)| field == name) {
                Some(index) => index,
                None => {
                    fields.push(((*name).to_owned(), Node::Null));
                    fields.len() - 1
                }
            };
            node_at(&mut fields[index].1, rest)
        }
        (Node::List(values), PathElement::Index(index)) => {
            node_at(values.get_mut(*index).ok_or_else(|| invalid("list index exceeds declaration"))?, rest)
        }
        _ => Err(invalid("payload path differs from schema")),
    }
}

fn array(kind: &DataType, values: &[Option<&Node<'_>>]) -> Result<ArrayRef> {
    Ok(match kind {
        DataType::Boolean => scalar_array!(values, Bool, BooleanArray),
        DataType::UInt32 => scalar_array!(values, U32, UInt32Array),
        DataType::UInt64 => scalar_array!(values, U64, UInt64Array),
        DataType::Int32 => scalar_array!(values, I32, Int32Array),
        DataType::Int64 => scalar_array!(values, I64, Int64Array),
        DataType::Float32 => scalar_array!(values, F32, Float32Array),
        DataType::Float64 => scalar_array!(values, F64, Float64Array),
        DataType::Utf8 => {
            let items = values
                .iter()
                .map(|value| match value {
                    Some(Node::Scalar(Scalar::String(value))) => Ok(Some(*value)),
                    None | Some(Node::Null) => Ok(None),
                    _ => Err(invalid("expected string payload")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(StringArray::from(items))
        }
        DataType::Binary => {
            let items = values
                .iter()
                .map(|value| match value {
                    Some(Node::Scalar(Scalar::Bytes(value))) => Ok(Some(*value)),
                    None | Some(Node::Null) => Ok(None),
                    _ => Err(invalid("expected binary payload")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(BinaryArray::from(items))
        }
        DataType::Struct(fields) => {
            let mut children = Vec::with_capacity(fields.len());
            for field in fields {
                let members = values
                    .iter()
                    .map(|value| match value {
                        Some(Node::Message(entries)) => Ok(entries.iter().find(|(name, _)| name == field.name()).map(|(_, value)| value)),
                        None | Some(Node::Null) => Ok(None),
                        _ => Err(invalid("expected nested message payload")),
                    })
                    .collect::<Result<Vec<_>>>()?;
                children.push(array(field.data_type(), &members)?);
            }
            let valid = values.iter().map(|value| matches!(value, Some(Node::Message(_)))).collect::<Vec<_>>();
            if fields.is_empty() {
                Arc::new(StructArray::new_empty_fields(values.len(), Some(NullBuffer::from(valid))))
            } else {
                Arc::new(
                    StructArray::try_new(fields.clone(), children, Some(NullBuffer::from(valid)))
                        .map_err(|_| invalid("nested Arrow schema mismatch"))?,
                )
            }
        }
        DataType::List(field) => {
            let mut offsets = Vec::with_capacity(values.len() + 1);
            let mut elements = Vec::new();
            let mut valid = Vec::with_capacity(values.len());
            offsets.push(0i32);
            for value in values {
                match value {
                    Some(Node::List(items)) => {
                        valid.push(true);
                        elements.extend(items.iter().map(Some));
                    }
                    None | Some(Node::Null) => valid.push(false),
                    _ => return Err(invalid("expected repeated payload")),
                }
                offsets.push(i32::try_from(elements.len()).map_err(|_| invalid("list exceeds Arrow offset range"))?);
            }
            Arc::new(
                ListArray::try_new(
                    field.clone(),
                    OffsetBuffer::new(ScalarBuffer::from(offsets)),
                    array(field.data_type(), &elements)?,
                    Some(NullBuffer::from(valid)),
                )
                .map_err(|_| invalid("list Arrow schema mismatch"))?,
            )
        }
        _ => return Err(invalid("unsupported payload type")),
    })
}

/* === Tests === */
