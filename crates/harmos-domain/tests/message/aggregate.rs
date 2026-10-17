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
use harmos_domain::{Aggregate, MemorySize, Message, Protobuf, message};

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
fn protobuf_matches_derive() {
            for value in EVERY {
                let encoded = Protobuf::encode_to_vec(&value);
                assert_eq!(encoded.len(), Protobuf::encoded_len(&value));
                assert_eq!(encoded, Protobuf::encode_to_vec(&twin(value)), "{value:?}");
                assert_eq!(<Aggregate as Protobuf>::decode(&encoded).unwrap(), value);
                assert_eq!(
                    <Aggregate as Protobuf>::decoding_memory(&encoded).unwrap(),
                    value.memory_size()
                );
                let derived = <twin::Aggregate as Protobuf>::decode(&encoded).unwrap();
                assert!(derived == twin(value), "{value:?}");
            }
        }

#[test]
fn aggregate_protobuf_edges() {
            assert_eq!(
                <Aggregate as Protobuf>::decode(&[]).unwrap(),
                Aggregate::Count
            );
            let wide = Protobuf::encode_to_vec(&twin::Aggregate::Percentile { rank: 256 });
            assert!(<Aggregate as Protobuf>::decode(&wide).is_err());
            assert!(<Aggregate as Protobuf>::decoding_memory(&wide).is_err());
            // Last Occurrence Wins, Matching Derived Enum Messages
            let mut last = Protobuf::encode_to_vec(&Aggregate::Percentile(50));
            last.extend(Protobuf::encode_to_vec(&Aggregate::Max));
            assert_eq!(
                <Aggregate as Protobuf>::decode(&last).unwrap(),
                Aggregate::Max
            );
            let mut unknown = Vec::new();
            prost::encoding::bytes::encode(7, &Vec::new(), &mut unknown);
            assert!(<Aggregate as Protobuf>::decode(&unknown).is_err());
            assert!(<Aggregate as Protobuf>::decode(&[0xff]).is_err());
        }

#[test]
fn postcard_matches_serde() {
            for value in EVERY {
                let encoded = Message::encode_to_vec(&value);
                assert_eq!(encoded.len(), Message::encoded_len(&value));
                assert_eq!(encoded, postcard::to_allocvec(&value).unwrap(), "{value:?}");
                assert_eq!(<Aggregate as Message>::decode(&encoded).unwrap(), value);
                assert_eq!(
                    <Aggregate as Message>::decoding_memory(&encoded).unwrap(),
                    value.memory_size()
                );
            }
            assert!(<Aggregate as Message>::decode(&[9]).is_err());
            assert!(<Aggregate as Message>::decode(&[8]).is_err());
        }

#[test]
fn aggregate_json_unchanged() {
            assert_eq!(
                serde_json::to_string(&Aggregate::Count).unwrap(),
                "\"count\""
            );
            assert_eq!(
                serde_json::to_string(&Aggregate::Percentile(95)).unwrap(),
                "{\"percentile\":95}"
            );
            assert_eq!(Aggregate::Percentile(95).name(), "p95");
            assert_eq!(Aggregate::parse("p95"), Some(Aggregate::Percentile(95)));
        }

#[test]
fn aggregate_message_field() {
            let value = Summary {
                aggregate: Aggregate::Percentile(95),
                optional: Some(Aggregate::Rms),
                aggregates: EVERY.to_vec(),
            };
            let encoded = Protobuf::encode_to_vec(&value);
            assert!(<Summary as Protobuf>::decode(&encoded).unwrap() == value);
            let encoded = Message::encode_to_vec(&value);
            assert!(<Summary as Message>::decode(&encoded).unwrap() == value);
            let empty = <Summary as Protobuf>::decode(&[]).unwrap();
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
                let (
                    Kind::Message { definition },
                    Kind::Message {
                        definition: expected,
                    },
                ) = (&field.kind, &derived.kind)
                else {
                    panic!("variant payloads are messages");
                };
                assert_eq!((definition.fields)(), (expected.fields)());
            }
            let percentile = Aggregate::FIELDS
                .iter()
                .find(|field| field.name == "Percentile")
                .unwrap();
            let Kind::Message { definition } = &percentile.kind else {
                panic!("percentile payload");
            };
            let rank = &(definition.fields)()[0];
            assert_eq!((rank.name.as_ref(), rank.kind), ("rank", Kind::U32));
            let settings = SchemaSettings::default();
            let left = Schema::from_declaration(Aggregate::DEFINITION, &settings).unwrap();
            let right = Schema::from_declaration(twin::Aggregate::DEFINITION, &settings).unwrap();
            assert!(
                left.equivalent(left.root, &right, right.root, &settings)
                    .unwrap()
            );
            Schema::from_declaration(Summary::DEFINITION, &settings).unwrap();
        }

#[cfg(feature = "grpc")]
#[test]
fn aggregate_reflects_oneof() {
            use harmos_packaging::proto::reflection::DynamicMessage;
            let schema = crate::schema_descriptor(Aggregate::DEFINITION);
            assert_eq!(schema.oneofs().count(), 1);
            for value in EVERY {
                let encoded = Protobuf::encode_to_vec(&value);
                let reflected = DynamicMessage::decode(schema.clone(), encoded.as_slice()).unwrap();
                assert_eq!(prost::Message::encode_to_vec(&reflected), encoded);
            }
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
        Aggregate::Percentile(rank) => twin::Aggregate::Percentile {
            rank: u32::from(rank),
        },
    }
}

/* === Tests === */
