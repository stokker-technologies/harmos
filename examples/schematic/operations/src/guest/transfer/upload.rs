/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Upload Transfer Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
#[cfg(feature = "runtime")]
use std::collections::VecDeque;

// Third Party Imports
use schematic_domain::model::transfer::{Batch, Receipt};

// Local Imports
#[cfg(feature = "runtime")]
use super::store::Store;

#[cfg(feature = "runtime")]
use super::store::Stored;

#[cfg(feature = "runtime")]
use harmos_operations::MemorySize;

/* === Constants === */

const POLICY: harmos_operations::operations::OperationPolicy = harmos_operations::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_operations::message(harmos_path = harmos_operations)]
#[derive(harmos_operations::Request)]
#[harmos_path(harmos_operations)]
#[request(response = Receipt, input = Batch)]
#[derive(Clone)]
pub struct Upload {
    pub transfer: u64,
}

pub static UPLOAD_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<Upload>("benchmark.upload", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Idempotent);

pub const UPLOAD_OPERATION: harmos_operations::operations::OperationSpec<Upload> =
    harmos_operations::operations::OperationSpec::<Upload>::new(&UPLOAD_DESCRIPTION);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn upload_handler(
        request: Upload, ctx: harmos_operations::execution::operations::FunctionContext<Upload>,
    ) -> Result<harmos_operations::Response<Receipt>, crate::Error> {
        let store = ctx.resource::<Store>().expect("declared resource is registered");

        let mut batches = VecDeque::new();
        let mut retained_bytes = 0usize;
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        while let Some(item) = ctx.streams().recv().await {
            let (batch, _claims) = item.into_owned_request()?;
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            retained_bytes = retained_bytes.saturating_add(batch.memory_size());
            if retained_bytes > 64 * 1024 * 1024 {
                return Err(crate::Error::Stream(harmos_operations::execution::operations::StreamError::Capacity));
            }
            batches.push_back(batch);
        }
        let mut stores = store.batches.lock().unwrap();
        let other_bytes: usize = stores.iter().filter(|(id, _)| **id != request.transfer).map(|(_, stored)| stored.bytes).sum();
        if retained_bytes.saturating_add(other_bytes) > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos_operations::execution::operations::StreamError::Capacity));
        }
        stores.insert(request.transfer, Stored { bytes: retained_bytes, batches });
        Ok(receipt.into())
    }

    pub fn upload_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            UPLOAD_OPERATION,
            const { &[harmos_operations::Resource::of::<Store>()] },
            upload_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::upload_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
