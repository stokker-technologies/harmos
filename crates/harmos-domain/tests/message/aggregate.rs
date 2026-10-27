/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Hand-Written Aggregate Message Against Its Derived Twin

/* === Modules === */

// The Shape `#[harmos::message]` Derives For The Same Vocabulary
mod twin {
    use harmos_domain::message;

    #[message(harmos_path = ::harmos_domain)]
    #[derive(Clone, Copy, PartialEq)]
    pub enum Aggregate {
        Count,
        Sum,
        Mean,
        Median,
        Min,
        Max,
        Rms,
        Std,
        Percentile { rank: u32 },
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::graph::Schema;
use harmos_domain::schema::{Cardinality, Described, Kind};
use harmos_domain::{Aggregate, MemorySize, Message, message};
use harmos_sdk::grpc::{self, DecodeInput, prost};

/* === Constants === */

const EVERY: [Aggregate; 11] = [
    Aggregate::Count,
    Aggregate::Sum,
    Aggregate::Mean,
    Aggregate::Median,
    Aggregate::Min,
    Aggregate::Max,
    Aggregate::Rms,
    Aggregate::Std,
    Aggregate::Percentile(95),
    Aggregate::Percentile(0),
    Aggregate::Percentile(255),
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Aggregate Reaches Every Field Cardinality Of A Derived Message
#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Summary {
    aggregate: Aggregate,
    optional: Option<Aggregate>,
    aggregates: Vec<Aggregate>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[test]
fn aggregate_protobuf_contract() {
    use harmos_packaging::proto::reflection::DynamicMessage;

    let schema = crate::schema_descriptor(Aggregate::DEFINITION);
    assert_eq!(schema.oneofs().count(), 1);
    for value in EVERY {
        let encoded = crate::encode(&value);
        assert_eq!(encoded, crate::encode(&twin(value)), "{value:?}");
        assert_eq!(grpc::decode::<Aggregate>(DecodeInput::new(&encoded)).unwrap(), value);
        // The Transcoding Budget Bounds The Decoded Value From Above.
        assert!(grpc::prepare::<Aggregate>(DecodeInput::new(&encoded)).map(|prepared| prepared.memory()).unwrap() >= value.memory_size());
        let derived = grpc::decode::<twin::Aggregate>(DecodeInput::new(&encoded)).unwrap();
        assert!(derived == twin(value), "{value:?}");
        let reflected = DynamicMessage::decode(schema.clone(), encoded.as_slice()).unwrap();
        assert_eq!(prost::Message::encode_to_vec(&reflected), encoded);
    }

    // Defaults, Repeated Variants And Malformed Values Follow The Same Wire Contract.
    assert_eq!(grpc::decode::<Aggregate>(DecodeInput::new(&[])).unwrap(), Aggregate::Count);
    let wide = crate::encode(&twin::Aggregate::Percentile { rank: 256 });
    assert!(grpc::decode::<Aggregate>(DecodeInput::new(&wide)).is_err());
    assert!(grpc::prepare::<Aggregate>(DecodeInput::new(&wide)).map(|prepared| prepared.memory()).is_err());
    // Last Occurrence Wins, Matching Derived Enum Messages
    let mut last = crate::encode(&Aggregate::Percentile(50));
    last.extend(crate::encode(&Aggregate::Max));
    assert_eq!(grpc::decode::<Aggregate>(DecodeInput::new(&last)).unwrap(), Aggregate::Max);
    let mut unknown = Vec::new();
    prost::encoding::bytes::encode(7, &Vec::new(), &mut unknown);
    assert!(grpc::decode::<Aggregate>(DecodeInput::new(&unknown)).is_err());
    assert!(grpc::decode::<Aggregate>(DecodeInput::new(&[0xff])).is_err());
}

#[test]
fn postcard_matches_serde() {
    for value in EVERY {
        let encoded = Message::encode(&value);
        assert_eq!(encoded.len(), Message::encoded_size(&value));
        assert_eq!(encoded, postcard::to_allocvec(&value).unwrap(), "{value:?}");
        assert_eq!(<Aggregate as Message>::decode(&encoded).unwrap(), value);
        assert_eq!(<Aggregate as Message>::decoding_memory(&encoded).unwrap(), value.memory_size());
    }
    assert!(<Aggregate as Message>::decode(&[9]).is_err());
    assert!(<Aggregate as Message>::decode(&[8]).is_err());
}

#[test]
fn aggregate_message_field() {
    let value = Summary { aggregate: Aggregate::Percentile(95), optional: Some(Aggregate::Rms), aggregates: EVERY.to_vec() };
    let encoded = crate::encode(&value);
    assert!(grpc::decode::<Summary>(DecodeInput::new(&encoded)).unwrap() == value);
    let encoded = Message::encode(&value);
    assert!(<Summary as Message>::decode(&encoded).unwrap() == value);
    let empty = grpc::decode::<Summary>(DecodeInput::new(&[])).unwrap();
    assert_eq!(empty.aggregate, Aggregate::Count);
    assert_eq!(empty.optional, None);
}

#[test]
fn schema_matches_derive() {
    assert_eq!(Aggregate::FIELDS.len(), twin::Aggregate::FIELDS.len());
    for (field, derived) in Aggregate::FIELDS.iter().zip(twin::Aggregate::FIELDS) {
        assert_eq!(field.name, derived.name);
        assert_eq!(field.tag, derived.tag);
        assert_eq!(field.cardinality, Cardinality::Variant);
        assert_eq!(derived.cardinality, Cardinality::Variant);
        let (Kind::Message { definition }, Kind::Message { definition: expected }) = (&field.kind, &derived.kind) else {
            panic!("variant payloads are messages");
        };
        assert_eq!((definition.fields)(), (expected.fields)());
    }
    let percentile = Aggregate::FIELDS.iter().find(|field| field.name == "Percentile").unwrap();
    let Kind::Message { definition } = &percentile.kind else {
        panic!("percentile payload");
    };
    let rank = &(definition.fields)()[0];
    assert_eq!((rank.name.as_ref(), rank.kind), ("rank", Kind::U32));
    let settings = SchemaSettings::default();
    let left = Schema::from_declaration(Aggregate::DEFINITION, &settings).unwrap();
    let right = Schema::from_declaration(twin::Aggregate::DEFINITION, &settings).unwrap();
    assert!(left.equivalent(left.root, &right, right.root, &settings).unwrap());
    Schema::from_declaration(Summary::DEFINITION, &settings).unwrap();
}

/* === Private Functions === */

fn twin(value: Aggregate) -> twin::Aggregate {
    match value {
        Aggregate::Count => twin::Aggregate::Count,
        Aggregate::Sum => twin::Aggregate::Sum,
        Aggregate::Mean => twin::Aggregate::Mean,
        Aggregate::Median => twin::Aggregate::Median,
        Aggregate::Min => twin::Aggregate::Min,
        Aggregate::Max => twin::Aggregate::Max,
        Aggregate::Rms => twin::Aggregate::Rms,
        Aggregate::Std => twin::Aggregate::Std,
        Aggregate::Percentile(rank) => twin::Aggregate::Percentile { rank: u32::from(rank) },
    }
}

/* === Tests === */
