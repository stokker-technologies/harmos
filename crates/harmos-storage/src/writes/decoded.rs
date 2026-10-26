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
use super::DataBatch;
use crate::arrow::ArrowBatch;
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// A Write Batch Whose Arrow IPC Data Has Been Decoded For Admission.
pub struct DecodedBatch {
    pub destination: crate::Identifier,
    pub schema: crate::storage::SchemaKey,
    pub data: ArrowBatch,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl DecodedBatch {
    pub fn of(batch: DataBatch) -> Result<Self> {
        Ok(Self { data: ArrowBatch::from_ipc(batch.data.into_bytes())?, destination: batch.destination, schema: batch.schema })
    }
}

impl MemorySize for DecodedBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.destination.memory_size().saturating_sub(std::mem::size_of_val(&self.destination)))
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of_val(&self.schema)))
            .saturating_add(self.data.memory_size().saturating_sub(std::mem::size_of_val(&self.data)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
