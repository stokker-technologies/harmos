/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/scenarios.rs"]
mod scenario_runtime;

mod aggregate;
mod allocation;
mod boxed;
mod enums;
mod graph;
mod host;
mod native;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::{MemorySize, Protobuf, Storable, message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Original {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Reordered {
    payload: Vec<u8>,
    value: u64,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Added {
    extra: String,
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Renamed {
    renamed: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq, Default)]
struct Nested {
    row: Original,
    optional: Option<Original>,
    count: Option<u32>,
    rows: Vec<Original>,
    readings: Vec<f64>,
    signed: i64,
    number: f32,
    enabled: bool,
    r#type: String,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct QualifiedBytes {
    bytes: Vec<std::primitive::u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct DecoderNames {
    input: u64,
    field: String,
    owned: Vec<u8>,
    layout: Option<Original>,
    value: Vec<u32>,
    __harmos_input: bool,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Choices {
    selected: Choice,
    optional: Option<Choice>,
    history: Vec<Choice>,
}

/* === Enums === */
#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum Choice {
    Empty,
    Text(String),
    Sample {
        enabled: bool,
        row: Original,
        bytes: Vec<u8>,
        values: Vec<i64>,
        note: Option<String>,
    },
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum ReorderedChoice {
    Sample {
        note: Option<String>,
        values: Vec<i64>,
        bytes: Vec<u8>,
        row: Original,
        enabled: bool,
    },
    Text(String),
    Empty,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, Default, PartialEq)]
enum ExplicitDefault {
    Other(String),
    #[default]
    Selected,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum MergedChoice {
    Empty,
    Pair { left: String, right: String },
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
enum SingleVariant {
    Named { r#type: String },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn protobuf_fields(
    definition: harmos_domain::schema::declaration::Definition,
) -> harmos_domain::schema::graph::Schema {
    let mut schema = harmos_domain::schema::graph::Schema::from_declaration(
        definition,
        &harmos_domain::config::SchemaSettings::default(),
    )
    .unwrap();
    for fields in &mut schema.definitions {
        fields.sort_by_key(|field| field.tag);
    }
    schema
}

#[cfg(feature = "reflection")]
fn schema_descriptor(
    definition: harmos_domain::schema::declaration::Definition,
) -> harmos_domain::schema::reflection::MessageDescriptor {
    let settings = harmos_domain::config::SchemaSettings::default();
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(definition, &settings).unwrap();
    harmos_domain::schema::reflection::descriptor_schema(&graph, "harmos.stream", &settings)
        .unwrap()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    graph::check();
    boxed::check();
    enums::check();
    aggregate::check();
    {
        scenario_runtime::check("names_never_shadow", || {
            let value = DecoderNames {
                input: 7,
                field: "named".into(),
                owned: vec![255],
                layout: Some(Original {
                    value: 9,
                    payload: vec![1],
                }),
                value: vec![1, 128],
                __harmos_input: true,
            };
            let encoded = value.encode_to_vec();
            let budget = DecoderNames::decoding_memory(&encoded).unwrap();
            let decoded = DecoderNames::decode(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert!(decoded.memory_size() <= budget);
        });
    }
    {
        scenario_runtime::check("names_survive_reordering", || {
            let original = Original {
                value: u64::MAX,
                payload: vec![0, 128, 255],
            };
            let bytes = original.encode_to_vec();
            let reordered = Reordered::decode(&bytes).unwrap();
            assert_eq!(reordered.value, original.value);
            assert_eq!(reordered.payload, original.payload);
            assert_eq!(reordered.encode_to_vec(), bytes);
            let added = Added::decode(&bytes).unwrap();
            assert!(added.extra.is_empty());
            assert_eq!(
                Original::decode(
                    &Added {
                        extra: "ignored".into(),
                        ..added
                    }
                    .encode_to_vec()
                )
                .unwrap(),
                original
            );
            let renamed = Renamed::decode(&bytes).unwrap();
            assert_eq!(renamed.renamed, 0, "a rename loses the old wire identity");
            assert_eq!(renamed.payload, original.payload);
        });
    }
    {
        scenario_runtime::check("nested_values_defaults", || {
            let empty = Nested::decode(&[]).unwrap();
            assert_eq!(empty.row.value, 0);
            assert_eq!(empty.optional, None);
            assert_eq!(empty.count, None);
            assert!(empty.rows.is_empty());
            let value = Nested {
                row: Original {
                    value: u64::MAX,
                    payload: vec![0, 255],
                },
                optional: Some(Original::default()),
                count: Some(0),
                rows: vec![Original::default()],
                readings: vec![-1.0, f64::INFINITY],
                signed: i64::MIN,
                number: f32::NEG_INFINITY,
                enabled: true,
                r#type: "probe".into(),
            };
            let bytes = value.encode_to_vec();
            assert_eq!(bytes.len(), value.encoded_len());
            assert_eq!(Nested::decode(&bytes).unwrap(), value);
            assert!(Nested::decode(&[0xff]).is_err());
            assert_eq!(
                String::decode(&"hello".to_owned().encode_to_vec()).unwrap(),
                "hello"
            );
            <()>::decode(&[]).unwrap();
        });
    }
    {
        scenario_runtime::check("bytes_packed_numbers", || {
            #[message(harmos_path = ::harmos_domain)]
            struct Bytes {
                payload: Vec<u8>,
            }
            #[message(harmos_path = ::harmos_domain)]
            struct Numbers {
                payload: Vec<u32>,
            }
            let bytes = Bytes {
                payload: vec![1, 2, 127],
            }
            .encode_to_vec();
            assert_eq!(
                bytes,
                Numbers {
                    payload: vec![1, 2, 127]
                }
                .encode_to_vec()
            );
            assert_eq!(
                bytes.len(),
                9,
                "five-byte tag, one-byte length, three raw bytes"
            );
            assert_eq!(&bytes[5..], &[3, 1, 2, 127]);
        });
    }
    {
        scenario_runtime::check("schemas_preserve_wire", || {
            use harmos_domain::schema::{Cardinality, Described, Kind};
            assert_ne!(Original::FIELDS, Reordered::FIELDS);
            let left = protobuf_fields(Original::DEFINITION);
            let right = protobuf_fields(Reordered::DEFINITION);
            assert!(
                left.equivalent(
                    left.root,
                    &right,
                    right.root,
                    &harmos_domain::config::SchemaSettings::default()
                )
                .unwrap()
            );
            let value = Original::FIELDS
                .iter()
                .find(|field| field.name == "value")
                .unwrap();
            assert_eq!(value.tag, 39_772_037);
            assert_eq!(value.kind, Kind::U64);
            let payload = Original::FIELDS
                .iter()
                .find(|field| field.name == "payload")
                .unwrap();
            assert_eq!(payload.kind, Kind::Bytes);
            for (name, cardinality) in [
                ("row", Cardinality::Required),
                ("optional", Cardinality::Optional),
                ("rows", Cardinality::Repeated),
            ] {
                let field = Nested::FIELDS
                    .iter()
                    .find(|field| field.name == name)
                    .unwrap();
                assert_eq!(field.cardinality, cardinality);
                let Kind::Message { definition } = &field.kind else {
                    panic!("nested message descriptor")
                };
                assert_eq!((definition.identity)(), std::any::TypeId::of::<Original>());
                assert_eq!((definition.fields)(), Original::FIELDS);
            }
            let graph = harmos_domain::schema::graph::Schema::from_declaration(
                Nested::DEFINITION,
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            let encoded = serde_json::to_vec(&graph).unwrap();
            let owned: harmos_domain::schema::graph::Schema =
                serde_json::from_slice(&encoded).unwrap();
            assert_eq!(owned, graph);
        });
    }
    {
        scenario_runtime::check("fields_keep_binary", || {
            use harmos_domain::schema::{Described, Kind};
            assert_eq!(QualifiedBytes::FIELDS[0].kind, Kind::Bytes);
            let value = QualifiedBytes {
                bytes: vec![0, 255],
            };
            assert_eq!(
                QualifiedBytes::decode(&value.encode_to_vec()).unwrap(),
                value
            );
        });
    }
    {
        scenario_runtime::check("sizes_count_capacity", || {
            let mut payload = Vec::with_capacity(64);
            payload.extend_from_slice(&[1, 2, 3]);
            let row = Original { value: 7, payload };
            let row_heap = row.payload.capacity();
            let mut rows = Vec::with_capacity(4);
            rows.push(row.clone());
            let rows_heap =
                rows.capacity() * std::mem::size_of::<Original>() + rows[0].payload.capacity();
            let mut name = String::with_capacity(80);
            name.push_str("sample");
            let mut value = Nested {
                row,
                optional: Some(Original {
                    value: 9,
                    payload: Vec::with_capacity(32),
                }),
                rows,
                r#type: name,
                ..Default::default()
            };
            let expected = std::mem::size_of::<Nested>() + row_heap + 32 + rows_heap + 80;
            assert_eq!(value.memory_size(), expected);
            assert_eq!(
                value.encoded_size(),
                harmos_domain::Message::encode_to_vec(&value).len()
            );

            value.optional = None;
            assert_eq!(value.memory_size(), expected - 32);
            assert_eq!(
                value.encoded_size(),
                harmos_domain::Message::encode_to_vec(&value).len()
            );
            assert!(value.memory_size() > value.encoded_size());
        });
    }
    {
        scenario_runtime::check("allocation_never_small", || {
            struct Overflow;
            impl MemorySize for Overflow {
                fn memory_size(&self) -> usize {
                    usize::MAX
                }
            }
            assert_eq!(Some(Overflow).memory_size(), usize::MAX);
            assert_eq!(vec![Overflow].memory_size(), usize::MAX);
            assert_eq!(
                None::<Overflow>.memory_size(),
                std::mem::size_of::<Option<Overflow>>()
            );
        });
    }
    {
        scenario_runtime::check("enum_shapes_sizes", || {
            use harmos_domain::schema::{Cardinality, Described};
            let choices = [
                Choice::Empty,
                Choice::Text("value".to_owned()),
                Choice::Sample {
                    enabled: false,
                    row: Original {
                        value: 42,
                        payload: vec![0, 255],
                    },
                    bytes: vec![1, 2],
                    values: vec![i64::MIN, 0, 3],
                    note: Some(String::new()),
                },
            ];
            for value in choices {
                let encoded = value.encode_to_vec();
                assert_eq!(value.encoded_len(), encoded.len());
                assert_eq!(Choice::decode(&encoded).unwrap(), value);
                let reordered = ReorderedChoice::decode(&encoded).unwrap();
                assert_eq!(Choice::decode(&reordered.encode_to_vec()).unwrap(), value);
            }
            let nested = Choices {
                selected: Choice::Text(String::new()),
                optional: Some(Choice::Empty),
                history: vec![Choice::Empty, Choice::Text("owned".into())],
            };
            assert_eq!(Choices::decode(&nested.encode_to_vec()).unwrap(), nested);
            assert_ne!(Choice::FIELDS, ReorderedChoice::FIELDS);
            let left = protobuf_fields(Choice::DEFINITION);
            let right = protobuf_fields(ReorderedChoice::DEFINITION);
            assert!(
                left.equivalent(
                    left.root,
                    &right,
                    right.root,
                    &harmos_domain::config::SchemaSettings::default()
                )
                .unwrap()
            );
            assert!(
                Choice::FIELDS
                    .iter()
                    .all(|field| field.cardinality == Cardinality::Variant)
            );
            harmos_domain::schema::graph::Schema::from_declaration(
                Choice::DEFINITION,
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            assert_eq!(Choice::decode(&[]).unwrap(), Choice::Empty);
            assert_eq!(
                ExplicitDefault::decode(&[]).unwrap(),
                ExplicitDefault::Selected
            );
            assert!(Choice::decode(&[0xff]).is_err());
            let mut text = String::with_capacity(77);
            text.push_str("resident");
            let value = Choice::Text(text);
            assert_eq!(value.memory_size(), std::mem::size_of::<Choice>() + 77);
            assert_eq!(
                value.encoded_size(),
                harmos_domain::Message::encode_to_vec(&value).len()
            );
        });
    }
    {
        scenario_runtime::check("wire_uses_last", || {
            let mut bytes = Choice::Text("before".into()).encode_to_vec();
            bytes.extend(Choice::Empty.encode_to_vec());
            bytes.extend([8, 1]);
            assert_eq!(Choice::decode(&bytes).unwrap(), Choice::Empty);
        });
    }
    {
        scenario_runtime::check("variant_keeps_raw", || {
            use harmos_domain::schema::{Described, Kind};
            let variant = Choice::FIELDS
                .iter()
                .find(|field| field.name == "Text")
                .unwrap();
            let Kind::Message { definition } = &variant.kind else {
                panic!("variant payload");
            };
            let fields = (definition.fields)();
            let mut wrong_type = Vec::new();
            prost::encoding::uint64::encode(fields[0].tag, &7, &mut wrong_type);
            let mut bytes = Vec::new();
            prost::encoding::bytes::encode(variant.tag, &wrong_type, &mut bytes);
            assert!(Choice::decode(&bytes).is_err());
            assert!(Choice::decoding_memory(&bytes).is_err());
            bytes.extend(Choice::Empty.encode_to_vec());
            assert_eq!(Choice::decode(&bytes).unwrap(), Choice::Empty);
            assert!(Choice::decoding_memory(&bytes).unwrap() >= std::mem::size_of::<Choice>());

            let mut malformed = Vec::new();
            prost::encoding::bytes::encode(variant.tag, &vec![255], &mut malformed);
            malformed.extend(Choice::Empty.encode_to_vec());
            assert!(Choice::decode(&malformed).is_err());
            assert!(Choice::decoding_memory(&malformed).is_err());
        });
    }
    {
        scenario_runtime::check("declared_unit_default", || {
            let budget = ExplicitDefault::decoding_memory(&[]).unwrap();
            let value = ExplicitDefault::decode(&[]).unwrap();
            assert_eq!(value, ExplicitDefault::Selected);
            assert_eq!(budget, std::mem::size_of::<ExplicitDefault>());
            assert_eq!(value.memory_size(), budget);
        });
    }
    #[cfg(feature = "reflection")]
    {
        scenario_runtime::check("enums_use_oneof", || {
            use harmos_domain::schema::{Described, reflection::DynamicMessage};
            let schema = schema_descriptor(Choice::DEFINITION);
            assert_eq!(schema.oneofs().count(), 1);
            let value = Choice::Text("reflected".into());
            let reflected =
                DynamicMessage::decode(schema, value.encode_to_vec().as_slice()).unwrap();
            let bytes = prost::Message::encode_to_vec(&reflected);
            assert_eq!(Choice::decode(&bytes).unwrap(), value);
        });
    }
    #[cfg(feature = "reflection")]
    {
        scenario_runtime::check("composites_match_canonical", || {
            use harmos_domain::schema::{Described, reflection::DynamicMessage};
            fn canonical<T: Protobuf + Described>(value: &T) {
                let encoded = Protobuf::encode_to_vec(value);
                let reflected =
                    DynamicMessage::decode(schema_descriptor(T::DEFINITION), encoded.as_slice())
                        .unwrap();
                assert_eq!(encoded, prost::Message::encode_to_vec(&reflected));
                assert_eq!(encoded.len(), Protobuf::encoded_len(value));
            }
            canonical(&Nested {
                row: Original {
                    value: 7,
                    payload: vec![255],
                },
                optional: Some(Original {
                    value: 9,
                    payload: vec![1, 2],
                }),
                count: Some(0),
                rows: vec![Original {
                    value: 13,
                    payload: vec![3],
                }],
                signed: 17,
                r#type: "typed".into(),
                ..Default::default()
            });
            for (left, right) in [("left", "right"), ("", "right"), ("left", ""), ("", "")] {
                canonical(&MergedChoice::Pair {
                    left: left.into(),
                    right: right.into(),
                });
            }
        });
    }
    {
        scenario_runtime::check("variant_merge_payload", || {
            use harmos_domain::schema::{Described, Kind};
            let variant = MergedChoice::FIELDS
                .iter()
                .find(|field| field.name == "Pair")
                .unwrap();
            let Kind::Message { definition } = &variant.kind else {
                unreachable!()
            };
            let fields = (definition.fields)();
            let mut wire = Vec::new();
            for (name, text) in [("left", "first"), ("right", "second")] {
                let field = fields.iter().find(|field| field.name == name).unwrap();
                let mut payload = Vec::new();
                prost::encoding::string::encode(field.tag, &text.to_owned(), &mut payload);
                prost::encoding::bytes::encode(variant.tag, &payload, &mut wire);
            }
            assert_eq!(
                MergedChoice::decode(&wire).unwrap(),
                MergedChoice::Pair {
                    left: "first".into(),
                    right: "second".into()
                }
            );
        });
    }
    {
        scenario_runtime::check("variants_preserve_raw", || {
            let value = SingleVariant::Named {
                r#type: "raw".into(),
            };
            assert_eq!(
                SingleVariant::decode(&value.encode_to_vec()).unwrap(),
                value
            );
            assert_eq!(
                SingleVariant::decode(&[]).unwrap(),
                SingleVariant::Named {
                    r#type: String::new()
                }
            );
        });
    }
    {
        scenario_runtime::check("validation_accepts_generated", || {
            use harmos_domain::schema::{Described, ProtobufValidator};
            let value = Choices {
                selected: Choice::Text("checked".into()),
                optional: Some(Choice::Empty),
                history: vec![Choice::Empty],
            };
            let validator = ProtobufValidator::from_schema(
                &harmos_domain::schema::graph::Schema::from_declaration(
                    Choices::DEFINITION,
                    &harmos_domain::config::SchemaSettings::default(),
                )
                .unwrap(),
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            validator.validate(&value.encode_to_vec()).unwrap();
            let validator = ProtobufValidator::from_schema(
                &harmos_domain::schema::graph::Schema::from_declaration(
                    Choice::DEFINITION,
                    &harmos_domain::config::SchemaSettings::default(),
                )
                .unwrap(),
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            validator
                .validate(&Choice::Text("checked".into()).encode_to_vec())
                .unwrap();
        });
    }
}
