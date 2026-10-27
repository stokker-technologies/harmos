/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Schema Preparation And Recursive Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::Cardinality;
use harmos_domain::schema::Described;
use harmos_domain::schema::declaration::{self, Definition};
use harmos_domain::schema::graph::{Error, Field, Kind, Schema};
use harmos_sdk::grpc::{self, DecodeInput};

/* === Constants === */

// Complete Dokime CheckpointDocument Declaration, Source SHA-256:
// 1395b816ab4edc53a07aac298fd73c81fdccebd56325a3095d0c2b20a029017a
const CHECKPOINT: &str = include_str!("../fixtures/checkpoint-schema.json");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct RecursiveDeclaration;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_domain::schema::Described for RecursiveDeclaration {
    const NAME: &'static str = "recursive.test";
    const FIELDS: &'static [Field<Definition>] = &[Field {
        name: Cow::Borrowed("children"),
        tag: 1,
        cardinality: Cardinality::Repeated,
        kind: Kind::Message { definition: Self::DEFINITION },
    }];
    const MARKER: &'static [u8] = {
        const LENGTH: usize = declaration::encoded_len(RecursiveDeclaration::NAME, RecursiveDeclaration::FIELDS);
        static MARKER: [u8; LENGTH] = declaration::encode(RecursiveDeclaration::NAME, RecursiveDeclaration::FIELDS);
        &MARKER
    };
}

/* === Public Functions === */

