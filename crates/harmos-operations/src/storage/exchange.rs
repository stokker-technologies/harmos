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
use crate::execution::operations::{ProtobufInput, ProtobufOutput};
use crate::{Error, Result};
use harmos_protocol::storage::{BatchReceipt, WriteCompletion, WriteReceipt, write_receipt::Receipt};
use harmos_storage::StorageManager;
use harmos_storage::writes::DecodedBatch;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Copy)]
pub(crate) enum WriteKind {
    Streams,
    Records,
    Objects,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tracing::instrument(target = "harmos::storage", name = "storage.exchange", level = "debug", skip_all)]
pub(crate) async fn exchange<C>(
    storage: &StorageManager<C>, ctx: &std::sync::Arc<dyn crate::execution::operations::access::GrantAuthority>,
    scope: Option<&crate::Identifier>,
    endpoints: &(ProtobufInput<harmos_protocol::storage::DataBatch>, ProtobufOutput<harmos_protocol::storage::WriteReceipt>),
    kind: WriteKind, operation: &'static str,
) -> Result<WriteCompletion> {
    storage.retry_transient_persistence();
    let (input, output) = endpoints;
    let mut pending = VecDeque::<(u64, harmos_storage::StorageCursor)>::new();
    let mut previous = None;
    let mut batches = 0u64;
    let mut cursor = storage.cursor_saved();
    let mut closed = false;

    loop {
        if closed && pending.is_empty() {
            return Ok(WriteCompletion { batches, cursor: Some(cursor.into()) });
        }
        let head = pending.front().cloned();
        tokio::select! {
            result = async {
                storage.await_durable(&head.as_ref().expect("guarded pending receipt").1).await
            }, if head.is_some() => {
                result?;
                let receipt = pending.pop_front().expect("pending receipt");
                cursor = receipt.1.clone();
                let durable = WriteReceipt { receipt: Some(Receipt::Durable(BatchReceipt { sequence: receipt.0, cursor: Some(receipt.1.into()) })) };
                output.send(durable).await?;
            }
            item = input.recv(), if !closed && pending.len() < storage.write_receipt_capacity() => {
                let Some(batch) = item else {
                    closed = true;
                    continue;
                };
                let sequence = batch.sequence;
                if previous.is_some_and(|previous| sequence <= previous) {
                    return Err(Error::InvalidInput {
                        field: "write batch sequence",
                        reason: "sequences must increase within an invocation",
                    });
                }
                let (batch, _claims) = batch.into_owned_request()?;
                let batch = DecodedBatch::of(batch.try_into()?)?;
                if batch.data.as_record_batch().num_rows() == 0 {
                    return Err(Error::InvalidInput {
                        field: "write batch",
                        reason: "empty batches are not accepted",
                    });
                }
                let grant = harmos_domain::plugin::Grant::of(operation, batch.destination.as_str().to_owned());
                ctx.admit(&grant).map_err(Error::Access)?;
                let admitted = match kind {
                    WriteKind::Streams => storage.admit_arrow_streams(scope, &batch).await?,
                    WriteKind::Records => storage.admit_arrow_records(scope, &batch).await?,
                    WriteKind::Objects => storage.admit_arrow_objects(scope, &batch).await?,
                };
                let receipt = (sequence, admitted);
                previous = Some(sequence);
                batches = batches.checked_add(1).ok_or(Error::ResourceExhausted { resource: "write batch count" })?;
                pending.push_back(receipt.clone());
                // Storage Owns the Accepted Batch Even If Delivery Fails
                let accepted = WriteReceipt { receipt: Some(Receipt::Accepted(BatchReceipt { sequence: receipt.0, cursor: Some(receipt.1.into()) })) };
                output.send(accepted).await?;
            }
        }
    }
}

/* === Tests === */
