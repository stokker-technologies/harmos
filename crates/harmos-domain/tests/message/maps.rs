/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable Map Fields And Root Payloads Across Their Codec Boundaries.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::{Cardinality, Described, graph::Schema};
use harmos_domain::{MemorySize, Message, Portable, message};
use harmos_sdk::grpc::{self, DecodeInput};

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Hashed {
    values: HashMap<String, String>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Ordered {
    values: BTreeMap<String, String>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Seeded {
    values: HashMap<String, String, BuildHasherDefault<DefaultHasher>>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Row {
    values: HashMap<i32, super::Original>,
    bytes: BTreeMap<u64, Vec<u8>>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Recursive {
    children: HashMap<String, Recursive>,
}

#[derive(Debug)]
struct OwnedHasher {
    state: Vec<u8>,
}

impl Default for OwnedHasher {
    fn default() -> Self {
        Self { state: vec![0; 128] }
    }
}

impl BuildHasher for OwnedHasher {
    type Hasher = DefaultHasher;
    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher::new()
    }
}

impl MemorySize for OwnedHasher {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.state.capacity()
    }
}

fn roundtrip<T: Portable + PartialEq + Debug>(value: &T) {
    let bytes = Message::encode(value);
    assert_eq!(value.encoded_size(), bytes.len());
    let bound = T::decoding_memory(&bytes).unwrap();
    let decoded = T::decode(&bytes).unwrap();
    assert_eq!(&decoded, value);
    assert!(decoded.memory_size() <= bound, "{bound} must cover {}", decoded.memory_size());

    let bytes = super::encode(value);
    let bound = grpc::prepare::<T>(DecodeInput::new(&bytes)).unwrap().memory();
    let decoded = grpc::decode::<T>(DecodeInput::new(&bytes)).unwrap();
    assert_eq!(&decoded, value);
    assert!(decoded.memory_size() <= bound);
    let settings = SchemaSettings::default();
    let schema = Schema::from_declaration(T::DEFINITION, &settings).unwrap();
    let validator = harmos_domain::schema::Validator::from_schema(&schema, &settings).unwrap();
    validator.validate(&Message::encode(value)).unwrap();
}

#[test]
fn map_order_is_canonical() {
    let entries = [("beta".to_owned(), "second".to_owned()), ("alpha".to_owned(), "first".to_owned())];
    let first = Hashed { values: entries.clone().into() };
    let second = Hashed {
        values: entries.clone().into_iter().rev().collect(),
    };
    let ordered = Ordered { values: entries.clone().into() };
    let seeded = Seeded { values: entries.into_iter().collect() };
    for bytes in [Message::encode(&second), Message::encode(&ordered), Message::encode(&seeded)] {
        assert_eq!(Message::encode(&first), bytes);
    }
    assert_eq!(super::encode(&first), super::encode(&ordered));
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.digest(), ordered.digest());
    assert_eq!(Hashed::fingerprint(), Ordered::fingerprint());
    assert_eq!(Hashed::fingerprint(), Seeded::fingerprint());
    let settings = SchemaSettings::default();
    let graph = Schema::from_declarations(
        &[("hashed", HashMap::<String, String>::DEFINITION), ("ordered", BTreeMap::<String, String>::DEFINITION)],
        &settings,
    )
    .unwrap();
    graph.prepare(&settings).unwrap();
    roundtrip(&first);
    roundtrip(&ordered);
    roundtrip(&seeded);
    roundtrip(&first.values);
    roundtrip(&ordered.values);
    assert_eq!(Message::encode(&first.values), Message::encode(&ordered.values));
    assert_eq!(HashMap::<String, String>::fingerprint(), BTreeMap::<String, String>::fingerprint());
}

#[test]
fn map_keys_values_and_defaults() {
    roundtrip(&Hashed::default());
    roundtrip(&Ordered::default());
    roundtrip(&HashMap::<String, String>::new());
    roundtrip(&BTreeMap::<String, String>::new());
    roundtrip(&HashMap::from([(false, 0_u32), (true, u32::MAX)]));
    roundtrip(&HashMap::from([(u32::MAX, f64::MAX), (0, -1.0)]));
    roundtrip(&BTreeMap::from([(u64::MAX, f32::MAX), (0, -1.0)]));
    roundtrip(&HashMap::from([(i32::MIN, true), (i32::MAX, false)]));
    roundtrip(&BTreeMap::from([(i64::MIN, i64::MAX), (i64::MAX, i64::MIN)]));
    let value = Row {
        values: HashMap::from([(-1, super::Original { value: u64::MAX, payload: vec![0, 255] })]),
        bytes: BTreeMap::from([(1, vec![0, 128, 255])]),
    };
    roundtrip(&value);
    assert_eq!(grpc::decode::<Row>(DecodeInput::new(&[])).unwrap(), Row::default());
    let graph = Schema::from_declaration(Row::DEFINITION, &SchemaSettings::default()).unwrap();
    assert!(Row::FIELDS.iter().all(|field| field.cardinality == Cardinality::Repeated));
    let descriptor = super::schema_descriptor(Row::DEFINITION);
    for field in descriptor.fields() {
        assert!(field.is_list());
    }
    graph.prepare(&SchemaSettings::default()).unwrap();
}

#[test]
fn map_decoding_refuses_invalid() {
    type Map = HashMap<u32, u32>;
    for bytes in [&[2, 1, 7, 1, 8][..], &[2, 1][..], &[1][..], &[0, 1][..], &[255][..]] {
        assert!(Map::decode(bytes).is_err());
        assert!(BTreeMap::<u32, u32>::decode(bytes).is_err());
    }
    let oversized = postcard::to_allocvec(&1_048_577_usize).unwrap();
    let duplicate = [super::encode(&HashMap::from([(1_u32, 7_u32)])), super::encode(&HashMap::from([(1_u32, 8_u32)]))].concat();
    assert!(grpc::decode::<Map>(DecodeInput::new(&duplicate)).is_err());
    assert!(Map::decoding_memory(&oversized).is_err());
    assert!(BTreeMap::<u32, u32>::decoding_memory(&oversized).is_err());

    // Independent Producers May Supply Entry Messages In Any Order.
    let unordered = [2, 9, 7, 1, 8];
    let value = Map::decode(&unordered).unwrap();
    assert_eq!(value, HashMap::from([(9, 7), (1, 8)]));
    assert_eq!(Message::encode(&value), [2, 1, 8, 9, 7]);

    let mut recursive = Recursive::default();
    for _ in 0..40 {
        recursive = Recursive {
            children: HashMap::from([("child".into(), recursive)]),
        };
    }
    assert!(Recursive::decoding_memory(&Message::encode(&recursive)).is_err());
    let bytes = Message::encode(&Hashed {
        values: HashMap::from([("name".into(), "value".into())]),
    });
    for end in 0..bytes.len() {
        assert!(Hashed::decode(&bytes[..end]).is_err());
    }
}

#[test]
fn map_capacity_is_charged() {
    for count in 0..64 {
        let value: HashMap<_, _> = (0..count).map(|value| (value.to_string(), "value".repeat(value))).collect();
        let bytes = Message::encode(&value);
        let decoded = HashMap::<String, String>::decode(&bytes).unwrap();
        assert!(decoded.memory_size() <= HashMap::<String, String>::decoding_memory(&bytes).unwrap());
        assert!(value.memory_size() >= std::mem::size_of_val(&value) + value.len() * std::mem::size_of::<(String, String)>());
    }
    let mut value: HashMap<u32, String> = HashMap::with_capacity(1024);
    let reserved = value.memory_size();
    value.insert(1, "allocated".repeat(100));
    assert!(value.memory_size() >= reserved + 900);
}

#[test]
fn map_hasher_ownership_is_charged() {
    let mut value = HashMap::<u32, u32, OwnedHasher>::default();
    value.insert(1, 7);
    roundtrip(&value);
    let default = HashMap::<u32, u32>::from([(1, 7)]);
    assert_eq!(Message::encode(&value), Message::encode(&default));
    assert!(value.memory_size() >= std::mem::size_of_val(&value) + 128);
}

#[test]
fn root_option_preserves_presence() {
    roundtrip(&Option::<u64>::None);
    roundtrip(&Some(0_u64));
    roundtrip(&Some(false));
    roundtrip(&Some(String::new()));
    roundtrip(&Some(Vec::<u8>::new()));
    roundtrip(&Some(()));
    roundtrip(&Option::<super::Original>::None);
    roundtrip(&Some(super::Original { value: 0, payload: vec![] }));
    roundtrip(&Some(super::Original { value: 7, payload: vec![0, 255] }));
    roundtrip(&Some(BTreeMap::from([("name".to_owned(), "value".to_owned())])));
    roundtrip(&Some(Option::<u64>::None));
    roundtrip(&Some(Some(0_u64)));
    assert_eq!(grpc::decode::<Option<u64>>(DecodeInput::new(&[])).unwrap(), None);
    assert_ne!(super::encode(&Some(0_u64)), super::encode(&Option::<u64>::None));
    assert_ne!(Some(0_u64).digest(), Option::<u64>::None.digest());
    assert_ne!(Option::<u64>::fingerprint(), u64::fingerprint());
    assert!(Option::<u64>::decode(&[1]).is_err());
    assert!(Option::<u64>::decode(&[2]).is_err());
}
