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
use std::borrow::Cow;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::message::{PostcardReader, PostcardValue};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described, declaration};
use crate::{MemorySize, MessageError, RowMetadata};

/* === Constants === */
// Protobuf variant tags, hashed from each authored variant name exactly as
// `#[harmos::message]` derives them; the message tests pin them against a
// derived twin, so the hand-written codec and the derive cannot drift apart.
const COUNT: u32 = 31_971_292;
const SUM: u32 = 534_305_306;
const MEAN: u32 = 96_322_653;
const MEDIAN: u32 = 148_660_223;
const MIN: u32 = 244_600_265;
const MAX: u32 = 480_766_859;
const RMS: u32 = 43_272_471;
const STD: u32 = 450_256_219;
const PERCENTILE: u32 = 455_936_439;
const RANK: u32 = 211_531_165;

// Variant tags in declaration order, indexed by the Postcard ordinal.
crate::with_protobuf! {
    const TAGS: [u32; 9] = [COUNT, SUM, MEAN, MEDIAN, MIN, MAX, RMS, STD, PERCENTILE];
}

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The percentile variant payload: one `rank` field carrying the percentage.
struct Rank;

/* === Enums === */
// The one statistic vocabulary every Harmos reduction answers: plot buckets,
// storage group-bys and stream batch measurements name what they compute with
// these members, and `harmos::signal` holds the math behind each of them.
// `Std` is the population standard deviation. `Percentile` carries a whole
// percentage, 0 to 100, answered by linear interpolation between ranks.
//
// A message field of this type travels as a Harmos enum message, the shape
// `#[harmos::message]` gives a data-carrying enum: one protobuf oneof whose
// members are the PascalCase variant names, each holding a payload message.
// Unit members carry an empty payload; `Percentile` carries `rank` (uint32,
// refused above 255 on decode). An absent member decodes as `Count`. Postcard
// writes the declaration ordinal, then the rank byte for `Percentile`, the
// same bytes serde's Postcard form produces.
//
// JSON has two spellings, one per codec:
// - serde: unit members are snake case strings (`"count"`), a percentile is
//   `{"percentile": 95}`.
// - protobuf JSON (Python `MessageToJson`, gRPC gateways):
//   `{"Count": {}}`, `{"Percentile": {"rank": 95}}`.
// `name()` and `parse()` keep the compact display spelling (`"p95"`) for
// labels and text inputs; it is never a wire form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregate {
    Count,
    Sum,
    Mean,
    Median,
    Min,
    Max,
    Rms,
    Std,
    Percentile(u8),
}

/* === Traits === */

