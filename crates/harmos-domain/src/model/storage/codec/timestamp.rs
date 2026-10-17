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
use std::time::SystemTime;

// Third Party Imports

// Local Imports
use super::Value;
#[cfg(feature = "grpc")]
use super::length_delimited_size;
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
#[cfg(feature = "grpc")]
use crate::__private::prost::encoding::{bool as boolean, uint32, uint64};
use crate::MessageError;
#[cfg(feature = "grpc")]
use crate::model::storage::position::codec::{bytes_field, uint_field, write_header};
use crate::model::storage::time::Wire;
use crate::schema::{Cardinality, Field, Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Time;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Value for Time {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Time));
    fn postcard_write(value: &SystemTime, bytes: &mut Vec<u8>) {
        use crate::__private::PostcardValue;
        let wire = Wire::from(*value);
        wire.secs_since_epoch.write(bytes);
        wire.nanos_since_epoch.write(bytes);
        wire.before_epoch.write(bytes);
    }

    fn postcard_size(value: &SystemTime) -> usize {
        use crate::__private::PostcardValue;
        let wire = Wire::from(*value);
        wire.secs_since_epoch.size() + wire.nanos_since_epoch.size() + 1
    }

    fn postcard_memory(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }

    fn postcard_read(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> Result<SystemTime, MessageError> {
        input.nested(|input| {
            Wire {
                secs_since_epoch: input.value()?,
                nanos_since_epoch: input.value()?,
                before_epoch: input.value()?,
            }
            .timestamp()
            .map_err(MessageError::borrowed)
        })
    }

    type Native = SystemTime;
    const KIND: Kind = Kind::Message {
        definition: <Self as crate::schema::Described>::DEFINITION,
    };
    #[cfg(feature = "grpc")]
    fn write(value: &SystemTime, tag: u32, bytes: &mut Vec<u8>) {
        let wire = Wire::from(*value);
        write_header(tag, size(&wire), bytes);
        uint64::encode(1, &wire.secs_since_epoch, bytes);
        uint32::encode(2, &wire.nanos_since_epoch, bytes);
        boolean::encode(3, &wire.before_epoch, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &SystemTime, tag: u32) -> usize {
        length_delimited_size(tag, size(&Wire::from(*value)))
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize, MessageError> {
        Self::read(input, tag)?;
        Ok(0)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<SystemTime, MessageError> {
        bytes_field(input, tag)?
            .ok_or_else(|| MessageError::borrowed("missing source timestamp"))?;
        let input = input.field(tag)?;
        let nanos = uint_field(input, 2)?.unwrap_or(0);
        let wire = Wire {
            secs_since_epoch: uint_field(input, 1)?.unwrap_or(0),
            nanos_since_epoch: u32::try_from(nanos)
                .map_err(|_| MessageError::borrowed("invalid source timestamp nanoseconds"))?,
            before_epoch: uint_field(input, 3)?.unwrap_or(0) != 0,
        };
        wire.timestamp().map_err(MessageError::borrowed)
    }
}

impl crate::schema::Described for Time {
    const NAME: &'static str = <Self as Value>::NAME;
    const FIELDS: &'static [Field] = FIELDS;
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        {
            crate::schema::declaration::encoded_len(
                <Self as crate::schema::Described>::NAME,
                Self::FIELDS,
            )
        },
    >(<Self as crate::schema::Described>::NAME, Self::FIELDS);
}

const FIELDS: &[Field] = &[
    Field {
        name: Cow::Borrowed("secs_since_epoch"),
        tag: 1,
        kind: Kind::U64,
        cardinality: Cardinality::Required,
    },
    Field {
        name: Cow::Borrowed("nanos_since_epoch"),
        tag: 2,
        kind: Kind::U32,
        cardinality: Cardinality::Required,
    },
    Field {
        name: Cow::Borrowed("before_epoch"),
        tag: 3,
        kind: Kind::Bool,
        cardinality: Cardinality::Required,
    },
];

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn size(wire: &Wire) -> usize {
    uint64::encoded_len(1, &wire.secs_since_epoch)
        + uint32::encoded_len(2, &wire.nanos_since_epoch)
        + boolean::encoded_len(3, &wire.before_epoch)
}

/* === Tests === */
