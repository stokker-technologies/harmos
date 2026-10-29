/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::{ObjectEntry, ObjectReceipt, ObjectVersionToken};
use crate::MessageError;
use crate::model::storage::codec::{Bytes, Nested, Value, product};
use crate::model::storage::{ObjectKey, ObjectValue, SchemaKey, StorageCursor};

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

type Result<T> = std::result::Result<T, MessageError>;

pub struct Key;

impl Value for Key {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Key));
    crate::model::storage::codec::postcard_field!();
    type Native = ObjectKey;
    const KIND: crate::schema::declaration::Kind = crate::schema::declaration::Kind::String;
}

impl crate::__private::PostcardValue for ObjectKey {
    fn write(&self, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(&self.as_str().len(), bytes);
        bytes.extend_from_slice(self.as_str().as_bytes());
    }

    fn size(&self) -> usize {
        self.as_str().len().saturating_add(crate::__private::PostcardValue::size(&self.as_str().len()))
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
        Self::from_owned(Box::<str>::from(text).into_string()).map_err(|_| MessageError::borrowed("empty object key"))
    }
}
