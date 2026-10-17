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
use super::{ArrowIpc, SchemaKey, StorageCursor};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(WriteRequest {
    scope: crate::model::storage::codec::Optional<crate::model::storage::codec::Id> => 1,
});
crate::model::storage::codec::product!(DataBatch {
    sequence: crate::model::storage::codec::Unsigned => 1,
    destination: crate::model::storage::codec::Id => 2,
    schema: crate::model::storage::codec::Nested<SchemaKey> => 3,
    data: crate::model::storage::codec::Nested<ArrowIpc> => 4,
});
crate::model::storage::codec::product!(BatchReceipt {
    sequence: crate::model::storage::codec::Unsigned => 1,
    cursor: crate::model::storage::codec::Nested<StorageCursor> => 2,
});
crate::model::storage::codec::product!(WriteCompletion {
    batches: crate::model::storage::codec::Unsigned => 1,
    cursor: crate::model::storage::codec::Nested<StorageCursor> => 2,
});
crate::model::storage::codec::variants!(WriteReceipt {
    0, 1, "accepted": Self::Accepted(receipt) => {
        receipt: crate::model::storage::codec::Nested<BatchReceipt> => 1,
    } => Self::Accepted(receipt);
    1, 2, "durable": Self::Durable(receipt) => {
        receipt: crate::model::storage::codec::Nested<BatchReceipt> => 1,
    } => Self::Durable(receipt);
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteRequest {
    pub scope: Option<Identifier>,
}

#[derive(Clone, Debug)]
pub struct DataBatch {
    pub sequence: u64,
    pub destination: Identifier,
    pub schema: SchemaKey,
    pub data: ArrowIpc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchReceipt {
    pub sequence: u64,
    pub cursor: StorageCursor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteCompletion {
    pub batches: u64,
    pub cursor: StorageCursor,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteReceipt {
    Accepted(BatchReceipt),
    Durable(BatchReceipt),
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for WriteRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.scope
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.scope)),
        )
    }
}

impl MemorySize for DataBatch {
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

impl MemorySize for BatchReceipt {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.cursor
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.cursor)),
        )
    }
}

impl MemorySize for WriteReceipt {
    fn memory_size(&self) -> usize {
        let receipt = match self {
            Self::Accepted(receipt) | Self::Durable(receipt) => receipt,
        };
        std::mem::size_of::<Self>().saturating_add(
            receipt
                .memory_size()
                .saturating_sub(std::mem::size_of::<BatchReceipt>()),
        )
    }
}

impl MemorySize for WriteCompletion {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.cursor
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.cursor)),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