#[test]
fn typed_recursive_declaration() {
    let root = RecursiveDeclaration::DEFINITION;
    // Type Identity Survives Distinct Accessor Functions
    let alias = Definition { fields: || RecursiveDeclaration::FIELDS, marker: || RecursiveDeclaration::MARKER, ..root };
    let declarations = declaration::markers(alias, &SchemaSettings::default()).unwrap();
    assert_eq!(declarations.len(), 1);
    let conflicting = Definition { identity: std::any::TypeId::of::<()>, ..root };
    assert!(matches!(
        Schema::from_declaration(conflicting, &SchemaSettings::default()),
        Err(Error::Field { reason: "schema definition name has multiple owners", .. })
    ));
    let schema = Schema::from_declaration(root, &SchemaSettings::default()).unwrap();
    let prepared = schema.prepare(&SchemaSettings::default()).unwrap();
    assert_eq!(prepared.definitions, 1);
    assert_eq!(prepared.fields, 1);
    wire_round_trip(&schema);
    let validator = harmos_domain::schema::Validator::from_schema(&schema, &SchemaSettings::default()).unwrap();
    validator.validate(&[1, 1, 0]).unwrap();
    let combined = Schema::from_declarations(&[("request", root), ("response", alias)], &SchemaSettings::default()).unwrap();
    assert_eq!(combined.definitions.len(), 2);
    wire_round_trip(&combined);
    let directions = &combined.definitions[combined.root];
    assert_eq!(directions[0].kind, directions[1].kind);
    let shared = combined.root().unwrap();
    assert_eq!(shared.message(&shared.fields()[0]).unwrap().unwrap().index(), 0);
    assert!(combined.view(combined.definitions.len()).is_err());

    let Kind::Message { definition } = directions[0].kind else { unreachable!() };
    let prepared = harmos_domain::schema::Validator::from_schema(&combined, &SchemaSettings::default()).unwrap();
    prepared.at(definition).unwrap().validate(&[1, 1, 0]).unwrap();
    assert!(prepared.at(combined.definitions.len()).is_err());
    let protobuf = grpc::Validator::from_schema(&combined, &SchemaSettings::default()).unwrap();
    protobuf.at(definition).unwrap().validate(&[10, 0]).unwrap();
    assert!(protobuf.at(combined.definitions.len()).is_err());
    let descriptor =
        harmos_packaging::proto::reflection::descriptor_root(&combined, definition, "harmos.direction", &SchemaSettings::default())
            .unwrap();
    assert_eq!(descriptor.full_name(), "harmos.direction.Row0");
    let prost_reflect::Kind::Message(child) = descriptor.fields().next().unwrap().kind() else { unreachable!() };
    assert_eq!(child.full_name(), descriptor.full_name());
    let allowance = combined.prepare(&SchemaSettings::default()).unwrap().bytes;
    let settings = SchemaSettings { preparation_bytes: NonZeroUsize::new(allowance).unwrap(), ..SchemaSettings::default() };
    assert!(harmos_packaging::proto::reflection::descriptor_root(&combined, definition, "harmos.direction", &settings).is_err());

    let markers = declaration::markers(root, &SchemaSettings::default()).unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0], RecursiveDeclaration::MARKER);
    let body = &markers[0][harmos_domain::PREFIX.len()..markers[0].len() - 1];
    let data: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(data["fields"][0]["kind"]["definition"], RecursiveDeclaration::NAME);
    assert!(data["fields"][0]["kind"].get("fields").is_none());
    let named: declaration::Named = serde_json::from_slice(body).unwrap();
    let resolved = Schema::from_named(&named.name, std::slice::from_ref(&named), &SchemaSettings::default()).unwrap();
    assert_eq!(resolved.definitions, schema.definitions);
    let resolved_directions = Schema::from_named_roots(
        &[("request", named.name.as_str()), ("response", named.name.as_str())],
        std::slice::from_ref(&named),
        &SchemaSettings::default(),
    )
    .unwrap();
    assert_eq!(resolved_directions.definitions, combined.definitions);
    assert_eq!(resolved_directions.root, combined.root);
    assert!(combined.equivalent(definition, &resolved, resolved.root, &SchemaSettings::default()).unwrap());
    let mut mutual = Schema { root: 0, definitions: vec![schema.definitions[0].clone(), schema.definitions[0].clone()] };
    mutual.definitions[0][0].kind = Kind::Message { definition: 1 };
    assert!(schema.equivalent(schema.root, &mutual, mutual.root, &SchemaSettings::default()).unwrap());
    mutual.definitions[1][0].tag = 2;
    assert!(!schema.equivalent(schema.root, &mutual, mutual.root, &SchemaSettings::default()).unwrap());

    let duplicate = Schema::from_named(&named.name, &[named.clone(), named.clone()], &SchemaSettings::default()).unwrap();
    assert_eq!(duplicate.definitions, schema.definitions);
    let mut conflicting = named.clone();
    conflicting.fields[0].tag = 2;
    assert!(Schema::from_named(&named.name, &[named.clone(), conflicting], &SchemaSettings::default()).is_err());
    let mut missing = named.clone();
    missing.fields[0].kind = Kind::Message { definition: "absent.type".into() };
    assert!(Schema::from_named(&named.name, &[missing], &SchemaSettings::default()).is_err());
    let mut malformed = data.clone();
    malformed["kind"] = "operations".into();
    assert!(serde_json::from_value::<declaration::Named>(malformed).is_err());
}

