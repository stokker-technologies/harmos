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

// Third Party Imports

// Local Imports
use harmos_domain::MemorySize;
use harmos_domain::operations::streams::{FollowingRow, Publish};
use harmos_sdk::grpc::{self, DecodeInput, prost};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Previous Wire Schema Is An Independent Compatibility Oracle
#[derive(Clone, PartialEq, prost::Message)]
#[prost(prost_path = "harmos_sdk::grpc::prost")]
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
#[prost(prost_path = "harmos_sdk::grpc::prost")]
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
#[prost(prost_path = "harmos_sdk::grpc::prost")]
struct BatchedRow {
    #[prost(bytes = "vec", tag = "1")]
    payload: Vec<u8>,

    #[prost(int64, optional, tag = "2")]
    published_delta_micros: Option<i64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn roundtrip<T: harmos_domain::Message + harmos_domain::schema::Described + MemorySize + std::fmt::Debug + PartialEq>(value: T) {
    let encoded = crate::encode(&value);
    let bound = grpc::prepare::<T>(DecodeInput::new(&encoded)).map(|prepared| prepared.memory()).unwrap();
    let decoded = grpc::decode::<T>(DecodeInput::new(&encoded)).unwrap();
    assert_eq!(decoded, value);
    assert!(decoded.memory_size() <= bound);
    let mut appended = vec![255];
    grpc::encode(&(value), &mut appended).unwrap();
    assert_eq!(&appended[1..], encoded);
}

/* === Tests === */

#[test]
fn publication_keeps_tags() {
    for value in [
        Publish::default(),
        Publish {
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
        assert_eq!(crate::encode(&value), bytes);
        assert_eq!(grpc::decode::<Publish>(DecodeInput::new(&bytes)).unwrap(), value);
        assert_eq!(<PreviousPublication as prost::Message>::decode(bytes.as_slice()).unwrap(), previous);
        roundtrip(value);
    }
}

#[test]
fn batch_rows_follow() {
    // A Steady 1 kHz Series: The Head Carries The Base Stamp And Each
    // Following Row A Short Delta; A Backwards Row Stays Encodable.
    let deltas = [Some(1_000), Some(1_000), Some(-1), None];
    let value = Publish {
        stream: "records".into(),
        sequence: Some(7),
        published_unix_micros: Some(1_760_000_000_000_000),
        payload: vec![1],
        following: deltas
            .iter()
            .zip(2_u8..)
            .map(|(&published_delta_micros, payload)| FollowingRow { payload: vec![payload], published_delta_micros })
            .collect(),
        ..Publish::default()
    };
    let oracle = BatchedPublication {
        stream: value.stream.clone(),
        sequence: value.sequence,
        published_unix_micros: value.published_unix_micros,
        payload: value.payload.clone(),
        following: deltas
            .iter()
            .zip(2_u8..)
            .map(|(&published_delta_micros, payload)| BatchedRow { payload: vec![payload], published_delta_micros })
            .collect(),
    };
    let bytes = prost::Message::encode_to_vec(&oracle);
    assert_eq!(crate::encode(&value), bytes);
    assert_eq!(grpc::decode::<Publish>(DecodeInput::new(&bytes)).unwrap(), value);
    // One Byte Of Payload Plus A Millisecond Delta: Six Bytes
    let steady = FollowingRow { payload: vec![0], published_delta_micros: Some(1_000) };
    let oracle = BatchedRow { payload: vec![0], published_delta_micros: Some(1_000) };
    assert_eq!(crate::encode(&steady), prost::Message::encode_to_vec(&oracle));
    assert_eq!(crate::encode(&steady).len(), 6);
    roundtrip(value);
}
