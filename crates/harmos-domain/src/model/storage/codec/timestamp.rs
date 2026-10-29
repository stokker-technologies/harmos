/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::borrow::Cow;
use std::time::SystemTime;

use super::Value;
use crate::MessageError;
use crate::model::storage::time::Wire;
use crate::schema::{Cardinality, Field, Kind};

pub struct Time;

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

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<SystemTime, MessageError> {
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
}

impl crate::schema::Described for Time {
    const NAME: &'static str = <Self as Value>::NAME;
    const FIELDS: &'static [Field] = FIELDS;
    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(<Self as crate::schema::Described>::NAME, Self::FIELDS) },
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
