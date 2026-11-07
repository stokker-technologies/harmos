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

#[cfg(feature = "runtime")]
use super::store::Store;
#[cfg(feature = "runtime")]
use harmos::MemorySize;
use schematic_domain::model::transfer::{Batch, Receipt};

/* === Constants === */

const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(resources = [Store],
    id = "benchmark.download", response = Receipt, output = Batch,
    policy = POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Download {
    pub transfer: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for Download {
    type Error = crate::Error;

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<Receipt>, Self::Error> {
        let store = ctx.resource::<Store>();

        let batches = store.batches.lock().unwrap().remove(&self.transfer).unwrap_or_default();
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
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
