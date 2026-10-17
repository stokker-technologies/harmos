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
use std::collections::VecDeque;
// Third Party Imports
// Local Imports
use crate::operations::{Context, StreamInput, StreamOutput};
use crate::storage::StorageManager;
use crate::storage::arrow::ArrowBatch;
use crate::storage::{SchemaKey, StorageCursor};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(WriteRequest {
    scope: crate::storage::codec::Optional<crate::storage::codec::Id> => 1,
});
crate::storage::codec::product!(DataBatch {
    sequence: crate::storage::codec::Unsigned => 1,
    destination: crate::storage::codec::Id => 2,
    schema: crate::storage::codec::Nested<SchemaKey> => 3,
    data: crate::storage::codec::Nested<ArrowBatch> => 4,
});
crate::storage::codec::product!(BatchReceipt {
    sequence: crate::storage::codec::Unsigned => 1,
    cursor: crate::storage::codec::Nested<StorageCursor> => 2,
});
crate::storage::codec::product!(WriteCompletion {
    batches: crate::storage::codec::Unsigned => 1,
    cursor: crate::storage::codec::Nested<StorageCursor> => 2,
});
crate::storage::codec::variants!(WriteReceipt {
    0, 1, "accepted": Self::Accepted(receipt) => {
        receipt: crate::storage::codec::Nested<BatchReceipt> => 1,
    } => Self::Accepted(receipt);
    1, 2, "durable": Self::Durable(receipt) => {
        receipt: crate::storage::codec::Nested<BatchReceipt> => 1,
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
    pub data: ArrowBatch,
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

#[derive(Clone, Copy)]
pub(crate) enum WriteKind {
    Streams,
    Records,
    Objects,
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
pub(crate) async fn exchange<C>(
    storage: &StorageManager<C>,
    ctx: &Context<'_>,
    request: &WriteRequest,
    endpoints: &mut (StreamInput<DataBatch>, StreamOutput<WriteReceipt>),
    kind: WriteKind,
    operation: &'static str,
) -> Result<WriteCompletion> {
    storage.retry_transient_persistence();
    let (input, output) = endpoints;
    let mut pending = VecDeque::<BatchReceipt>::new();
    let mut previous = None;
    let mut batches = 0u64;
    let mut cursor = storage.cursor_saved();
    let mut closed = false;

    loop {
        if closed && pending.is_empty() {
            return Ok(WriteCompletion { batches, cursor });
        }
        let head = pending.front().cloned();
        tokio::select! {
            result = async {
                storage.await_durable(&head.as_ref().expect("guarded pending receipt").cursor).await
            }, if head.is_some() => {
                result?;
                let receipt = pending.pop_front().expect("pending receipt");
                cursor = receipt.cursor.clone();
                let durable = WriteReceipt::Durable(receipt);
                output.send_with(durable.memory_size(), || Ok(durable)).await?;
            }
            item = input.recv(), if !closed && pending.len() < storage.write_receipt_capacity() => {
                let Some(batch) = item else {
                    closed = true;
                    continue;
                };
                if previous.is_some_and(|sequence| batch.sequence <= sequence) {
                    return Err(Error::InvalidInput {
                        field: "write batch sequence",
                        reason: "sequences must increase within an invocation",
                    });
                }
                if batch.data.as_record_batch().num_rows() == 0 {
                    return Err(Error::InvalidInput {
                        field: "write batch",
                        reason: "empty batches are not accepted",
                    });
                }
                let grant = harmos_domain::plugin::Grant::of(operation, batch.destination.as_str().to_owned());
                ctx.admit_stream_grant(&grant)?;
                let admitted = match kind {
                    WriteKind::Streams => storage.admit_arrow_streams(request.scope.as_ref(), &batch).await?,
                    WriteKind::Records => storage.admit_arrow_records(request.scope.as_ref(), &batch).await?,
                    WriteKind::Objects => storage.admit_arrow_objects(request.scope.as_ref(), &batch).await?,
                };
                let receipt = BatchReceipt { sequence: batch.sequence, cursor: admitted };
                previous = Some(batch.sequence);
                batches = batches.checked_add(1).ok_or(Error::ResourceExhausted { resource: "write batch count" })?;
                pending.push_back(receipt.clone());
                // Storage Owns The Accepted Batch Even If Delivery Fails
                let accepted = WriteReceipt::Accepted(receipt);
                output.send_with(accepted.memory_size(), || Ok(accepted)).await?;
            }
        }
    }
}

/* === Tests === */
