/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Immutable Record Pages

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::{ReadBatch, Result, StoredRecord};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// An immutable retained record with its exact serialized page cost checked once.
#[derive(Clone, Debug)]
pub struct RecordView {
    record: ReadBatch<StoredRecord>,
    encoded_bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl RecordView {
    pub fn new(record: StoredRecord) -> Result<Self> {
        let encoded_bytes = crate::manager::encoded_size(&record)?;
        Ok(Self {
            record: ReadBatch::new(record),
            encoded_bytes,
        })
    }

    pub fn record(&self) -> &StoredRecord {
        &self.record
    }

    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, PayloadEncoding, SchemaKey, StorageCursor, StoreHistory};

    #[test]
    fn retained_records_share_one_row_and_keep_exact_byte_bounds() {
        let record = StoredRecord {
            position: StorageCursor {
                history: StoreHistory {
                    store: Identifier::new("view").unwrap(),
                    epoch: Identifier::new("epoch").unwrap(),
                },
                sequence: u64::MAX,
            },
            schema: SchemaKey {
                id: Identifier::new("sample").unwrap(),
                version: std::num::NonZeroU32::MAX,
                payload_encoding: PayloadEncoding::PostcardV1,
            },
            scope: Some(Identifier::new("scope").unwrap()),
            item: Identifier::new("row").unwrap(),
            source: None,
            source_time: None,
            payload: (0..=255).collect::<Vec<u8>>(),
        };
        let expected = serde_json::to_vec(&record).unwrap().len();
        let view = RecordView::new(record.clone()).unwrap();
        let retained = view.clone();
        assert_eq!(view.encoded_bytes(), expected);
        assert!(std::ptr::eq(view.record(), retained.record()));
        drop(view);
        assert_eq!(retained.record(), &record);
        assert_eq!(retained.encoded_bytes(), expected);
    }
}
