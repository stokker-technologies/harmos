/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use prost::encoding::{WireType, encode_key, encode_varint};

use harmos_domain::{MemorySize, MessageError, message, schema::Described};
use harmos_sdk::grpc::{self, DecodeInput, prost};

// A Consumer Domain Alias Must Never Bind Generated Codec Signatures
#[allow(dead_code, reason = "consumer alias remains a macro name-resolution fixture")]
type Result<T> = ::std::result::Result<T, MessageError>;

// Manual Codec Without Default Or Prost Message
#[derive(Clone, Debug, PartialEq)]
struct Native {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
struct Wire {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Required {
    row: Native,
    optional: Option<Native>,
    rows: Vec<Native>,
    label: String,
}

#[message(harmos_path = ::harmos_domain)]
struct Optional {
    row: Option<Native>,
    rows: Vec<Native>,
}

#[message(skip_debug, harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Secret {
    secrets_json: String,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
enum Choice {
    Selected { row: Native },
    Empty,
}

#[message(harmos_path = ::harmos_domain, skip_debug)]
#[derive(PartialEq)]
enum RedactedChoice {
    Selected { row: Native },
    Empty,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret([REDACTED])")
    }
}

impl std::fmt::Debug for RedactedChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RedactedChoice([REDACTED])")
    }
}

harmos_domain::postcard_product!(Native { value: u64, payload: Vec<u8> }, validate = |value: &Native| {
    if value.value == 0 || value.payload.is_empty() {
        return Err(MessageError::borrowed("native value and payload must be present"));
    }
    Ok(())
});

impl MemorySize for Native {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.payload.capacity()
    }
}

impl Described for Native {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Native));
    const FIELDS: &'static [harmos_domain::schema::Field] = Wire::FIELDS;
    const MARKER: &'static [u8] = &harmos_domain::schema::declaration::encode::<
        { harmos_domain::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

fn frame(tag: u32, payload: &[u8], bytes: &mut Vec<u8>) {
    encode_key(tag, WireType::LengthDelimited, bytes);
    encode_varint(payload.len() as u64, bytes);
    bytes.extend_from_slice(payload);
}

fn native(value: u64) -> Native {
    Native { value, payload: vec![1, 255] }
}

#[test]
fn custom_codec_default() {
    let value = native(7);
    let expected = crate::encode(&value);
    let mut bytes = vec![255];
    grpc::encode(&(value), &mut bytes).unwrap();
    assert_eq!(bytes[0], 255);
    assert_eq!(&bytes[1..], expected);
    assert_eq!(grpc::decode::<Native>(DecodeInput::new(&bytes[1..])).unwrap(), value);
    assert!(grpc::decode::<Native>(DecodeInput::new(&[])).is_err());
}

#[test]
fn fields_keep_types() {
    let value = Required {
        row: native(7),
        optional: Some(native(8)),
        rows: vec![native(9)],
        label: "sample".into(),
    };
    let bytes = crate::encode(&value);
    assert_eq!(grpc::decode::<Required>(DecodeInput::new(&bytes)).unwrap(), value);
    assert!(grpc::decode::<Required>(DecodeInput::new(&[])).is_err());
    let empty = Optional::default();
    assert!(empty.row.is_none());
    assert!(grpc::decode::<Optional>(DecodeInput::new(&[])).unwrap().rows.is_empty());
    let selected = Choice::Selected { row: native(42) };
    let bytes = crate::encode(&selected);
    assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&bytes)).unwrap(), selected);
    assert!(grpc::decode::<Choice>(DecodeInput::new(&[])).is_err());
}

