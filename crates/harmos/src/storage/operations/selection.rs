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
#[cfg(feature = "grpc")]
use crate::__private::{DecodeInput, decoding_add};

#[cfg(feature = "grpc")]
use crate::MessageError;
use crate::schema::{Cardinality, Described, Field, Kind};
use crate::storage::StorageCursor;
#[cfg(feature = "grpc")]
use crate::storage::position::codec::{
    bytes_field, field_len, identifier, text_field, uint_field, write_bytes, write_cursor,
    write_header,
};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub after: Option<StorageCursor>,
    pub through: Option<u64>,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub page_bytes: u64,
    pub page_rows: u64,
    pub descending: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for Selection {
    fn default() -> Self {
        Self {
            after: None,
            through: None,
            scope: None,
            item: None,
            page_bytes: 64 * 1024,
            page_rows: 128,
            descending: false,
        }
    }
}

impl Selection {
    pub fn validate(&self) -> Result<()> {
        let bytes = usize::try_from(self.page_bytes)
            .ok()
            .and_then(NonZeroUsize::new);
        let rows = usize::try_from(self.page_rows)
            .ok()
            .and_then(NonZeroUsize::new);
        if bytes.is_none()
            || rows.is_none()
            || self.through.is_some_and(|end| {
                self.after
                    .as_ref()
                    .is_some_and(|start| start.sequence > end)
            })
        {
            return Err(Error::InvalidInput {
                field: "storage selection",
                reason: "invalid page bounds or reversed position range",
            });
        }
        Ok(())
    }

    pub(super) fn options(&self) -> Result<crate::storage::QueryOptions> {
        self.validate()?;
        Ok(crate::storage::QueryOptions {
            after: self.after.as_ref().map_or(0, |cursor| cursor.sequence),
            through: self.through,
            order: if self.descending {
                crate::storage::ReadOrder::Descending
            } else {
                crate::storage::ReadOrder::Ascending
            },
            page: Some(crate::storage::ReadLimits {
                rows: NonZeroUsize::new(self.page_rows as usize).expect("validated rows"),
                bytes: NonZeroUsize::new(self.page_bytes as usize).expect("validated bytes"),
            }),
            ..Default::default()
        })
    }
}

impl MemorySize for Selection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.after
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.after)),
            )
            .saturating_add(
                self.scope
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.scope)),
            )
            .saturating_add(
                self.item
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.item)),
            )
    }
}

harmos_contract::postcard_product!(Selection {
    after: Option<StorageCursor>, through: Option<u64>, scope: Option<Identifier>,
    item: Option<Identifier>, page_bytes: u64, page_rows: u64, descending: bool,
}, validate = |value: &Selection| value.validate().map_err(|_| crate::MessageError::borrowed("invalid storage selection")));

#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for Selection {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
        bytes
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        if let Some(after) = &self.after {
            write_header(1, crate::__private::Protobuf::encoded_len(after), bytes);
            write_cursor(after, bytes);
        }
        if let Some(through) = self.through {
            crate::__private::prost::encoding::uint64::encode(2, &through, bytes);
        }
        if let Some(scope) = &self.scope {
            write_bytes(3, scope.as_str().as_bytes(), bytes);
        }
        if let Some(item) = &self.item {
            write_bytes(4, item.as_str().as_bytes(), bytes);
        }
        crate::__private::prost::encoding::uint64::encode(5, &self.page_bytes, bytes);
        crate::__private::prost::encoding::uint64::encode(6, &self.page_rows, bytes);
        crate::__private::prost::encoding::bool::encode(7, &self.descending, bytes);
    }

    fn encoded_len(&self) -> usize {
        use crate::__private::prost::encoding::encoded_len_varint;
        self.after
            .as_ref()
            .map_or(0, |cursor| {
                field_len(crate::__private::Protobuf::encoded_len(cursor))
            })
            .saturating_add(
                self.through
                    .map_or(0, |value| 1 + encoded_len_varint(value)),
            )
            .saturating_add(
                self.scope
                    .as_ref()
                    .map_or(0, |value| field_len(value.as_str().len())),
            )
            .saturating_add(
                self.item
                    .as_ref()
                    .map_or(0, |value| field_len(value.as_str().len())),
            )
            .saturating_add(1 + encoded_len_varint(self.page_bytes))
            .saturating_add(1 + encoded_len_varint(self.page_rows))
            .saturating_add(2)
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> std::result::Result<usize, MessageError> {
        let mut memory = std::mem::size_of::<Self>();
        if bytes_field(input, 1)?.is_some() {
            memory = decoding_add(
                memory,
                StorageCursor::decoding_memory_input(input.field(1)?)?,
            )?;
        }
        for tag in [3, 4] {
            memory = decoding_add(memory, text_field(input, tag)?.map_or(0, str::len))?;
        }
        for tag in [2, 5, 6, 7] {
            uint_field(input, tag)?;
        }
        Ok(memory)
    }

    fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
        let cursor = input.field(1)?;
        let after = if bytes_field(input, 1)?.is_some() {
            Some(StorageCursor::decode_input(cursor)?)
        } else {
            None
        };
        let value = Self {
            after,
            through: uint_field(input, 2)?,
            scope: text_field(input, 3)?.map(identifier).transpose()?,
            item: text_field(input, 4)?.map(identifier).transpose()?,
            page_bytes: uint_field(input, 5)?.unwrap_or(0),
            page_rows: uint_field(input, 6)?.unwrap_or(0),
            descending: uint_field(input, 7)?.unwrap_or(0) != 0,
        };
        value
            .validate()
            .map_err(|_| MessageError::borrowed("invalid storage selection"))?;
        Ok(value)
    }
}

impl Described for Selection {
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("after"),
            tag: 1,
            kind: Kind::Message {
                fields: Cow::Borrowed(StorageCursor::FIELDS),
            },
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("through"),
            tag: 2,
            kind: Kind::U64,
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("scope"),
            tag: 3,
            kind: Kind::String,
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("item"),
            tag: 4,
            kind: Kind::String,
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("page_bytes"),
            tag: 5,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("page_rows"),
            tag: 6,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("descending"),
            tag: 7,
            kind: Kind::Bool,
            cardinality: Cardinality::Required,
        },
    ];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