/* === Implementations === */
impl Aggregate {
    // The wire name: the member in snake case, a percentile as `p` and its rank.
    pub fn name(&self) -> String {
        match self {
            Self::Count => "count".to_owned(),
            Self::Sum => "sum".to_owned(),
            Self::Mean => "mean".to_owned(),
            Self::Median => "median".to_owned(),
            Self::Min => "min".to_owned(),
            Self::Max => "max".to_owned(),
            Self::Rms => "rms".to_owned(),
            Self::Std => "std".to_owned(),
            Self::Percentile(rank) => format!("p{rank}"),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let named = match name {
            "count" => Self::Count,
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "median" => Self::Median,
            "min" => Self::Min,
            "max" => Self::Max,
            "rms" => Self::Rms,
            "std" => Self::Std,
            _ => {
                let rank = name.strip_prefix('p')?;
                if rank.is_empty() || !rank.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                Self::Percentile(rank.parse().ok()?)
            }
        };

        named.answerable().then_some(named)
    }

    // A percentile rank is a percentage; every other member always answers.
    pub fn answerable(&self) -> bool {
        match self {
            Self::Percentile(rank) => *rank <= 100,
            _ => true,
        }
    }

    // Whether a running summary of count, sum, extremes and spread answers this
    // member, so partial answers merge without the values. Median and
    // percentiles need the ordered values themselves.
    pub fn mergeable(&self) -> bool {
        !matches!(self, Self::Median | Self::Percentile(_))
    }

    fn ordinal(&self) -> u32 {
        match self {
            Self::Count => 0,
            Self::Sum => 1,
            Self::Mean => 2,
            Self::Median => 3,
            Self::Min => 4,
            Self::Max => 5,
            Self::Rms => 6,
            Self::Std => 7,
            Self::Percentile(_) => 8,
        }
    }

    fn unit(ordinal: u32) -> Option<Self> {
        Some(match ordinal {
            0 => Self::Count,
            1 => Self::Sum,
            2 => Self::Mean,
            3 => Self::Median,
            4 => Self::Min,
            5 => Self::Max,
            6 => Self::Rms,
            7 => Self::Std,
            _ => return None,
        })
    }

    fn read_postcard(input: &mut PostcardReader<'_>) -> Result<Self, MessageError> {
        let ordinal: u32 = input.value()?;
        if ordinal == 8 {
            return Ok(Self::Percentile(u8::read(input)?));
        }
        Self::unit(ordinal).ok_or(MessageError::borrowed("unknown Postcard enum variant"))
    }
}

impl Described for Aggregate {
    const NAME: &'static str = "harmos::Aggregate";
    const FIELDS: &'static [Field] = &[
        variant("Count", COUNT, <()>::DEFINITION),
        variant("Sum", SUM, <()>::DEFINITION),
        variant("Mean", MEAN, <()>::DEFINITION),
        variant("Median", MEDIAN, <()>::DEFINITION),
        variant("Min", MIN, <()>::DEFINITION),
        variant("Max", MAX, <()>::DEFINITION),
        variant("Rms", RMS, <()>::DEFINITION),
        variant("Std", STD, <()>::DEFINITION),
        variant("Percentile", PERCENTILE, Rank::DEFINITION),
    ];
    const MARKER: &'static [u8] = &declaration::encode::<
        { declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for Rank {
    const NAME: &'static str = "harmos::Aggregate::Percentile";
    const FIELDS: &'static [Field] = &[Field {
        name: Cow::Borrowed("rank"),
        tag: RANK,
        kind: Kind::U32,
        cardinality: Cardinality::Required,
    }];
    const MARKER: &'static [u8] = &declaration::encode::<
        { declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl PostcardValue for Aggregate {
    fn write(&self, bytes: &mut Vec<u8>) {
        self.ordinal().write(bytes);
        if let Self::Percentile(rank) = self {
            rank.write(bytes);
        }
    }

    fn size(&self) -> usize {
        let rank = usize::from(matches!(self, Self::Percentile(_)));
        self.ordinal().size().saturating_add(rank)
    }

    fn memory(input: &mut PostcardReader<'_>) -> Result<usize, MessageError> {
        input.nested(|input| {
            Self::read_postcard(input)?;
            Ok(std::mem::size_of::<Self>())
        })
    }

    fn read(input: &mut PostcardReader<'_>) -> Result<Self, MessageError> {
        input.nested(Self::read_postcard)
    }
}

crate::with_protobuf! {
    impl crate::Protobuf for Aggregate {
        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(crate::Protobuf::encoded_len(self));
            crate::Protobuf::encode_into(self, &mut bytes);
            bytes
        }

        fn encode_into(&self, bytes: &mut Vec<u8>) {
            use prost::encoding::{self, WireType};
            let tag = TAGS[self.ordinal() as usize];
            encoding::encode_key(tag, WireType::LengthDelimited, bytes);
            encoding::encode_varint(protobuf_payload_len(self) as u64, bytes);
            if let Self::Percentile(rank @ 1..) = self {
                encoding::uint32::encode(RANK, &u32::from(*rank), bytes);
            }
        }

        fn encoded_len(&self) -> usize {
            use prost::encoding;
            let length = protobuf_payload_len(self);
            encoding::key_len(TAGS[self.ordinal() as usize])
                .saturating_add(encoding::encoded_len_varint(length as u64))
                .saturating_add(length)
        }

        fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
            Self::decoding_memory_input(crate::message::DecodeInput::new(bytes))
        }

        fn decoding_memory_input(
            input: crate::message::DecodeInput<'_>,
        ) -> Result<usize, MessageError> {
            Self::decode_input(input)?;
            Ok(std::mem::size_of::<Self>())
        }

        fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
            Self::decode_input(crate::message::DecodeInput::new(bytes))
        }

        // Unknown members refuse the body, an absent member is `Count`, and the
        // last rank occurrence wins, matching derived enum messages.
        fn decode_input(input: crate::message::DecodeInput<'_>) -> Result<Self, MessageError> {
            use crate::message::{DecodeScalar, DecodeValue};
            let Some((tag, selected)) = input.variant(&TAGS)? else {
                return Ok(Self::Count);
            };
            let ordinal = TAGS.iter().position(|known| *known == tag);
            let ordinal = ordinal.expect("selected a declared variant") as u32;
            if let Some(unit) = Self::unit(ordinal) {
                return Ok(unit);
            }
            let mut rank = 0;
            selected.fields(&mut |field| {
                if field.tag != RANK {
                    return Ok(());
                }
                let DecodeValue::Varint(value) = field.scalar(DecodeScalar::Varint)? else {
                    unreachable!("validated varint wire type");
                };
                rank = u8::try_from(value)
                    .map_err(|_| MessageError::borrowed("percentile rank exceeds 255"))?;
                Ok(())
            })?;
            Ok(Self::Percentile(rank))
        }
    }
}

impl MemorySize for Aggregate {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl RowMetadata for Aggregate {}

/* === Public Functions === */

/* === Private Functions === */
const fn variant(name: &'static str, tag: u32, payload: declaration::Definition) -> Field {
    Field {
        name: Cow::Borrowed(name),
        tag,
        kind: Kind::Message {
            definition: payload,
        },
        cardinality: Cardinality::Variant,
    }
}

// A zero rank is the proto3 default and stays off the wire.
crate::with_protobuf! {
    fn protobuf_payload_len(value: &Aggregate) -> usize {
        match value {
            Aggregate::Percentile(rank @ 1..) => {
                prost::encoding::uint32::encoded_len(RANK, &u32::from(*rank))
            }
            _ => 0,
        }
    }
}

/* === Tests === */
