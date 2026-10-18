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
use super::{RecordInput, StoredRecord, StoredRow, StreamInput};
use crate::model::storage::codec::{Bytes, Id, Nested, Optional, Time, Unsigned, Version, product};
use crate::model::storage::{SchemaKey, StorageCursor};

/* === Constants === */

/* === Macros === */
product!(SchemaKey { id: Id => 1, version: Version => 2, payload_encoding: Encoding => 3 });
product!(RecordInput {
    schema: Nested<SchemaKey> => 1,
    scope: Optional<Id> => 2,
    item: Id => 3,
    source: Optional<Id> => 4,
    source_time: Optional<Time> => 5,
    payload: Bytes => 6,
});
product!(StreamInput {
    record: Nested<RecordInput> => 1,
    stream: Id => 2,
    sequence: Unsigned => 3,
});
product!(StoredRecord<Vec<u8>> {
    position: Nested<StorageCursor> => 1,
    schema: Nested<SchemaKey> => 2,
    scope: Optional<Id> => 3,
    item: Id => 4,
    source: Optional<Id> => 5,
    source_time: Optional<Time> => 6,
    payload: Bytes => 7,
});
product!(StoredRow<Vec<u8>> {
    record: Nested<StoredRecord<Vec<u8>>> => 1,
    stream: Id => 2,
    sequence: Unsigned => 3,
});

/* === Type Aliases === */

/* === Structs === */
struct Encoding;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::model::storage::codec::Value for Encoding {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Encoding));
    type Native = crate::model::storage::PayloadEncoding;
    const KIND: crate::schema::declaration::Kind = crate::schema::declaration::Kind::U64;
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(&(*value as u64), bytes);
    }

    fn postcard_size(_: &Self::Native) -> usize {
        1
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize, crate::MessageError> {
        Self::postcard_read(input).map(|_| 0)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Self::Native, crate::MessageError> {
        encoding(input.value()?)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn encoding(value: u64) -> Result<crate::model::storage::PayloadEncoding, crate::MessageError> {
    match value {
        0 => Ok(crate::model::storage::PayloadEncoding::ProtobufV1),
        1 => Ok(crate::model::storage::PayloadEncoding::PostcardV1),
        _ => Err(crate::MessageError::borrowed("unknown persisted payload encoding")),
    }
}

/* === Tests === */
