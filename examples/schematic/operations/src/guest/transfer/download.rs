/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Download Transfer Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use schematic_domain::model::transfer::{Batch, Receipt};

// Local Imports
#[cfg(feature = "runtime")]
use super::store::Store;

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

#[derive(harmos_operations::Message, Debug, Default)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Receipt, output = Batch)]
#[derive(Clone)]
pub struct Download {
    pub transfer: u64,
}

pub static DOWNLOAD_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<Download>("benchmark.download", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Unknown);

pub const DOWNLOAD_OPERATION: harmos_operations::operations::OperationSpec<Download> =
    harmos_operations::operations::OperationSpec::<Download>::new(&DOWNLOAD_DESCRIPTION);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn download_handler(
        request: Download, ctx: harmos_operations::execution::operations::FunctionContext<Download>,
    ) -> Result<harmos_operations::Response<Receipt>, crate::Error> {
        let store = ctx.resource::<Store>().expect("declared resource is registered");

        let batches = store.batches.lock().unwrap().remove(&request.transfer).unwrap_or_default();
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        for batch in batches.batches {
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            let bytes = batch.memory_size();
            ctx.streams().send_with(bytes, || Ok(batch)).await?;
        }
        Ok(receipt.into())
    }

    pub fn download_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            DOWNLOAD_OPERATION,
            const { &[harmos_operations::Resource::of::<Store>()] },
            download_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::download_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
