/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use crate::{MemorySize, Result};
use crate::arrow::ArrowBatch;
use super::DataBatch;

/* === Structs === */
// A Write Batch Whose Arrow IPC Data Has Been Decoded For Admission.
pub struct DecodedBatch {
    pub destination: crate::Identifier,
    pub schema: crate::storage::SchemaKey,
    pub data: ArrowBatch,
}

/* === Implementations === */
impl DecodedBatch {
    pub fn of(batch: &DataBatch) -> Result<Self> {
        Ok(Self {
            data: ArrowBatch::from_ipc(batch.data.bytes())?,
            destination: batch.destination.clone(),
            schema: batch.schema.clone(),
        })
    }
}

impl MemorySize for DecodedBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.destination
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.destination)),
            )
            .saturating_add(
                self.schema
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.schema)),
            )
            .saturating_add(
                self.data
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.data)),
            )
    }
}

