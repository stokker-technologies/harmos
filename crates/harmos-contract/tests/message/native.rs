/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::cell::Cell;
// Third Party Imports
use prost::encoding::{WireType, encode_key, encode_varint};
// Local Imports
use harmos_contract::{MemorySize, MessageError, Protobuf, message, schema::Described};

/* === Constants === */

/* === Macros === */
thread_local! { static ENCODED: Cell<usize> = const { Cell::new(0) }; }

/* === Type Aliases === */
// A Consumer Domain Alias Must Never Bind Generated Codec Signatures
type Result<T> = ::std::result::Result<T, MessageError>;

/* === Structs === */
// Intentionally Neither Clone Nor Default Nor Prost Message
#[derive(Debug, PartialEq)]
struct Native {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_contract)]
struct Wire {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct Required {
    row: Native,
    optional: Option<Native>,
    rows: Vec<Native>,
    label: String,
}

#[message(harmos_path = ::harmos_contract)]
struct Optional {
    row: Option<Native>,
    rows: Vec<Native>,
}

#[message(skip_debug, harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
struct Secret {
    secrets_json: String,
}

/* === Enums === */
#[message(harmos_path = ::harmos_contract)]
#[derive(PartialEq)]
enum Choice {
    Selected { row: Native },
    Empty,
}

#[message(harmos_path = ::harmos_contract, skip_debug)]
#[derive(PartialEq)]
enum RedactedChoice {
    Selected { row: Native },
    Empty,
}

/* === Traits === */

/* === Implementations === */
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

impl Protobuf for Native {
    fn encode_to_vec(&self) -> Vec<u8> {
        ENCODED.set(ENCODED.get() + 1);
        let mut bytes = Vec::new();
        let value_tag = Wire::FIELDS
            .iter()
            .find(|field| field.name == "value")
            .unwrap()
            .tag;
        prost::encoding::uint64::encode(value_tag, &self.value, &mut bytes);
        // Find By Name Because Descriptors Are Sorted By Tag
        let tag = Wire::FIELDS
            .iter()
            .find(|field| field.name == "payload")
            .unwrap()
            .tag;
        prost::encoding::bytes::encode(tag, &self.payload, &mut bytes);
        bytes
    }
    fn encoded_len(&self) -> usize {
        let value_tag = Wire::FIELDS
            .iter()
            .find(|field| field.name == "value")
            .unwrap()
            .tag;
        let payload_tag = Wire::FIELDS
            .iter()
            .find(|field| field.name == "payload")
            .unwrap()
            .tag;
        prost::encoding::uint64::encoded_len(value_tag, &self.value)
            + prost::encoding::bytes::encoded_len(payload_tag, &self.payload)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        Self::decode_input(harmos_contract::__private::DecodeInput::new(bytes))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize> {
        Self::decoding_memory_input(harmos_contract::__private::DecodeInput::new(bytes))
    }
    fn decoding_memory_input(input: harmos_contract::__private::DecodeInput<'_>) -> Result<usize> {
        let wire = Wire::decoding_memory_input(input)?;
        harmos_contract::__private::decoding_add(std::mem::size_of::<Self>(), wire)
    }
    fn decode_input(input: harmos_contract::__private::DecodeInput<'_>) -> Result<Self> {
        let wire = Wire::decode_input(input)?;
        if wire.value == 0 || wire.payload.is_empty() {
            return Err(MessageError::borrowed(
                "native value and payload must be present",
            ));
        }
        Ok(Self {
            value: wire.value,
            payload: wire.payload,
        })
    }
}

harmos_contract::postcard_product!(Native { value: u64, payload: Vec<u8> }, validate = |value: &Native| {
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
    const FIELDS: &'static [harmos_contract::schema::Field] = Wire::FIELDS;
    const MARKER: &'static [u8] = &harmos_contract::schema::declaration::encode::<
        { harmos_contract::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

/* === Private Functions === */
fn frame(tag: u32, payload: &[u8], bytes: &mut Vec<u8>) {
    encode_key(tag, WireType::LengthDelimited, bytes);
    encode_varint(payload.len() as u64, bytes);
    bytes.extend_from_slice(payload);
}

fn native(value: u64) -> Native {
    Native {
        value,
        payload: vec![1, 255],
    }
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("custom_codec_default", || {
            let value = native(7);
            let expected = value.encode_to_vec();
            let mut bytes = vec![255];
            ENCODED.set(0);
            value.encode_into(&mut bytes);
            assert_eq!(ENCODED.get(), 1);
            assert_eq!(bytes[0], 255);
            assert_eq!(&bytes[1..], expected);
            assert_eq!(Native::decode(&bytes[1..]).unwrap(), value);
            assert!(Native::decode(&[]).is_err());
        });
    }
    {
        scenario_runtime::check("fields_keep_types", || {
            let value = Required {
                row: native(7),
                optional: Some(native(8)),
                rows: vec![native(9)],
                label: "sample".into(),
            };
            let before = ENCODED.get();
            let length = value.encoded_len();
            assert_eq!(ENCODED.get(), before);
            let bytes = value.encode_to_vec();
            assert_eq!(bytes.len(), length);
            assert_eq!(Required::decode(&bytes).unwrap(), value);
            assert!(Required::decode(&[]).is_err());
            let empty = Optional::default();
            assert!(empty.row.is_none());
            assert!(Optional::decode(&[]).unwrap().rows.is_empty());
            let selected = Choice::Selected { row: native(42) };
            let bytes = selected.encode_to_vec();
            assert_eq!(bytes.len(), selected.encoded_len());
            assert_eq!(Choice::decode(&bytes).unwrap(), selected);
            assert!(Choice::decode(&[]).is_err());
        });
    }
    {
        scenario_runtime::check("fields_merge_domain", || {
            let tag = Required::FIELDS
                .iter()
                .find(|field| field.name == "row")
                .unwrap()
                .tag;
            let mut bytes = Vec::new();
            frame(
                tag,
                &Wire {
                    value: 11,
                    payload: vec![],
                }
                .encode_to_vec(),
                &mut bytes,
            );
            frame(
                tag,
                &Wire {
                    value: 0,
                    payload: vec![3],
                }
                .encode_to_vec(),
                &mut bytes,
            );
            let budget = Required::decoding_memory(&bytes).unwrap();
            let decoded = Required::decode(&bytes).unwrap();
            assert!(decoded.memory_size() <= budget);
            assert_eq!(decoded.row.value, 11);
            assert_eq!(decoded.row.payload, vec![3]);
            let mut invalid = Vec::new();
            frame(tag, &[], &mut invalid);
            assert!(Required::decode(&invalid).is_err());
            frame(tag, &[0xff], &mut invalid);
            assert!(Required::decode(&invalid).is_err());
        });
    }
    {
        scenario_runtime::check("encoding_matches_protobuf", || {
            let value = Required {
                row: native(7),
                optional: None,
                rows: vec![],
                label: String::new(),
            };
            let tag = Required::FIELDS
                .iter()
                .find(|field| field.name == "row")
                .unwrap()
                .tag;
            let mut expected = Vec::new();
            frame(
                tag,
                &Wire {
                    value: 7,
                    payload: vec![1, 255],
                }
                .encode_to_vec(),
                &mut expected,
            );
            assert_eq!(value.encode_to_vec(), expected);
        });
    }
    {
        scenario_runtime::check("debug_preserves_secret", || {
            let secret = Secret {
                secrets_json: "private-token".into(),
            };
            assert_eq!(format!("{secret:?}"), "Secret([REDACTED])");
            let mut previous = Vec::new();
            prost::encoding::string::encode(424_980_157, &secret.secrets_json, &mut previous);
            assert_eq!(Secret::FIELDS[0].tag, 424_980_157);
            assert_eq!(secret.encode_to_vec(), previous);
            let bound = Secret::decoding_memory(&previous).unwrap();
            let decoded = Secret::decode(&previous).unwrap();
            assert_eq!(decoded, secret);
            assert!(decoded.memory_size() <= bound);
        });
    }
    {
        scenario_runtime::check("debug_preserves_wire", || {
            let original = Choice::Selected {
                row: Native {
                    value: 7,
                    payload: b"private-token".to_vec(),
                },
            };
            let bytes = original.encode_to_vec();
            let redacted = RedactedChoice::decode(&bytes).unwrap();
            assert_eq!(format!("{redacted:?}"), "RedactedChoice([REDACTED])");
            assert_eq!(redacted.encode_to_vec(), bytes);
            let settings = harmos_contract::config::SchemaSettings::default();
            let redacted_schema = harmos_contract::schema::graph::Schema::from_declaration(
                RedactedChoice::DEFINITION,
                &settings,
            )
            .unwrap();
            let original_schema = harmos_contract::schema::graph::Schema::from_declaration(
                Choice::DEFINITION,
                &settings,
            )
            .unwrap();
            assert!(
                redacted_schema
                    .equivalent(
                        redacted_schema.root,
                        &original_schema,
                        original_schema.root,
                        &settings
                    )
                    .unwrap()
            );
            assert!(redacted.memory_size() <= RedactedChoice::decoding_memory(&bytes).unwrap());
        });
    }
    {
        scenario_runtime::check("composition_preserves_domain", || {
            let value = Required {
                row: native(7),
                optional: Some(native(8)),
                rows: vec![native(9)],
                label: "postcard".into(),
            };
            let encoded = harmos_contract::Message::encode_to_vec(&value);
            let bound = <Required as harmos_contract::Message>::decoding_memory(&encoded).unwrap();
            let decoded = <Required as harmos_contract::Message>::decode(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert!(decoded.memory_size() <= bound);
            let invalid = Wire {
                value: 0,
                payload: vec![1],
            };
            let encoded = harmos_contract::Message::encode_to_vec(&invalid);
            assert!(<Native as harmos_contract::Message>::decode(&encoded).is_err());
        });
    }
}