#[test]
fn shared_checkpoint_schema() {
    let schema: Schema = serde_json::from_str(CHECKPOINT).unwrap();
    // Resolve The Same Complete Contract Through Inert Named Fragments
    let named: Vec<declaration::Named> = schema
        .definitions
        .iter()
        .enumerate()
        .map(|(index, fields)| {
            let fields = fields
                .iter()
                .map(|field| Field {
                    name: field.name.clone(),
                    tag: field.tag,
                    cardinality: field.cardinality,
                    kind: match field.kind {
                        Kind::Bool => Kind::Bool,
                        Kind::U32 => Kind::U32,
                        Kind::U64 => Kind::U64,
                        Kind::I32 => Kind::I32,
                        Kind::I64 => Kind::I64,
                        Kind::F32 => Kind::F32,
                        Kind::F64 => Kind::F64,
                        Kind::String => Kind::String,
                        Kind::Bytes => Kind::Bytes,
                        Kind::Message { definition } => Kind::Message { definition: format!("fixture.{definition}") },
                    },
                })
                .collect();
            declaration::Named { name: format!("fixture.{index}"), fields }
        })
        .collect();
    let encoded: Vec<_> = named.iter().map(|declaration| serde_json::to_vec(declaration).unwrap()).collect();
    let decoded = declaration::Named::decode(encoded.iter().map(Vec::as_slice), &SchemaSettings::default()).unwrap();
    let resolved = Schema::from_named(&format!("fixture.{}", schema.root), &decoded, &SchemaSettings::default()).unwrap();
    let resolved_preparation = resolved.prepare(&SchemaSettings::default()).unwrap();
    assert_eq!(resolved_preparation.definitions, 164);
    assert_eq!(resolved_preparation.fields, 607);
    assert!(harmos_packaging::proto::reflection::descriptor_schema(&resolved, "checkpoint", &SchemaSettings::default()).is_ok());
    let constrained = SchemaSettings { preparation_bytes: NonZeroUsize::new(1).unwrap(), ..SchemaSettings::default() };
    assert!(matches!(declaration::Named::decode(encoded.iter().map(Vec::as_slice), &constrained), Err(Error::Budget)));
    let preparation = schema.prepare(&SchemaSettings::default()).unwrap();
    assert_eq!(preparation.definitions, 164);
    wire_round_trip(&schema);
    assert_eq!(preparation.fields, 607);
    assert!(preparation.bytes < 128 * 1024);
    let validator = harmos_domain::schema::Validator::from_schema(&schema, &SchemaSettings::default()).unwrap();
    assert!(harmos_domain::MemorySize::memory_size(&validator) < 64 * 1024);
    let protobuf = grpc::Validator::from_schema(&schema, &SchemaSettings::default()).unwrap();
    assert!(harmos_domain::MemorySize::memory_size(&protobuf) < 32 * 1024);
    let descriptor =
        harmos_packaging::proto::reflection::descriptor_schema(&schema, "dokime.checkpoint", &SchemaSettings::default()).unwrap();
    assert_eq!(descriptor.name(), "Row0");
    assert_eq!(descriptor.parent_file_descriptor_proto().message_type.len(), 164);
    assert_eq!(descriptor.fields().count(), 3);
    let mut expanded = Vec::new();
    let mut depths = Vec::new();
    for (index, fields) in schema.definitions.iter().enumerate() {
        let mut count = fields.len();
        let mut depth = 0;
        for field in fields {
            if let Kind::Message { definition } = field.kind {
                assert!(definition < index);
                count += expanded[definition];
                depth = depth.max(1 + depths[definition]);
            }
        }
        expanded.push(count);
        depths.push(depth);
    }
    assert_eq!(expanded[schema.root], 5583);
    assert_eq!(depths[schema.root], 18);

    let bounded = SchemaSettings { preparation_bytes: NonZeroUsize::new(preparation.bytes - 1).unwrap(), ..Default::default() };
    assert!(matches!(schema.prepare(&bounded), Err(Error::Budget)));
    let bounded = SchemaSettings { preparation_work: NonZeroUsize::new(preparation.work - 1).unwrap(), ..Default::default() };
    assert!(matches!(schema.prepare(&bounded), Err(Error::Budget)));
}

#[test]
fn large_schema_preparation() {
    let schema = Schema {
        root: 0,
        definitions: vec![
            (0..6000)
                .map(|index| Field {
                    name: format!("field_{index}").into(),
                    tag: index + 1,
                    kind: Kind::Bool,
                    cardinality: Cardinality::Required,
                })
                .collect(),
        ],
    };
    let encoded = serde_json::to_vec(&schema).unwrap();
    let restored: Schema = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored.prepare(&SchemaSettings::default()).unwrap().fields, 6000);
    let validator = harmos_domain::schema::Validator::from_schema(&restored, &SchemaSettings::default()).unwrap();
    validator.validate(&vec![0; 6000]).unwrap();
}

