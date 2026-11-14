/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Bounded Immutable Append Handlers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::requests::{DataBatch, MutationId};

use super::requests::required;

use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::model;
use harmos_storage::StorageManager;

/* === Enums === */

pub(super) enum Kind {
    Records,
    Samples,
    Objects,
}

/* === Public Functions === */

pub(super) async fn append(
    context: &ExecutionContext, id: &'static str, kind: Kind, mutation: Option<MutationId>, scope: Option<String>, batch: Option<DataBatch>,
) -> Result<model::Receipt, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

    let result: crate::Result<_> = async {
        if !storage.capabilities().mutation_receipts {
            return Err(harmos_storage::Error::backend(
                id,
                harmos_storage::BackendFailure::Unsupported,
                std::io::Error::other("provider must retain mutation receipts"),
            )
            .into());
        }
        let batch = required(batch)?;
        if batch.memory_size() > storage.settings().max_batch_bytes().get() {
            return Err(harmos_storage::Error::ResourceExhausted { resource: "Append batch" }.into());
        }
        let mutation = required(mutation)?;
        let identity = mutation.clone();
        let scope = scope.map(harmos_domain::Identifier::new).transpose().map_err(harmos_domain::Error::from)?;
        let batch = harmos_storage::writes::DecodedBatch::of(batch)?;
        if batch.data.as_record_batch().num_rows() == 0 {
            return Err(crate::Error::InvalidInput {
                field: "Append",
                reason: "empty batches are not accepted",
            });
        }
        storage.retry_transient_persistence();
        let position = match kind {
            Kind::Records => storage.append_record_batch(&identity, scope.as_ref(), &batch).await?,
            Kind::Samples => storage.append_sample_batch(&identity, scope.as_ref(), &batch).await?,
            Kind::Objects => {
                if scope.is_some() {
                    return Err(crate::Error::InvalidInput {
                        field: "AppendObjectRevisions",
                        reason: "object revisions do not have record scopes",
                    });
                }
                storage.append_object_batch(&identity, &batch).await?
            }
        };
        let durable = storage.mutation_durable(&identity, &position)?;
        let mut receipt = super::transfers::receipt(&mutation, Some(position));
        receipt.completed = durable;
        if !durable {
            receipt.durable_at = None;
        }
        Ok(receipt)
    }
    .await;
    result.map_err(super::admission::failure)
}
