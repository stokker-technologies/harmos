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

// Third Party Imports

// Local Imports
use harmos_domain::__private::DecodeInput;
use harmos_domain::operations::host::{
    EgressRequest, Fetched, FollowingRow, Header, Measured, Publication, RecallResponse,
    Remembered, SurveyResponse,
};
use harmos_domain::{MemorySize, Protobuf};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The Previous Wire Schema Is An Independent Compatibility Oracle
#[derive(Clone, PartialEq, prost::Message)]
struct PreviousPublication {
    #[prost(string, tag = "1")]
    stream: String,
    #[prost(uint32, tag = "2")]
    schema: u32,
    #[prost(uint64, optional, tag = "3")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "4")]
    published_unix_micros: Option<i64>,
    #[prost(string, optional, tag = "5")]
    context: Option<String>,
    #[prost(bytes = "vec", tag = "6")]
    payload: Vec<u8>,
}

// A Batch Appends Following Rows As Repeated Tag 7 To The Single-Row Shape
#[derive(Clone, PartialEq, prost::Message)]
struct BatchedPublication {
    #[prost(string, tag = "1")]
    stream: String,
    #[prost(uint64, optional, tag = "3")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "4")]
    published_unix_micros: Option<i64>,
    #[prost(bytes = "vec", tag = "6")]
    payload: Vec<u8>,
    #[prost(message, repeated, tag = "7")]
    following: Vec<BatchedRow>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct BatchedRow {
    #[prost(bytes = "vec", tag = "1")]
    payload: Vec<u8>,
    #[prost(int64, optional, tag = "2")]
    published_delta_micros: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct PreviousHeader {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    value: String,
}

#[derive(Clone, PartialEq, prost::Message)]
struct PreviousEgress {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    method: String,
    #[prost(string, tag = "3")]
    path: String,
    #[prost(message, repeated, tag = "4")]
    headers: Vec<PreviousHeader>,
    #[prost(bytes = "vec", tag = "5")]
    body: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn roundtrip<T: Protobuf + MemorySize + std::fmt::Debug + PartialEq>(value: T) {
    let encoded = value.encode_to_vec();
    assert_eq!(encoded.len(), value.encoded_len());
    let bound = T::decoding_memory(&encoded).unwrap();
    let decoded = T::decode(&encoded).unwrap();
    assert_eq!(decoded, value);
    assert!(decoded.memory_size() <= bound);
    let mut appended = vec![255];
    value.encode_into(&mut appended);
    assert_eq!(&appended[1..], encoded);
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("publication_keeps_tags", || {
            for value in [
                Publication::default(),
                Publication {
                    stream: "records".into(),
                    schema: u32::MAX,
                    sequence: Some(0),
                    published_unix_micros: Some(i64::MIN),
                    context: Some(String::new()),
                    payload: vec![0, 128, 255],
                    following: Vec::new(),
                },
            ] {
                let previous = PreviousPublication {
                    stream: value.stream.clone(),
                    schema: value.schema,
                    sequence: value.sequence,
                    published_unix_micros: value.published_unix_micros,
                    context: value.context.clone(),
                    payload: value.payload.clone(),
                };
                let bytes = prost::Message::encode_to_vec(&previous);
                assert_eq!(value.encode_to_vec(), bytes);
                assert_eq!(Publication::decode(&bytes).unwrap(), value);
                assert_eq!(
                    <PreviousPublication as prost::Message>::decode(bytes.as_slice()).unwrap(),
                    previous
                );
                roundtrip(value);
            }
        });
    }
    {
        scenario_runtime::check("batch_rows_follow", || {
            // A Steady 1 kHz Series: The Head Carries The Base Stamp And Each
            // Following Row A Short Delta; A Backwards Row Stays Encodable.
            let deltas = [Some(1_000), Some(1_000), Some(-1), None];
            let value = Publication {
                stream: "records".into(),
                sequence: Some(7),
                published_unix_micros: Some(1_760_000_000_000_000),
                payload: vec![1],
                following: deltas
                    .iter()
                    .zip(2_u8..)
                    .map(|(&published_delta_micros, payload)| FollowingRow {
                        payload: vec![payload],
                        published_delta_micros,
                    })
                    .collect(),
                ..Publication::default()
            };
            let oracle = BatchedPublication {
                stream: value.stream.clone(),
                sequence: value.sequence,
                published_unix_micros: value.published_unix_micros,
                payload: value.payload.clone(),
                following: deltas
                    .iter()
                    .zip(2_u8..)
                    .map(|(&published_delta_micros, payload)| BatchedRow {
                        payload: vec![payload],
                        published_delta_micros,
                    })
                    .collect(),
            };
            let bytes = prost::Message::encode_to_vec(&oracle);
            assert_eq!(value.encode_to_vec(), bytes);
            assert_eq!(Publication::decode(&bytes).unwrap(), value);
            // One Byte Of Payload Plus A Millisecond Delta: Six Bytes
            let steady = FollowingRow {
                payload: vec![0],
                published_delta_micros: Some(1_000),
            };
            let oracle = BatchedRow {
                payload: vec![0],
                published_delta_micros: Some(1_000),
            };
            assert_eq!(
                steady.encode_to_vec(),
                prost::Message::encode_to_vec(&oracle)
            );
            assert_eq!(steady.encode_to_vec().len(), 6);
            roundtrip(value);
        });
    }
    {
        scenario_runtime::check("headers_keep_elements", || {
            let value = EgressRequest {
                name: "endpoint".into(),
                method: "GET".into(),
                path: "/".into(),
                headers: vec![
                    Header::default(),
                    Header {
                        name: "Accept".into(),
                        value: "text/plain".into(),
                    },
                ],
                body: vec![7],
            };
            let previous = PreviousEgress {
                name: value.name.clone(),
                method: value.method.clone(),
                path: value.path.clone(),
                headers: value
                    .headers
                    .iter()
                    .map(|header| PreviousHeader {
                        name: header.name.clone(),
                        value: header.value.clone(),
                    })
                    .collect(),
                body: value.body.clone(),
            };
            assert_eq!(
                value.encode_to_vec(),
                prost::Message::encode_to_vec(&previous)
            );
            roundtrip(value);
        });
    }
    {
        scenario_runtime::check("messages_charge_container", || {
            let bytes = [0x22, 0].repeat(4096);
            let bound = EgressRequest::decoding_memory(&bytes).unwrap();
            let value = EgressRequest::decode(&bytes).unwrap();
            assert_eq!(value.headers.len(), 4096);
            assert_eq!(value.headers.capacity(), 4096);
            assert!(
                bound
                    >= std::mem::size_of::<EgressRequest>() + 4096 * std::mem::size_of::<Header>()
            );
            assert!(bound >= value.memory_size());
            assert!(value.memory_size() > bytes.len());
        });
    }
    {
        scenario_runtime::check("preflight_checks_replacements", || {
            // Last Scalar Wins, But Both Allocations Remain In The Bound
            let bytes = [0x0a, 3, b'o', b'l', b'd', 0x0a, 1, b'n'];
            let value = Publication::decode(&bytes).unwrap();
            assert_eq!(value.stream, "n");
            assert!(
                Publication::decoding_memory(&bytes).unwrap()
                    >= std::mem::size_of::<Publication>() + 4
            );
            // A Borrowed Parent Merge Supplies Two Fragments Without Concatenation
            let parent = [0x0a, 5, 0x0a, 3, b'o', b'l', b'd', 0x0a, 3, 0x0a, 1, b'n'];
            let source = DecodeInput::new(&parent);
            let input = source.field(1).unwrap();
            assert_eq!(Publication::decode_input(input).unwrap(), value);
            assert_eq!(
                Publication::decoding_memory_input(input).unwrap(),
                Publication::decoding_memory(&bytes).unwrap()
            );
            for malformed in [vec![0x22, 3, 0x0a, 1, 255], vec![0x22, 1, 0], vec![0x08, 0]] {
                assert!(EgressRequest::decoding_memory(&malformed).is_err());
                assert!(EgressRequest::decode(&malformed).is_err());
            }
            // Unknown Fields Remain Accepted By The Original Protobuf Contract
            assert_eq!(
                Publication::decode(&[0x78, 1]).unwrap(),
                Publication::default()
            );
        });
    }
    {
        scenario_runtime::check("responses_bound_rows", || {
            roundtrip(Fetched {
                status: 200,
                headers: vec![Header::default()],
                body: vec![8; 64],
            });
            roundtrip(RecallResponse {
                rows: vec![
                    Remembered {
                        position: u64::MAX,
                        name: "record".into(),
                        version: 2,
                        payload: vec![1; 32],
                    },
                    Remembered::default(),
                ],
            });
            roundtrip(SurveyResponse {
                rows: vec![
                    Measured {
                        field: "temperature".into(),
                        value: f64::NEG_INFINITY,
                    },
                    Measured::default(),
                ],
            });
        });
    }
}
