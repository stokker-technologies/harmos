/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Admit Immutable Sample Columns Without Encoding Row Payloads.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::OnceLock;

// Third Party Imports
use arrow_array::{Array, ListArray, StringArray, StructArray, UInt32Array, UInt64Array};
use arrow_schema::{DataType, TimeUnit};
use sha2::{Digest, Sha256};

// Local Imports
use super::DecodedBatch;
use crate::{
    Error, Identifier, MemorySize, MutationHeader, MutationReceipt, MutationValue, Result, StorageCursor, StorageSchema, StoredRow,
};
use harmos_query::source;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// One Immutable Columnar Append; Positions Are Assigned in Arrow Row Order.
/// The Backend Must Commit the Entire Batch and Its Mutation Receipt Atomically.
#[derive(Clone)]
pub struct SampleAppend {
    pub first: StorageCursor,
    pub through: StorageCursor,
    pub scope: Option<Identifier>,
    pub batch: DecodedBatch,
    pub schema: StorageSchema,
    pub(crate) header: OnceLock<MutationHeader>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl SampleAppend {
    pub fn mutation(&self) -> &MutationHeader {
        self.header.get().expect("admitted sample append owns its mutation header")
    }

    pub fn receipt(&self) -> MutationReceipt {
        let header = self.mutation();
        MutationReceipt {
            mutation: header.mutation.clone(),
            fingerprint: header.fingerprint,
            expires_at: header.expires_at,
            value: MutationValue::Cursor(self.through.clone()),
        }
    }

    pub fn replay(&self, receipt: &MutationReceipt) -> Result<()> {
        if *receipt != self.receipt() {
            return Err(invalid("replay the unchanged sample batch with its original mutation identity"));
        }
        Ok(())
    }

    pub fn retained_bytes(&self) -> usize {
        self.batch
            .data
            .memory_size()
            .saturating_add(self.scope.memory_size())
            .saturating_add(self.batch.destination.memory_size())
            .saturating_add(self.batch.schema.memory_size())
            .saturating_add(self.first.memory_size())
            .saturating_add(self.through.memory_size())
            .saturating_add(self.schema.memory_size())
            .saturating_add(size_of::<Self>())
    }

    pub(crate) fn fingerprint(scope: Option<&Identifier>, batch: &DecodedBatch) -> [u8; 32] {
        let mut hash = Sha256::new();
        for bytes in [
            b"storage.sample_columns:v1".as_slice(),
            batch.destination.as_str().as_bytes(),
            batch.schema.id.as_str().as_bytes(),
            &batch.schema.version.get().to_le_bytes(),
            scope.map_or(b"".as_slice(), |scope| scope.as_str().as_bytes()),
            batch.data.ipc_bytes(),
        ] {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.finalize().into()
    }

    pub(crate) fn validate(scope: Option<&Identifier>, batch: &DecodedBatch, schema: &StorageSchema) -> Result<()> {
        let data = batch.data.as_record_batch();
        if data.num_rows() == 0 {
            return Err(invalid("submit a nonempty sample batch"));
        }
        let metadata = data
            .column_by_name("_storage")
            .and_then(|value| value.as_any().downcast_ref::<StructArray>())
            .ok_or_else(|| invalid("supply sample metadata as a struct column"))?;
        if metadata.null_count() != 0 {
            return Err(invalid("supply nonnull sample metadata"));
        }
        let sequence = metadata
            .column_by_name("sequence")
            .and_then(|value| value.as_any().downcast_ref::<UInt64Array>())
            .ok_or_else(|| invalid("supply a UInt64 sample sequence column"))?;
        if sequence.null_count() != 0 || sequence.values().windows(2).any(|pair| pair[0].checked_add(1) != Some(pair[1])) {
            return Err(invalid("supply consecutive nonnull sample sequences in Arrow row order"));
        }
        for name in ["item", "scope", "source", "stream"] {
            let Some(column) = metadata.column_by_name(name) else {
                if name == "item" {
                    return Err(invalid("supply a sample item column"));
                }
                continue;
            };
            let column = column.as_any().downcast_ref::<StringArray>().ok_or_else(|| invalid("supply UTF-8 sample identities"))?;
            if name == "item" && column.null_count() != 0 {
                return Err(invalid("supply nonnull sample items"));
            }
            for value in column.iter().flatten() {
                Identifier::validate(value).map_err(|_| invalid("supply valid sample identities"))?;
                if name == "scope" && scope.is_some_and(|scope| scope.as_str() != value) {
                    return Err(invalid("supply sample scopes matching the authorized scope"));
                }
                if name == "stream" && value != batch.destination.as_str() {
                    return Err(invalid("supply sample streams matching the authorized destination"));
                }
            }
        }
        if let Some(time) = metadata.column_by_name("source_time")
            && !matches!(time.data_type(), DataType::Timestamp(TimeUnit::Nanosecond, _))
        {
            return Err(invalid("supply nanosecond source timestamps"));
        }
        let expected = source::payload_fields(&schema.schema)?;
        let payload = data.column_by_name("payload").ok_or_else(|| invalid("supply a typed sample payload column"))?;
        if payload.data_type() != &DataType::Struct(expected.clone()) {
            return Err(invalid("supply sample payload columns matching the registered schema"));
        }
        if payload.null_count() != 0 {
            return Err(invalid("supply nonnull sample payloads"));
        }
        payload.to_data().validate_full().map_err(|_| invalid("supply valid nested sample arrays and nullability"))?;
        validate_variants(payload.as_ref())?;
        Ok(())
    }

    // The Reference Backend Materializes Only When a Row Reader Needs Payload Bytes.
    pub(crate) fn rows(&self) -> Result<Vec<StoredRow>> {
        let records = super::convert::records(self.scope.as_ref(), &self.batch, &self.schema.schema)?;
        let metadata =
            self.batch.data.as_record_batch().column_by_name("_storage").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
        let sequences = metadata.column_by_name("sequence").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
        Ok(records
            .into_iter()
            .enumerate()
            .map(|(index, record)| StoredRow {
                record: record.stored(StorageCursor {
                    history: self.first.history.clone(),
                    sequence: self.first.sequence + index as u64,
                }),
                stream: self.batch.destination.clone(),
                sequence: sequences.value(index),
            })
            .collect())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn validate_variants(array: &dyn Array) -> Result<()> {
    if let Some(values) = array.as_any().downcast_ref::<StructArray>() {
        if values.fields().first().is_some_and(|field| field.name() == "__variant" && !field.metadata().contains_key("harmos.tag"))
            && let Some(discriminant) = values.column_by_name("__variant")
        {
            let discriminant =
                discriminant.as_any().downcast_ref::<UInt32Array>().ok_or_else(|| invalid("supply UInt32 sample variant discriminants"))?;
            for row in 0..values.len() {
                if values.is_null(row) {
                    continue;
                }
                let selected = discriminant.value(row) as usize;
                let variants = &values.columns()[1..];
                if selected >= variants.len()
                    || variants.iter().enumerate().any(|(index, value)| value.is_valid(row) != (index == selected))
                {
                    return Err(invalid("supply exactly the selected sample variant"));
                }
            }
        }
        for column in values.columns() {
            validate_variants(column.as_ref())?;
        }
    }
    if let Some(values) = array.as_any().downcast_ref::<ListArray>() {
        validate_variants(values.values().as_ref())?;
    }
    Ok(())
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "sample columns", reason }
}

/* === Tests === */
