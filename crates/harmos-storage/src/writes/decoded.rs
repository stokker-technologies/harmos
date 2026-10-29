/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::DataBatch;
use crate::Result;
use crate::arrow::ArrowBatch;

// A Write Batch Whose Arrow IPC Data Has Been Decoded For Admission.
pub struct DecodedBatch {
    pub destination: crate::Identifier,
    pub schema: crate::storage::SchemaKey,
    pub data: ArrowBatch,
}

impl DecodedBatch {
    pub fn of(batch: DataBatch) -> Result<Self> {
        Ok(Self {
            data: ArrowBatch::from_ipc(batch.data.into_bytes())?,
            destination: batch.destination,
            schema: batch.schema,
        })
    }
}
