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
use harmos_protocol::host::{FollowingRow, Publish};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// An Independent Protobuf Oracle For The Current Publication Shape
#[derive(Clone, PartialEq, prost::Message)]
struct SinglePublication {
    #[prost(string, tag = "1")]
    stream: String,

    #[prost(uint64, optional, tag = "3")]
    sequence: Option<u64>,

    #[prost(int64, optional, tag = "4")]
    published_unix_micros: Option<i64>,

    #[prost(string, optional, tag = "5")]
    context: Option<String>,

    #[prost(bytes = "vec", tag = "6")]
    payload: Vec<u8>,
}

// A Batch Appends Following Rows as Repeated Tag 7 to the Single-Row Shape
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn admitted(value: &Publish) {
    let bytes = prost::Message::encode_to_vec(value);
    let factory = harmos_operations::execution::operations::OperationFactory::protobuf::<harmos_operations::streams::Publish>().unwrap();
    let bound = factory.grpc_decoding_memory(&bytes).unwrap();
    let decoded = <Publish as prost::Message>::decode(bytes.as_slice()).unwrap();
    let allocated = std::mem::size_of::<Publish>()
        + decoded.stream.capacity()
        + decoded.payload.capacity()
        + decoded.context.as_ref().map_or(0, String::capacity)
        + decoded.following.capacity() * std::mem::size_of::<FollowingRow>()
        + decoded.following.iter().map(|row| row.payload.capacity()).sum::<usize>();
    assert!(allocated <= bound);
    assert_eq!(&decoded, value);
}

/* === Tests === */

#[test]
fn publication_keeps_tags() {
    for value in [
        Publish::default(),
        Publish {
            stream: "records".into(),
            sequence: Some(0),
            published_unix_micros: Some(i64::MIN),
            context: Some(String::new()),
            payload: vec![0, 128, 255],
            following: Vec::new(),
        },
    ] {
        let previous = SinglePublication {
            stream: value.stream.clone(),
            sequence: value.sequence,
            published_unix_micros: value.published_unix_micros,
            context: value.context.clone(),
            payload: value.payload.clone(),
        };
        let bytes = prost::Message::encode_to_vec(&previous);
        assert_eq!(prost::Message::encode_to_vec(&value), bytes);
        assert_eq!(<Publish as prost::Message>::decode(bytes.as_slice()).unwrap(), value);
        assert_eq!(<SinglePublication as prost::Message>::decode(bytes.as_slice()).unwrap(), previous);
        admitted(&value);
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
            .map(|(&published_delta_micros, payload)| FollowingRow {
                payload: vec![payload],
                published_delta_micros,
            })
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
            .map(|(&published_delta_micros, payload)| BatchedRow {
                payload: vec![payload],
                published_delta_micros,
            })
            .collect(),
    };
    let bytes = prost::Message::encode_to_vec(&oracle);
    assert_eq!(prost::Message::encode_to_vec(&value), bytes);
    assert_eq!(<Publish as prost::Message>::decode(bytes.as_slice()).unwrap(), value);
    // One Byte Of Payload Plus A Millisecond Delta: Six Bytes
    let steady = FollowingRow {
        payload: vec![0],
        published_delta_micros: Some(1_000),
    };
    let oracle = BatchedRow {
        payload: vec![0],
        published_delta_micros: Some(1_000),
    };
    assert_eq!(prost::Message::encode_to_vec(&steady), prost::Message::encode_to_vec(&oracle));
    admitted(&value);
}
