/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Boxed Message Fields Against Their Plain Twins

/* === Modules === */

// Plain Nested Messages Own The Reference Wire Shape
mod plain {
    use super::Leaf;
    use harmos_domain::message;

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Holder {
        pub row: Leaf,
        pub count: u32,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Choice {
        Empty,
        Tuple(Leaf, u32),
        Named { row: Leaf, note: Option<String> },
    }
}

// Boxed Twins Keep Every Authored Name And Shape
mod heap {
    use super::Leaf;
    use harmos_domain::message;

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Holder {
        pub row: Box<Leaf>,
        pub count: u32,
    }

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum Choice {
        Empty,
        Tuple(Box<Leaf>, u32),
        Named { row: Box<Leaf>, note: Option<String> },
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt::Debug;
// Third Party Imports

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::graph::Schema;
use harmos_domain::schema::{Described, Kind};
use harmos_domain::{MemorySize, Message, message};
use harmos_sdk::grpc::{self, DecodeInput, prost};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Leaf {
    value: u64,
    payload: Vec<u8>,
    label: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<heap::Holder> for plain::Holder {
    fn from(value: heap::Holder) -> Self {
        Self { row: *value.row, count: value.count }
    }
}

impl From<heap::Choice> for plain::Choice {
    fn from(value: heap::Choice) -> Self {
        match value {
            heap::Choice::Empty => Self::Empty,
            heap::Choice::Tuple(row, count) => Self::Tuple(*row, count),
            heap::Choice::Named { row, note } => Self::Named { row: *row, note },
        }
    }
}

/* === Public Functions === */

#[test]
fn boxed_descriptors_match() {
    descriptors::<plain::Holder, heap::Holder>();
    descriptors::<plain::Choice, heap::Choice>();
    let row = heap::Holder::FIELDS.iter().find(|field| field.name == "row").unwrap();
    let Kind::Message { definition } = &row.kind else { panic!("boxed field describes its nested message") };
    assert_eq!((definition.identity)(), std::any::TypeId::of::<Leaf>());
    assert_eq!((definition.fields)(), Leaf::FIELDS);
}

#[test]
fn boxed_codec_parity() {
    let (plain, boxed) = holders();
    assert_eq!(crate::encode(&boxed), crate::encode(&plain));
    assert_eq!(crate::encode(&boxed).len(), crate::encode(&plain).len());
    assert_eq!(Message::encode(&boxed), Message::encode(&plain));
    assert_eq!(Message::encoded_size(&boxed), Message::encoded_size(&plain));
    for (plain, boxed) in choices() {
        assert_eq!(crate::encode(&boxed), crate::encode(&plain));
        assert_eq!(crate::encode(&boxed).len(), crate::encode(&plain).len());
        assert_eq!(Message::encode(&boxed), Message::encode(&plain));
        assert_eq!(Message::encoded_size(&boxed), Message::encoded_size(&plain));
    }

    // Both Representations Decode Default, Repeated And Segmented Fields.
    let encoded = crate::encode(&plain);
    let mut duplicate = encoded.clone();
    duplicate.extend(crate::encode(&plain::Holder { row: Leaf { value: 0, payload: vec![4], label: "second".into() }, count: 9 }));
    let segmented = segmented();
    for bytes in [&[][..], &encoded[..], &duplicate[..], &segmented[..]] {
        let decoded = agree::<plain::Holder, heap::Holder>(bytes, 1).unwrap();
        assert_eq!(decoded.row.payload.capacity(), decoded.row.payload.len());
    }
    let empty = agree::<plain::Holder, heap::Holder>(&[], 1).unwrap();
    assert_eq!(empty.row, Leaf::default());
    let merged = agree::<plain::Holder, heap::Holder>(&duplicate, 1).unwrap();
    assert_eq!(merged.row.value, plain.row.value);
    assert_eq!(merged.row.payload, [4], "bytes keep the last occurrence");
    assert_eq!(merged.row.label, "second");
    assert_eq!(merged.count, 9);
    let segments = agree::<plain::Holder, heap::Holder>(&segmented, 1).unwrap();
    assert_eq!(segments.row.value, 7);
    assert_eq!(segments.row.label, "split");

    for (plain, _) in choices() {
        let boxes = usize::from(!matches!(plain, plain::Choice::Empty));
        let bytes = crate::encode(&plain);
        let decoded = agree::<plain::Choice, heap::Choice>(&bytes, boxes).unwrap();
        assert_eq!(decoded, plain);
        let postcard = Message::encode(&plain);
        let decoded = <heap::Choice as Message>::decode(&postcard).unwrap();
        assert_eq!(plain::Choice::from(decoded), plain);
    }

    // Portable Preflight Includes The Box Pointee And Bounds Resident Memory.
    let bytes = Message::encode(&plain);
    let plain_bound = <plain::Holder as Message>::decoding_memory(&bytes).unwrap();
    let boxed_bound = <heap::Holder as Message>::decoding_memory(&bytes).unwrap();
    assert_eq!(
        boxed_bound - std::mem::size_of::<heap::Holder>(),
        plain_bound - std::mem::size_of::<plain::Holder>() + std::mem::size_of::<Leaf>()
    );
    let decoded = <heap::Holder as Message>::decode(&bytes).unwrap();
    assert_eq!(decoded, boxed);
    assert!(decoded.memory_size() <= boxed_bound);
    assert_eq!(
        boxed.memory_size(),
        plain.memory_size() - std::mem::size_of::<plain::Holder>() + std::mem::size_of::<heap::Holder>() + std::mem::size_of::<Leaf>()
    );
}

#[test]
fn boxed_refuses_malformed() {
    let mut truncated = crate::encode(&holders().0);
    truncated.pop();
    let wrong_wire = wrong_wire();
    let deep_group = deep_group();
    for bytes in [&[0xff][..], &truncated[..], &wrong_wire[..], &deep_group[..]] {
        assert!(agree::<plain::Holder, heap::Holder>(bytes, 1).is_none());
    }
    let postcard = Message::encode(&holders().0);
    let trailing = [postcard.clone(), vec![0]].concat();
    for bytes in [&postcard[..postcard.len() - 1], &trailing[..]] {
        assert!(<plain::Holder as Message>::decode(bytes).is_err());
        assert!(<heap::Holder as Message>::decode(bytes).is_err());
        assert!(<heap::Holder as Message>::decoding_memory(bytes).is_err());
    }
}

/* === Private Functions === */

fn holders() -> (plain::Holder, heap::Holder) {
    let row = Leaf { value: 7, payload: vec![1, 2, 255], label: "first".into() };
    let boxed = heap::Holder { row: Box::new(row.clone()), count: 3 };
    let plain = plain::Holder { row, count: 3 };
    (plain, boxed)
}

fn choices() -> Vec<(plain::Choice, heap::Choice)> {
    let row = Leaf { value: u64::MAX, payload: vec![0, 128], label: "variant".into() };
    vec![
        (plain::Choice::Empty, heap::Choice::Empty),
        (plain::Choice::Tuple(row.clone(), 5), heap::Choice::Tuple(Box::new(row.clone()), 5)),
        (
            plain::Choice::Named { row: row.clone(), note: Some("noted".into()) },
            heap::Choice::Named { row: Box::new(row), note: Some("noted".into()) },
        ),
    ]
}

// Compare Descriptor Graphs, Their JSON Bytes And Authored Field Identity
fn descriptors<P: Described, B: Described>() {
    let settings = SchemaSettings::default();
    let plain = Schema::from_declaration(P::DEFINITION, &settings).unwrap();
    let boxed = Schema::from_declaration(B::DEFINITION, &settings).unwrap();
    assert_eq!(boxed, plain);
    assert_eq!(serde_json::to_vec(&boxed).unwrap(), serde_json::to_vec(&plain).unwrap());
    assert_eq!(B::FIELDS.len(), P::FIELDS.len());
    for (boxed, plain) in B::FIELDS.iter().zip(P::FIELDS) {
        assert_eq!((&boxed.name, boxed.tag, boxed.cardinality), (&plain.name, plain.tag, plain.cardinality));
    }
}

// Decode One Payload Through Both Twins; Both Accept Or Both Refuse
fn agree<P, B>(bytes: &[u8], boxes: usize) -> Option<P>
where
    P: harmos_domain::Message + harmos_domain::schema::Described + MemorySize + PartialEq + Debug,
    B: harmos_domain::Message + harmos_domain::schema::Described + MemorySize + Into<P>,
{
    let plain = grpc::decode::<P>(DecodeInput::new(bytes));
    let boxed = grpc::decode::<B>(DecodeInput::new(bytes));
    let plain_bound = grpc::prepare::<P>(DecodeInput::new(bytes)).map(|prepared| prepared.memory());
    let boxed_bound = grpc::prepare::<B>(DecodeInput::new(bytes)).map(|prepared| prepared.memory());
    let (Ok(plain), Ok(boxed)) = (plain, boxed) else {
        assert!(grpc::decode::<P>(DecodeInput::new(bytes)).is_err());
        assert!(grpc::decode::<B>(DecodeInput::new(bytes)).is_err());
        assert!(boxed_bound.is_err());
        return None;
    };

    // Bound And Resident Size Differ Only By The Box Pointee
    let boxed_bound = boxed_bound.unwrap();
    assert!(plain_bound.is_ok());
    assert!(boxed.memory_size() <= boxed_bound);
    assert_eq!(
        boxed.memory_size() - std::mem::size_of::<B>(),
        plain.memory_size() - std::mem::size_of::<P>() + boxes * std::mem::size_of::<Leaf>()
    );
    let boxed = boxed.into();
    assert_eq!(boxed, plain);
    Some(plain)
}

fn tag<T: Described>(name: &str) -> u32 {
    T::FIELDS.iter().find(|field| field.name == name).unwrap().tag
}

// Split One Nested Message Across Two Length-Delimited Segments
fn segmented() -> Vec<u8> {
    let row = tag::<plain::Holder>("row");
    let mut first = Vec::new();
    prost::encoding::uint64::encode(tag::<Leaf>("value"), &7, &mut first);
    let mut second = Vec::new();
    prost::encoding::string::encode(tag::<Leaf>("label"), &"split".to_owned(), &mut second);

    let mut bytes = Vec::new();
    prost::encoding::bytes::encode(row, &first, &mut bytes);
    prost::encoding::uint32::encode(tag::<plain::Holder>("count"), &2, &mut bytes);
    prost::encoding::bytes::encode(row, &second, &mut bytes);
    bytes
}

// A Varint Where The Nested Message Declares Bytes
fn wrong_wire() -> Vec<u8> {
    let mut row = Vec::new();
    prost::encoding::uint64::encode(tag::<Leaf>("payload"), &1, &mut row);
    let mut bytes = Vec::new();
    prost::encoding::bytes::encode(tag::<plain::Holder>("row"), &row, &mut bytes);
    bytes
}

// Unknown Groups Nested Beyond The Shared Decode Depth
fn deep_group() -> Vec<u8> {
    let mut row = Vec::new();
    for _ in 0..40 {
        prost::encoding::encode_key(1, prost::encoding::WireType::StartGroup, &mut row);
    }
    for _ in 0..40 {
        prost::encoding::encode_key(1, prost::encoding::WireType::EndGroup, &mut row);
    }
    let mut bytes = Vec::new();
    prost::encoding::bytes::encode(tag::<plain::Holder>("row"), &row, &mut bytes);
    bytes
}

/* === Tests === */