#[test]
fn recursive_schema_validation() {
    assert!(serde_json::from_str::<Kind>(r#"{"type":"bool","definition":0}"#).is_err());
    assert!(serde_json::from_str::<Kind>(r#"{"type":"message","definition":0,"fields":[]}"#).is_err());
    let mut schema = Schema {
        root: 0,
        definitions: vec![vec![Field {
            name: "children".into(),
            tag: 1,
            kind: Kind::Message { definition: 0 },
            cardinality: Cardinality::Repeated,
        }]],
    };
    let prepared = schema.prepare(&SchemaSettings::default()).unwrap();
    assert_eq!(prepared.definitions, 1);
    assert_eq!(prepared.fields, 1);
    let validator = harmos_domain::schema::Validator::from_schema(&schema, &SchemaSettings::default()).unwrap();
    let protobuf = grpc::Validator::from_schema(&schema, &SchemaSettings::default()).unwrap();
    protobuf.validate(&[]).unwrap();
    protobuf.validate(&[10, 2, 10, 0]).unwrap();
    assert!(protobuf.validate(&[10, 1]).is_err());
    let mut payload = Vec::new();
    for _ in 0..64 {
        let mut parent = vec![10];
        let mut length = payload.len();
        while length >= 128 {
            parent.push((length as u8 & 127) | 128);
            length >>= 7;
        }
        parent.push(length as u8);
        parent.extend_from_slice(&payload);
        payload = parent;
    }
    assert!(matches!(protobuf.validate(&payload), Err(grpc::PayloadError::Depth)));
    validator.validate(&[0]).unwrap();
    validator.validate(&[1, 1, 0]).unwrap();
    assert!(validator.validate(&[1]).is_err());
    let mut nested = vec![1; 64];
    nested.push(0);
    assert!(validator.validate(&nested).is_err());
    let descriptor =
        harmos_packaging::proto::reflection::descriptor_schema(&schema, "harmos.recursive", &SchemaSettings::default()).unwrap();
    assert_eq!(descriptor.parent_file_descriptor_proto().message_type.len(), 1);
    let nested = descriptor.fields().next().unwrap();
    let prost_reflect::Kind::Message(child) = nested.kind() else {
        panic!("recursive field must reference its message");
    };
    assert_eq!(child.full_name(), descriptor.full_name());
    let encoded = serde_json::to_vec(&schema).unwrap();
    let decoded: Schema = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.prepare(&SchemaSettings::default()).unwrap().fields, 1);
    schema.definitions[0][0].kind = Kind::Message { definition: 1 };
    assert!(matches!(schema.prepare(&SchemaSettings::default()), Err(Error::Reference { definition: 1 })));
    assert!(serde_json::from_slice::<Schema>(&serde_json::to_vec(&schema).unwrap()).is_err());
    schema.definitions[0][0].kind = Kind::Message { definition: 0 };
    schema.definitions.push(Vec::new());
    assert!(matches!(schema.prepare(&SchemaSettings::default()), Err(Error::Unreachable { definition: 1 })));
    schema.definitions.pop();
    let duplicate = schema.definitions[0][0].clone();
    schema.definitions[0].push(duplicate);
    assert!(matches!(schema.prepare(&SchemaSettings::default()), Err(Error::Field { reason: "duplicate name", .. })));
    schema.definitions[0][1].name = "other".into();
    assert!(matches!(schema.prepare(&SchemaSettings::default()), Err(Error::Field { reason: "duplicate protobuf tag", .. })));
}

/* === Private Functions === */

fn wire_round_trip(schema: &Schema) {
    let postcard = harmos_domain::Message::encode(schema);
    assert_eq!(postcard.len(), harmos_domain::Message::encoded_size(schema));
    assert!(<Schema as harmos_domain::Message>::decoding_memory(&postcard).unwrap() >= std::mem::size_of::<Schema>());
    assert_eq!(&<Schema as harmos_domain::Message>::decode(&postcard).unwrap(), schema);
    let protobuf = crate::encode(schema);
    assert!(
        grpc::prepare::<Schema>(DecodeInput::new(&protobuf)).map(|prepared| prepared.memory()).unwrap() >= std::mem::size_of::<Schema>()
    );
    assert_eq!(&grpc::decode::<Schema>(DecodeInput::new(&protobuf)).unwrap(), schema);
}

/* === Tests === */
