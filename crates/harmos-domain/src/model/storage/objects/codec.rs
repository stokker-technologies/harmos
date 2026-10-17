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

// Third Party Imports

// Local Imports
use super::{ObjectEntry, ObjectReceipt, ObjectVersionToken};
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
use crate::MessageError;
use crate::model::storage::codec::{Bytes, Nested, Value, product};
#[cfg(feature = "grpc")]
use crate::model::storage::position::codec::{field_len, text_field, write_bytes};
use crate::model::storage::{ObjectKey, ObjectValue, SchemaKey, StorageCursor};

/* === Constants === */

/* === Macros === */
product!(ObjectValue {
    schema: Nested<SchemaKey> => 1,
    item: Key => 2,
    payload: Bytes => 3,
});
product!(ObjectEntry {
    position: Nested<StorageCursor> => 1,
    version: Nested<ObjectVersionToken> => 2,
    value: Nested<ObjectValue> => 3,
});
product!(ObjectReceipt {
    position: Nested<StorageCursor> => 1,
    version: Nested<ObjectVersionToken> => 2,
});

/* === Type Aliases === */
type Result<T> = std::result::Result<T, MessageError>;

/* === Structs === */
pub struct Key;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Value for Key {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Key));
    crate::model::storage::codec::postcard_field!();
    type Native = ObjectKey;
    const KIND: crate::schema::declaration::Kind = crate::schema::declaration::Kind::String;
    #[cfg(feature = "grpc")]
    fn write(value: &ObjectKey, tag: u32, bytes: &mut Vec<u8>) {
        write_bytes(tag, value.as_str().as_bytes(), bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &ObjectKey, tag: u32) -> usize {
        field_len(value.as_str().len())
            .saturating_add(crate::__private::prost::encoding::key_len(tag) - 1)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> Result<usize> {
        let text = text_field(input, tag)?.unwrap_or("");
        if text.is_empty() {
            return Err(MessageError::borrowed("empty object key"));
        }
        Ok(text.len())
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> Result<ObjectKey> {
        ObjectKey::from_owned(text_field(input, tag)?.unwrap_or("").into())
            .map_err(|_| MessageError::borrowed("empty object key"))
    }
}

impl crate::__private::PostcardValue for ObjectKey {
    fn write(&self, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(&self.as_str().len(), bytes);
        bytes.extend_from_slice(self.as_str().as_bytes());
    }

    fn size(&self) -> usize {
        self.as_str()
            .len()
            .saturating_add(crate::__private::PostcardValue::size(&self.as_str().len()))
    }

    fn memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize> {
        let text: &str = input.value()?;
        if text.is_empty() {
            return Err(MessageError::borrowed("empty object key"));
        }
        crate::__private::decoding_add(std::mem::size_of::<Self>(), text.len())
    }

    fn read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Self> {
        let text: &str = input.value()?;
        if text.is_empty() {
            return Err(MessageError::borrowed("empty object key"));
        }
        Self::from_owned(Box::<str>::from(text).into_string())
            .map_err(|_| MessageError::borrowed("empty object key"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
