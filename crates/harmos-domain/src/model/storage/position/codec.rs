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
#[cfg(feature = "grpc")]
use crate::__private::{DecodeInput, DecodeScalar, DecodeValue, decoding_add};
#[cfg(feature = "grpc")]
use crate::MessageError;
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

#[cfg(feature = "grpc")]
const _: () = {
    use crate::__private::Protobuf as Message;
    impl Message for StoreHistory {
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            write_history(self, bytes);
        }

        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(Message::encoded_len(self));
            write_history(self, &mut bytes);
            bytes
        }

        fn encoded_len(&self) -> usize {
            field_len(self.store.as_str().len())
                .saturating_add(field_len(self.epoch.as_str().len()))
        }

        fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
            <Self as Message>::decode_input(DecodeInput::new(bytes))
        }

        fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
            <Self as Message>::decoding_memory_input(DecodeInput::new(bytes))
        }

        fn decoding_memory_input(
            input: DecodeInput<'_>,
        ) -> std::result::Result<usize, MessageError> {
            let store = text_field(input, 1)?.unwrap_or("");
            let epoch = text_field(input, 2)?.unwrap_or("");
            decoding_add(
                decoding_add(std::mem::size_of::<Self>(), store.len())?,
                epoch.len(),
            )
        }

        fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
            let store = identifier(text_field(input, 1)?.unwrap_or(""))?;
            let epoch = identifier(text_field(input, 2)?.unwrap_or(""))?;
            Ok(Self { store, epoch })
        }
    }

    impl Message for StorageCursor {
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            write_cursor(self, bytes);
        }

        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(Message::encoded_len(self));
            write_cursor(self, &mut bytes);
            bytes
        }

        fn encoded_len(&self) -> usize {
            use crate::__private::prost::encoding::encoded_len_varint;
            field_len(Message::encoded_len(&self.history))
                .saturating_add(1 + encoded_len_varint(self.sequence))
        }

        fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
            <Self as Message>::decode_input(DecodeInput::new(bytes))
        }

        fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
            <Self as Message>::decoding_memory_input(DecodeInput::new(bytes))
        }

        fn decoding_memory_input(
            input: DecodeInput<'_>,
        ) -> std::result::Result<usize, MessageError> {
            bytes_field(input, 1)?
                .ok_or_else(|| MessageError::borrowed("missing cursor history"))?;
            uint_field(input, 2)?
                .ok_or_else(|| MessageError::borrowed("missing cursor sequence"))?;
            let history = <StoreHistory as Message>::decoding_memory_input(input.field(1)?)?;
            decoding_add(std::mem::size_of::<Self>(), history)
        }

        fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
            bytes_field(input, 1)?
                .ok_or_else(|| MessageError::borrowed("missing cursor history"))?;
            let sequence = uint_field(input, 2)?
                .ok_or_else(|| MessageError::borrowed("missing cursor sequence"))?;
            let history = <StoreHistory as Message>::decode_input(input.field(1)?)?;
            Ok(Self { history, sequence })
        }
    }
};

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
#[cfg(feature = "grpc")]
pub fn field_len(length: usize) -> usize {
    use crate::__private::prost::encoding::encoded_len_varint;
    1usize
        .saturating_add(encoded_len_varint(length as u64))
        .saturating_add(length)
}

#[cfg(feature = "grpc")]
pub fn write_header(tag: u32, length: usize, bytes: &mut Vec<u8>) {
    use crate::__private::prost::encoding::{WireType, encode_key, encode_varint};
    encode_key(tag, WireType::LengthDelimited, bytes);
    encode_varint(length as u64, bytes);
}

#[cfg(feature = "grpc")]
pub fn write_bytes(tag: u32, value: &[u8], bytes: &mut Vec<u8>) {
    write_header(tag, value.len(), bytes);
    bytes.extend_from_slice(value);
}

#[cfg(feature = "grpc")]
pub fn write_history(history: &StoreHistory, bytes: &mut Vec<u8>) {
    write_bytes(1, history.store.as_str().as_bytes(), bytes);
    write_bytes(2, history.epoch.as_str().as_bytes(), bytes);
}

#[cfg(feature = "grpc")]
pub fn write_cursor(cursor: &StorageCursor, bytes: &mut Vec<u8>) {
    write_header(
        1,
        crate::__private::Protobuf::encoded_len(&cursor.history),
        bytes,
    );
    write_history(&cursor.history, bytes);
    crate::__private::prost::encoding::uint64::encode(2, &cursor.sequence, bytes);
}

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

#[cfg(feature = "grpc")]
pub fn text_field(
    input: DecodeInput<'_>,
    tag: u32,
) -> std::result::Result<Option<&str>, MessageError> {
    let mut value = None;
    input.fields(&mut |field| {
        if field.tag == tag {
            value = Some(field.text()?);
        }
        Ok(())
    })?;
    Ok(value)
}

#[cfg(feature = "grpc")]
pub fn bytes_field(
    input: DecodeInput<'_>,
    tag: u32,
) -> std::result::Result<Option<&[u8]>, MessageError> {
    let mut value = None;
    input.fields(&mut |field| {
        if field.tag == tag {
            value = Some(field.bytes()?);
        }
        Ok(())
    })?;
    Ok(value)
}

#[cfg(feature = "grpc")]
pub fn uint_field(
    input: DecodeInput<'_>,
    tag: u32,
) -> std::result::Result<Option<u64>, MessageError> {
    let mut value = None;
    input.fields(&mut |field| {
        if field.tag == tag {
            let DecodeValue::Varint(number) = field.scalar(DecodeScalar::Varint)? else {
                unreachable!("validated varint");
            };
            value = Some(number);
        }
        Ok(())
    })?;
    Ok(value)
}

#[cfg(feature = "grpc")]
pub fn identifier(value: &str) -> std::result::Result<Identifier, MessageError> {
    Identifier::new(Box::<str>::from(value).into_string())
        .map_err(|_| MessageError::borrowed("invalid storage identifier"))
}

/* === Private Functions === */
fn bound_error() -> Error {
    Error::InvalidInput {
        field: "storage native codec",
        reason: "value exceeds allowance",
    }
}

/* === Tests === */
