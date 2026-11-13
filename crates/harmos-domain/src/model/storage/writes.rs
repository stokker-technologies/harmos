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
use super::{ArrowIpc, SchemaKey};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct DataBatch {
    pub sequence: u64,
    pub destination: Identifier,
    pub schema: SchemaKey,
    pub data: ArrowIpc,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for DataBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.destination.memory_size().saturating_sub(std::mem::size_of_val(&self.destination)))
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of_val(&self.schema)))
            .saturating_add(self.data.memory_size().saturating_sub(std::mem::size_of_val(&self.data)))
    }
}

impl From<DataBatch> for harmos_protocol::storage::DataBatch {
    fn from(value: DataBatch) -> Self {
        Self {
            sequence: value.sequence,
            destination: value.destination.to_string(),
            schema: Some(value.schema.into()),
            ipc: value.data.into_bytes(),
        }
    }
}
impl TryFrom<harmos_protocol::storage::DataBatch> for DataBatch {
    type Error = crate::Error;
    fn try_from(value: harmos_protocol::storage::DataBatch) -> crate::Result<Self> {
        Ok(Self {
            sequence: value.sequence,
            destination: Identifier::new(value.destination)?,
            schema: value.schema.ok_or(crate::Error::InvalidInput { field: "schema", reason: "required" })?.try_into()?,
            data: ArrowIpc::new(value.ipc),
        })
    }
}
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
