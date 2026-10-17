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
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use super::{StorageCursor, StoreHistory};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{Error, Identifier, MemorySize, Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl StoreHistory {
    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        encode_bounded(self, max_bytes)
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        decode_bounded(bytes, max_bytes)
    }
}

impl StorageCursor {
    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        encode_bounded(self, max_bytes)
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        decode_bounded(bytes, max_bytes)
    }
}

crate::postcard_product!(StoreHistory {
    store: Identifier,
    epoch: Identifier
});
crate::postcard_product!(StorageCursor {
    history: StoreHistory,
    sequence: u64
});

impl Described for StoreHistory {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(StoreHistory));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("store"),
            tag: 1,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("epoch"),
            tag: 2,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for StorageCursor {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(StorageCursor));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("history"),
            tag: 1,
            kind: Kind::Message {
                definition: StoreHistory::DEFINITION,
            },
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("sequence"),
            tag: 2,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

pub fn encode_bounded<T: Message>(value: &T, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
    if value.encoded_len() > max_bytes.get() {
        return Err(bound_error());
    }
    Ok(value.encode_to_vec())
}

pub fn decode_bounded<T: Message + MemorySize>(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<T> {
    let required = T::decoding_memory(bytes).map_err(|_| Error::InvalidInput {
        field: "storage native codec",
        reason: "invalid encoding",
    })?;
    if required > max_bytes.get() {
        return Err(bound_error());
    }
    let value = T::decode(bytes).map_err(|_| Error::InvalidInput {
        field: "storage native codec",
        reason: "invalid encoding",
    })?;
    if value.memory_size() > max_bytes.get() {
        return Err(bound_error());
    }
    Ok(value)
}

/* === Private Functions === */
fn bound_error() -> Error {
    Error::InvalidInput {
        field: "storage native codec",
        reason: "value exceeds allowance",
    }
}

/* === Tests === */
