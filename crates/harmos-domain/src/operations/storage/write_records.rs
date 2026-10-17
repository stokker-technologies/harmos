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
use super::IO_POLICY;
use crate::Identifier;
use crate::MemorySize;
use crate::model::storage::codec::{Id, Optional, product};
use crate::model::storage::{DataBatch, WriteCompletion, WriteReceipt};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(WriteRecords {
    scope: Optional<Id> => 1,
});

crate::declare!(WriteRecords {
    id: "storage.write_records",
    version: 1,
    policy: IO_POLICY,
    response: WriteCompletion,
    input: DataBatch,
    output: WriteReceipt,
    visibility: Visibility::Shared,
    replay: ReplaySafety::Unknown,
    grpc: ("harmos.storage.v1", "Storage", "WriteRecords"),
});

/* === Type Aliases === */

/* === Structs === */
// Admit Streamed Arrow Batches Of Records Into One Optional Scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteRecords {
    pub scope: Option<Identifier>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for WriteRecords {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of_val(&self.scope)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
