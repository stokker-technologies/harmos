/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Column Decoding Against The Previous Row Tree

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use crate::schema::{Event, PathElement, Scalar};
use crate::{Identifier, Message, PayloadEncoding, SchemaKey, StoreHistory};
use arrow_array::{BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, ListArray};
use arrow_buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use std::num::NonZeroU32;

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

/* === Enums === */

enum Node<'a> {
    Null,
    Scalar(Scalar<'a>),
    Message(Vec<(String, Node<'a>)>),
    List(Vec<Node<'a>>),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn reference_payload(schema: &StorageSchema, payloads: &[&[u8]]) -> Result<ArrayRef> {
    let settings = harmos_domain::config::SchemaSettings::default();
    let validator = schema::Validator::from_schema(&schema.schema, &settings).map_err(|_| invalid("invalid payload schema"))?;
    let mut roots = Vec::with_capacity(payloads.len());
    for bytes in payloads {
        let mut root = Node::Null;
        validator
            .visit(bytes, |path, event| {
                match event {
                    Event::BeginMessage => *node_at(&mut root, path)? = Node::Message(Vec::new()),
                    Event::BeginRepeated { length } => *node_at(&mut root, path)? = Node::List((0..length).map(|_| Node::Null).collect()),
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
    array(&DataType::Struct(payload_fields(&schema.schema)?), &refs)
}

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

#[harmos_domain::message]
struct Sample {
    x: f64,
    y: f64,
}

#[harmos_domain::message]
struct Nested {
    scalar: Sample,
    optional: Option<Sample>,
    repeated: Vec<Sample>,
    event: Choice,
    label: Option<String>,
    bytes: Vec<u8>,
    flag: bool,
    small: i32,
    large: i64,
    unsigned: u32,
    wide: u64,
    single: Option<f32>,
}

#[harmos_domain::message]
enum Choice {
    Empty,
    Value(Sample),
    Many(Vec<Sample>),
}

fn schema<T: crate::schema::Described>() -> StorageSchema {
    let settings = harmos_domain::config::SchemaSettings::default();
    let schema = graph::Schema::from_declaration(T::DEFINITION, &settings).unwrap();
    let history = StoreHistory {
        store: Identifier::new("test").unwrap(),
        epoch: Identifier::new("epoch").unwrap(),
    };
    StorageSchema {
        position: StorageCursor { history, sequence: 1 },
        key: SchemaKey {
            id: Identifier::new("sample").unwrap(),
            version: NonZeroU32::MIN,
            payload_encoding: PayloadEncoding::PostcardV1,
        },
        schema: Arc::new(schema),
    }
}

#[test]
fn column_decoder_matches_previous_tree_for_seeded_nested_payloads() {
    let mut seed = 19_u64;
    let mut sample = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        Sample {
            x: (seed >> 11) as f64,
            y: -(seed as f64),
        }
    };
    let payloads = (0..4099)
        .map(|row| {
            Nested {
                scalar: sample(),
                optional: (row % 3 == 0).then(&mut sample),
                repeated: (0..row % 7).map(|_| sample()).collect(),
                event: match row % 3 {
                    0 => Choice::Empty,
                    1 => Choice::Value(sample()),
                    _ => Choice::Many(vec![sample(), sample()]),
                },
                label: (row % 2 == 0).then(|| format!("row-{row}")),
                bytes: vec![row as u8; row % 5],
                flag: row % 2 == 0,
                small: -(row as i32),
                large: i64::MIN + row as i64,
                unsigned: u32::MAX - row as u32,
                wide: u64::MAX - row as u64,
                single: (row % 2 == 0).then_some(-(row as f32)),
            }
            .encode()
        })
        .collect::<Vec<_>>();
    let refs = payloads.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let schema = schema::<Nested>();
    let expected = reference_payload(&schema, &refs).unwrap();
    assert_eq!(payload(&schema, &refs).unwrap().to_data(), expected.to_data());
    assert_eq!(payload(&schema, &[]).unwrap().to_data(), reference_payload(&schema, &[]).unwrap().to_data());
    assert!(payload(&schema, &[&[255]]).is_err());

    use crate::query::transform::{
        FieldPath, ProjectExecution,
        project::{Project, Projection},
    };
    let input_schema = Schema::new(vec![Field::new("payload", expected.data_type().clone(), false)]);
    let input = RecordBatch::try_new(Arc::new(input_schema), vec![expected]).unwrap();
    let mut records = payloads
        .into_iter()
        .map(|payload| StoredRecord {
            position: schema.position.clone(),
            schema: schema.key.clone(),
            scope: None,
            item: Identifier::new("row").unwrap(),
            source: None,
            source_time: None,
            payload,
        })
        .collect::<Vec<_>>();
    for paths in [
        vec!["payload.scalar.x", "payload.optional.y"],
        vec!["payload.repeated", "payload.event", "payload.label"],
        vec!["payload"],
        vec!["payload.event.Empty"],
        vec!["payload.event.__variant", "payload.event.Value"],
    ] {
        let fields = paths
            .iter()
            .enumerate()
            .map(|(index, path)| Projection {
                field: FieldPath::from(*path),
                name: format!("column{index}"),
            })
            .collect();
        let project = Project { fields };
        let expected = project.apply(&input, &mut || Ok(())).unwrap();
        let actual = projected(&schema, records.iter(), &project, expected.schema()).unwrap();
        assert_eq!(actual, expected);
    }
    // Corruption In An Unselected Field Must Still Be Rejected.
    records[0].payload.pop();
    let project = Project {
        fields: vec![Projection {
            field: FieldPath::from("payload.scalar.x"),
            name: "x".into(),
        }],
    };
    let output = Arc::new(project.output_schema(&input.schema()).unwrap());
    assert!(projected(&schema, records.iter(), &project, output).is_err());
}

#[test]
#[ignore = "Explicit Payload Decode Timing Diagnostic"]
fn column_decoder_timings() {
    let schema = schema::<Sample>();
    for rows in [40_000_u64, 1_000_000] {
        let payloads = (0..rows)
            .map(|row| {
                Sample {
                    x: row as f64,
                    y: (row * 7919 % 65521) as f64,
                }
                .encode()
            })
            .collect::<Vec<_>>();
        let mut elapsed = Vec::new();
        for decode in [reference_payload, payload] {
            let started = std::time::Instant::now();
            for chunk in payloads.chunks(16_384) {
                let refs = chunk.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let array = decode(&schema, &refs).unwrap();
                std::hint::black_box(array);
            }
            elapsed.push(started.elapsed());
        }
        eprintln!("column_decode rows={rows} before={:?} after={:?}", elapsed[0], elapsed[1]);
    }
}