#[test]
fn fields_merge_domain() {
    let tag = Required::FIELDS.iter().find(|field| field.name == "row").unwrap().tag;
    let mut bytes = Vec::new();
    frame(tag, &crate::encode(&Wire { value: 11, payload: vec![] }), &mut bytes);
    frame(tag, &crate::encode(&Wire { value: 0, payload: vec![3] }), &mut bytes);
    let budget = grpc::prepare::<Required>(DecodeInput::new(&bytes)).map(|prepared| prepared.memory()).unwrap();
    let decoded = grpc::decode::<Required>(DecodeInput::new(&bytes)).unwrap();
    assert!(decoded.memory_size() <= budget);
    assert_eq!(decoded.row.value, 11);
    assert_eq!(decoded.row.payload, vec![3]);
    let mut invalid = Vec::new();
    frame(tag, &[], &mut invalid);
    assert!(grpc::decode::<Required>(DecodeInput::new(&invalid)).is_err());
    frame(tag, &[0xff], &mut invalid);
    assert!(grpc::decode::<Required>(DecodeInput::new(&invalid)).is_err());
}

#[test]
fn encoding_matches_protobuf() {
    let value = Required {
        row: native(7),
        optional: None,
        rows: vec![],
        label: String::new(),
    };
    let tag = Required::FIELDS.iter().find(|field| field.name == "row").unwrap().tag;
    let mut expected = Vec::new();
    frame(tag, &crate::encode(&Wire { value: 7, payload: vec![1, 255] }), &mut expected);
    assert_eq!(crate::encode(&value), expected);
}

#[test]
fn debug_preserves_secret() {
    let secret = Secret { secrets_json: "private-token".into() };
    assert_eq!(format!("{secret:?}"), "Secret([REDACTED])");
    let mut previous = Vec::new();
    prost::encoding::string::encode(424_980_157, &secret.secrets_json, &mut previous);
    assert_eq!(Secret::FIELDS[0].tag, 424_980_157);
    assert_eq!(crate::encode(&secret), previous);
    let bound = grpc::prepare::<Secret>(DecodeInput::new(&previous)).map(|prepared| prepared.memory()).unwrap();
    let decoded = grpc::decode::<Secret>(DecodeInput::new(&previous)).unwrap();
    assert_eq!(decoded, secret);
    assert!(decoded.memory_size() <= bound);
}

#[test]
fn debug_preserves_wire() {
    let original = Choice::Selected {
        row: Native {
            value: 7,
            payload: b"private-token".to_vec(),
        },
    };
    let bytes = crate::encode(&original);
    let redacted = grpc::decode::<RedactedChoice>(DecodeInput::new(&bytes)).unwrap();
    assert_eq!(format!("{redacted:?}"), "RedactedChoice([REDACTED])");
    assert_eq!(crate::encode(&redacted), bytes);
    let settings = harmos_domain::config::SchemaSettings::default();
    let redacted_schema = harmos_domain::schema::graph::Schema::from_declaration(RedactedChoice::DEFINITION, &settings).unwrap();
    let original_schema = harmos_domain::schema::graph::Schema::from_declaration(Choice::DEFINITION, &settings).unwrap();
    assert!(redacted_schema.equivalent(redacted_schema.root, &original_schema, original_schema.root, &settings).unwrap());
    assert!(redacted.memory_size() <= grpc::prepare::<RedactedChoice>(DecodeInput::new(&bytes)).map(|prepared| prepared.memory()).unwrap());
}

#[test]
fn composition_preserves_domain() {
    let value = Required {
        row: native(7),
        optional: Some(native(8)),
        rows: vec![native(9)],
        label: "postcard".into(),
    };
    let encoded = harmos_domain::Message::encode(&value);
    let bound = <Required as harmos_domain::Message>::decoding_memory(&encoded).unwrap();
    let decoded = <Required as harmos_domain::Message>::decode(&encoded).unwrap();
    assert_eq!(decoded, value);
    assert!(decoded.memory_size() <= bound);
    let invalid = Wire { value: 0, payload: vec![1] };
    let encoded = harmos_domain::Message::encode(&invalid);
    assert!(<Native as harmos_domain::Message>::decode(&encoded).is_err());
}
