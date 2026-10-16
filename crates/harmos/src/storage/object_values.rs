/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod codec;

/* === Re-Exports === */
pub(crate) use codec::Key;

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::{NonZeroU64, NonZeroUsize};

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
#[cfg(feature = "grpc")]
use crate::__private::{DecodeInput, decoding_add};
use crate::Identifier;
#[cfg(feature = "grpc")]
use crate::MessageError;
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::storage::catalog::invalid;
#[cfg(feature = "grpc")]
use crate::storage::position::codec::{identifier, text_field, uint_field};
use crate::storage::{ObjectKey, ObjectValue, StorageCursor, StoreHistory};
use crate::{MemorySize, Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectEntry {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
    pub value: ObjectValue,
}

#[derive(Clone, Debug)]
pub struct ObjectRevision<T> {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectPoint {
    pub schema: Identifier,
    pub item: ObjectKey,
    pub version: ObjectVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectVersionToken {
    pub(crate) history: StoreHistory,
    pub(crate) revision: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectReceipt {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectVersion {
    Latest,
    Exact(ObjectVersionToken),
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for ObjectEntry {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.position
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
            .saturating_add(
                self.version
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ObjectVersionToken>()),
            )
            .saturating_add(
                self.value
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ObjectValue>()),
            )
    }
}

impl<T: MemorySize> MemorySize for ObjectRevision<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.position
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
            .saturating_add(
                self.version
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ObjectVersionToken>()),
            )
            .saturating_add(
                self.value
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<T>()),
            )
    }
}

impl MemorySize for ObjectReceipt {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.position.memory_size()
            - std::mem::size_of::<StorageCursor>()
            + self.version.memory_size()
            - std::mem::size_of::<ObjectVersionToken>()
    }
}

impl MemorySize for ObjectVersionToken {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.history
                .memory_size()
                .saturating_sub(std::mem::size_of::<StoreHistory>()),
        )
    }
}

impl ObjectVersionToken {
    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        if self.encoded_len() > max_bytes.get() {
            return Err(invalid("object version encoding exceeds allowance"));
        }
        Ok(self.encode_to_vec())
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        let required =
            Self::decoding_memory(bytes).map_err(|_| invalid("invalid object version encoding"))?;
        if required > max_bytes.get() {
            return Err(invalid("object version decoding exceeds allowance"));
        }
        let token = Self::decode(bytes).map_err(|_| invalid("invalid object version encoding"))?;
        if token.memory_size() > max_bytes.get() {
            return Err(invalid("object version decoding exceeds allowance"));
        }
        Ok(token)
    }

    pub(crate) fn new(position: StorageCursor) -> Self {
        Self {
            history: position.history,
            revision: NonZeroU64::new(position.sequence).expect("admission positions are nonzero"),
        }
    }

    pub fn check_position(&self, position: &StorageCursor) -> Result<()> {
        self.history.check(&position.history)?;
        if self.revision.get() != position.sequence {
            return Err(invalid(
                "object version does not match its admission identity",
            ));
        }
        Ok(())
    }
}

harmos_contract::postcard_product!(ObjectVersionToken {
    history: StoreHistory,
    revision: NonZeroU64
});

#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for ObjectVersionToken {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
        bytes
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        use crate::storage::position::codec::write_bytes;
        write_bytes(1, self.history.store.as_str().as_bytes(), bytes);
        write_bytes(2, self.history.epoch.as_str().as_bytes(), bytes);
        crate::__private::prost::encoding::uint64::encode(3, &self.revision.get(), bytes);
    }

    fn encoded_len(&self) -> usize {
        use crate::__private::prost::encoding::encoded_len_varint;
        [self.history.store.as_str(), self.history.epoch.as_str()]
            .into_iter()
            .fold(
                1usize.saturating_add(encoded_len_varint(self.revision.get())),
                |bytes, value| {
                    bytes
                        .saturating_add(1)
                        .saturating_add(encoded_len_varint(value.len() as u64))
                        .saturating_add(value.len())
                },
            )
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> std::result::Result<usize, MessageError> {
        let store = text_field(input, 1)?.unwrap_or("");
        let epoch = text_field(input, 2)?.unwrap_or("");
        NonZeroU64::new(uint_field(input, 3)?.unwrap_or(0))
            .ok_or_else(|| MessageError::borrowed("object revision must be positive"))?;
        decoding_add(
            decoding_add(std::mem::size_of::<Self>(), store.len())?,
            epoch.len(),
        )
    }

    fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
        let store = identifier(text_field(input, 1)?.unwrap_or(""))?;
        let epoch = identifier(text_field(input, 2)?.unwrap_or(""))?;
        let revision = NonZeroU64::new(uint_field(input, 3)?.unwrap_or(0))
            .ok_or_else(|| MessageError::borrowed("object revision must be positive"))?;
        Ok(Self {
            history: StoreHistory { store, epoch },
            revision,
        })
    }
}

impl Described for ObjectVersionToken {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(ObjectVersionToken));
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
        Field {
            name: Cow::Borrowed("revision"),
            tag: 3,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
