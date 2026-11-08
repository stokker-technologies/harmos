/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serde Wire Shape Proof

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use ::serde::Serialize;
use serde_json::{Value, json};

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::graph::Schema;
use harmos_domain::schema::json::JsonCodec;
use harmos_domain::{Described, Message, schema};

/* === Constants === */

/* === Macros === */

macro_rules! naming_case {
    ($name:ident, $rule:literal) => {
        #[test]
        fn $name() {
            #[derive(Serialize, Described)]
            #[harmos_path(::harmos_domain)]
            #[serde(rename_all = $rule)]
            enum Choice {
                HTTPValue,
                TwoWords { first_field: u32 },
            }
            #[derive(Serialize, Described)]
            #[harmos_path(::harmos_domain)]
            #[serde(rename_all = $rule)]
            struct Row {
                first_field: u32,
                r#type: bool,
            }
            assert_wire(&Choice::HTTPValue);
            assert_wire(&Choice::TwoWords { first_field: 7 });
            assert_wire(&Row { first_field: 3, r#type: true });
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
struct Plain {
    value: u64,
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(default, deny_unknown_fields, rename = "IgnoredContainerName")]
struct Omitted {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "error")]
    problem: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    items: Vec<u32>,

    #[serde(skip)]
    cache: std::cell::Cell<u64>,

    #[serde(default)]
    ordinary: Option<u32>,
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
struct Directional {
    #[serde(rename(serialize = "written", deserialize = "read"), alias = "legacy", deserialize_with = "unused")]
    first_field: String,

    next_field: bool,
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(transparent)]
struct Transparent {
    value: String,

    #[serde(skip)]
    marker: bool,
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
struct Unit;

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
struct NullableOmission {
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Unit>,

    #[serde(skip_serializing_if = "Option::is_none")]
    small: Option<u8>,
}

/* === Enums === */

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(rename_all = "snake_case")]
enum External {
    Unit,
    Newtype(u32),
    Tuple(u32, String),
    Named {
        first_field: bool,
    },

    #[serde(rename = "renamed")]
    Renamed,

    #[serde(skip)]
    Hidden,
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Internal {
    Unit,
    Named {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        note: Option<String>,

        value: u64,
    },
    Newtype(Plain),
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(tag = "kind", content = "identifier", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Adjacent {
    Unit,
    Newtype(String),
    Tuple(u32, bool),
    Named { first_field: u64 },
}

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(untagged)]
enum Untagged {
    Unit,
    Number(u32),
    Text(String),
    Tuple(bool, u32),
    Named { value: u64 },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn described<T: schema::Described>() -> Schema {
    Schema::from_declaration(T::DEFINITION, &SchemaSettings::default()).unwrap()
}

fn assert_wire<T: Serialize + schema::Described>(value: &T) {
    let schema = described::<T>();
    let serialized = serde_json::to_vec(value).unwrap();
    let value: Value = serde_json::from_slice(&serialized).unwrap();
    let codec = JsonCodec::new(schema.root().unwrap(), &SchemaSettings::default()).unwrap();
    let encoded = codec.encode(&value).unwrap();
    assert_eq!(codec.decode(&encoded).unwrap(), value);

    let json = serde_json::to_vec(&schema).unwrap();
    let decoded: Schema = serde_json::from_slice(&json).unwrap();
    assert_eq!(decoded, schema);
    let encoded = schema.encode();
    assert_eq!(Schema::decode(&encoded).unwrap(), schema);
    let markers = schema::declaration::markers(T::DEFINITION, &SchemaSettings::default()).unwrap();
    let prefix = harmos_domain::PREFIX;
    let fragments = markers.iter().map(|marker| &marker[prefix.len()..marker.len() - 1]);
    let named = schema::declaration::Named::decode(fragments, &SchemaSettings::default()).unwrap();
    let resolved = Schema::from_named(T::NAME, &named, &SchemaSettings::default()).unwrap();
    assert_eq!(schema::identity::revision(&resolved).unwrap(), schema::identity::revision(&schema).unwrap());
}

/* === Tests === */

naming_case!(lowercase, "lowercase");
naming_case!(uppercase, "UPPERCASE");
naming_case!(pascal_case, "PascalCase");
naming_case!(camel_case, "camelCase");
naming_case!(snake_case, "snake_case");
naming_case!(screaming_snake_case, "SCREAMING_SNAKE_CASE");
naming_case!(kebab_case, "kebab-case");
naming_case!(screaming_kebab_case, "SCREAMING-KEBAB-CASE");

#[test]
fn external_payload_styles() {
    assert_wire(&External::Unit);
    assert_wire(&External::Newtype(7));
    assert_wire(&External::Tuple(9, "payload".into()));
    assert_wire(&External::Named { first_field: true });
    assert_wire(&External::Renamed);
    assert_wire(&Directional {
        first_field: "value".into(),
        next_field: true,
    });
    let transparent = Transparent { value: "value".into(), marker: true };
    assert_wire(&transparent);
    let changed = Transparent {
        value: "value".into(),
        marker: !transparent.marker,
    };
    assert_eq!(serde_json::to_value(transparent).unwrap(), serde_json::to_value(changed).unwrap());
    assert!(serde_json::to_value(External::Hidden).is_err());
}

#[test]
fn internal_payload_styles() {
    assert_wire(&Internal::Unit);
    assert_wire(&Internal::Named { note: None, value: 7 });
    assert_wire(&Internal::Named { note: Some("note".into()), value: 7 });
    assert_wire(&Internal::Newtype(Plain { value: 9 }));
}

#[test]
fn adjacent_payload_styles() {
    assert_wire(&Adjacent::Unit);
    assert_wire(&Adjacent::Newtype("payload".into()));
    assert_wire(&Adjacent::Tuple(7, true));
    assert_wire(&Adjacent::Named { first_field: 8 });
}

#[test]
fn untagged_payload_styles() {
    assert_wire(&Untagged::Unit);
    assert_wire(&Untagged::Number(7));
    assert_wire(&Untagged::Text("payload".into()));
    assert_wire(&Untagged::Tuple(true, 7));
    assert_wire(&Untagged::Named { value: 8 });
}

#[test]
fn omission_differs_from_null_and_empty() {
    let omitted = Omitted {
        problem: None,
        items: vec![],
        cache: 7.into(),
        ordinary: None,
    };
    let before = serde_json::to_value(&omitted).unwrap();
    omitted.cache.set(omitted.cache.get() + 1);
    assert_eq!(before, serde_json::to_value(&omitted).unwrap());
    assert_wire(&omitted);
    assert_wire(&Omitted {
        problem: Some("error".into()),
        items: vec![1, 2],
        cache: 9.into(),
        ordinary: Some(3),
    });
    let schema = described::<Omitted>();
    let codec = JsonCodec::new(schema.root().unwrap(), &SchemaSettings::default()).unwrap();
    assert!(codec.encode(&json!({ "error": null, "ordinary": null })).is_err());
    assert!(codec.encode(&json!({ "items": [], "ordinary": null })).is_err());
    assert!(codec.encode(&json!({})).is_err());
    assert_wire(&NullableOmission { value: None, small: None });
    assert_wire(&NullableOmission { value: Some(Unit), small: Some(3) });
    let nullable = described::<NullableOmission>();
    let codec = JsonCodec::new(nullable.root().unwrap(), &SchemaSettings::default()).unwrap();
    let present = codec.encode(&json!({ "value": null })).unwrap();
    let absent = codec.encode(&json!({})).unwrap();
    assert_ne!(present, absent);
    assert_eq!(codec.decode(&present).unwrap(), json!({ "value": null }));
    assert_eq!(codec.decode(&absent).unwrap(), json!({}));
}

#[test]
fn renamed_variant_fields_and_maps() {
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    #[serde(tag = "type", content = "data")]
    enum Choice {
        #[serde(rename_all = "camelCase")]
        Named {
            first_field: bool,

            #[serde(rename = "wire")]
            second_field: u32,
        },
    }
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Maps {
        values: std::collections::BTreeMap<String, u32>,
    }
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    #[serde(transparent)]
    struct Map(std::collections::BTreeMap<String, u32>);
    assert_wire(&Choice::Named { first_field: true, second_field: 9 });
    let values = std::collections::BTreeMap::from([("first".to_owned(), 3), ("second".to_owned(), 7)]);
    assert_wire(&Maps { values: values.clone() });
    assert_wire(&Map(values));
}

#[test]
fn unchanged_identity_and_deserialization_attributes() {
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    #[serde(default, deny_unknown_fields, rename = "DoesNotChangeJSON")]
    struct Deserialization {
        #[serde(default, alias = "old", deserialize_with = "unused", skip_deserializing)]
        value: u64,
    }
    let plain = described::<Plain>();
    let expected = Schema {
        root: 0,
        definitions: vec![vec![schema::graph::Field {
            name: "value".into(),
            tag: schema::field_tag("value"),
            kind: schema::graph::Kind::U64,
            cardinality: schema::Cardinality::Required,
        }]],
        representations: Default::default(),
    };
    assert_eq!(schema::identity::canonical(plain.root().unwrap()).unwrap(), schema::identity::canonical(expected.root().unwrap()).unwrap());
    assert_eq!(schema::identity::revision(&plain).unwrap(), schema::identity::revision(&described::<Deserialization>()).unwrap());
    assert_ne!(
        schema::identity::revision(&described::<External>()).unwrap(),
        schema::identity::revision(&described::<Internal>()).unwrap()
    );
}

#[test]
fn wire_changes_drive_identity() {
    macro_rules! tagged {
        ($name:ident, $($attribute:tt)*) => {
            #[derive(Serialize, Described)]
            #[harmos_path(::harmos_domain)]
            #[serde($($attribute)*)]
            enum $name { Value { value: u64 } }
        };
    }
    tagged!(Kind, tag = "kind");
    tagged!(Type, tag = "type");
    tagged!(Payload, tag = "kind", content = "payload");
    tagged!(Data, tag = "kind", content = "data");
    tagged!(Bare, untagged);
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Renamed {
        #[serde(rename = "number")]
        value: u64,
    }
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Optional {
        value: Option<u32>,
    }
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Missing {
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<u32>,
    }
    assert_wire(&Kind::Value { value: 1 });
    assert_wire(&Type::Value { value: 1 });
    assert_wire(&Payload::Value { value: 1 });
    assert_wire(&Data::Value { value: 1 });
    assert_wire(&Bare::Value { value: 1 });
    assert_wire(&Renamed { value: 1 });
    assert_wire(&Optional { value: None });
    assert_wire(&Missing { value: None });
    let revision = |schema: Schema| schema::identity::revision(&schema).unwrap();
    assert_ne!(revision(described::<Kind>()), revision(described::<Type>()));
    assert_ne!(revision(described::<Payload>()), revision(described::<Data>()));
    assert_ne!(revision(described::<Kind>()), revision(described::<Bare>()));
    assert_ne!(revision(described::<Plain>()), revision(described::<Renamed>()));
    assert_ne!(revision(described::<Optional>()), revision(described::<Missing>()));
}

#[test]
fn transparent_wrappers_preserve_wire_identity() {
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    #[serde(transparent)]
    struct Text(String);
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Numbers(Vec<u32>);
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Nested(Plain);
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Original {
        text: String,
        numbers: Vec<u32>,
        nested: Plain,
        texts: Vec<String>,
        maybe: Option<String>,
    }
    #[derive(Serialize, Described)]
    #[harmos_path(::harmos_domain)]
    struct Wrapped {
        text: Text,
        numbers: Numbers,
        nested: Nested,
        texts: Vec<Text>,
        maybe: Option<Text>,
    }
    let original = Original {
        text: "text".into(),
        numbers: vec![1, 2],
        nested: Plain { value: 3 },
        texts: vec!["entry".into()],
        maybe: Some("present".into()),
    };
    let wrapped = Wrapped {
        text: Text("text".into()),
        numbers: Numbers(vec![1, 2]),
        nested: Nested(Plain { value: 3 }),
        texts: vec![Text("entry".into())],
        maybe: Some(Text("present".into())),
    };
    assert_wire(&original);
    assert_wire(&wrapped);
    assert_wire(&Text("text".into()));
    assert_wire(&Numbers(vec![1, 2]));
    assert_wire(&Nested(Plain { value: 3 }));
    assert_eq!(serde_json::to_value(original).unwrap(), serde_json::to_value(wrapped).unwrap());
    assert_eq!(schema::identity::revision(&described::<Original>()).unwrap(), schema::identity::revision(&described::<Wrapped>()).unwrap());
    assert_eq!(schema::identity::revision(&described::<Text>()).unwrap(), schema::identity::revision(&described::<String>()).unwrap());
    assert_eq!(schema::identity::revision(&described::<Nested>()).unwrap(), schema::identity::revision(&described::<Plain>()).unwrap());
}
