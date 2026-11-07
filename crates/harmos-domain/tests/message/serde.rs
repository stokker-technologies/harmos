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
use harmos_domain::{Described, Message, schema};
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::graph::Schema;
use harmos_domain::schema::json::JsonCodec;


/* === Constants === */

/* === Macros === */

macro_rules! naming_case {
    ($name:ident, $rule:literal) => {
        #[test]
        fn $name() {
            #[derive(Serialize, Described)]
            #[harmos_path(::harmos_domain)]
            #[serde(rename_all = $rule)]
            enum Choice { HTTPValue, TwoWords { first_field: u32 } }
            #[derive(Serialize, Described)]
            #[harmos_path(::harmos_domain)]
            #[serde(rename_all = $rule)]
            struct Row { first_field: u32, r#type: bool }
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

/* === Enums === */

#[derive(Serialize, Described)]
#[harmos_path(::harmos_domain)]
#[serde(rename_all = "snake_case")]
enum External {
    Unit,
    Newtype(u32),
    Tuple(u32, String),
    Named { first_field: bool },

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
    assert_wire(&Directional { first_field: "value".into(), next_field: true });
    let transparent = Transparent { value: "value".into(), marker: true };
    assert_wire(&transparent);
    let changed = Transparent { value: "value".into(), marker: !transparent.marker };
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
    let omitted = Omitted { problem: None, items: vec![], cache: 7.into(), ordinary: None };
    let before = serde_json::to_value(&omitted).unwrap();
    omitted.cache.set(omitted.cache.get() + 1);
    assert_eq!(before, serde_json::to_value(&omitted).unwrap());
    assert_wire(&omitted);
    assert_wire(&Omitted { problem: Some("error".into()), items: vec![1, 2], cache: 9.into(), ordinary: Some(3) });
    let schema = described::<Omitted>();
    let codec = JsonCodec::new(schema.root().unwrap(), &SchemaSettings::default()).unwrap();
    assert!(codec.encode(&json!({ "error": null, "ordinary": null })).is_err());
    assert!(codec.encode(&json!({ "items": [], "ordinary": null })).is_err());
    assert!(codec.encode(&json!({})).is_err());
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
    let expected = Schema { root: 0, definitions: vec![vec![schema::graph::Field {
        name: "value".into(), tag: schema::field_tag("value"), kind: schema::graph::Kind::U64, cardinality: schema::Cardinality::Required,
    }]], representations: Default::default() };
    assert_eq!(schema::identity::canonical(plain.root().unwrap()).unwrap(), schema::identity::canonical(expected.root().unwrap()).unwrap());
    assert_eq!(schema::identity::revision(&plain).unwrap(), schema::identity::revision(&described::<Deserialization>()).unwrap());
    assert_ne!(schema::identity::revision(&described::<External>()).unwrap(), schema::identity::revision(&described::<Internal>()).unwrap());
}
