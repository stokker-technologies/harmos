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

#[cfg(feature = "runtime")]
use super::store::Store;
#[cfg(feature = "runtime")]
use super::store::Stored;
#[cfg(feature = "runtime")]
use harmos::MemorySize;
use schematic_domain::model::transfer::{Batch, Receipt};
#[cfg(feature = "runtime")]
use std::collections::VecDeque;

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
    id = "benchmark.upload", response = Receipt, input = Batch,
    policy = POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Upload {
    pub transfer: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for Upload {
    type Error = crate::Error;

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<Receipt>, Self::Error> {
        let store = ctx.resource::<Store>();

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
                return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
            }
            batches.push_back(batch);
        }
        let mut stores = store.batches.lock().unwrap();
        let other_bytes: usize = stores.iter().filter(|(id, _)| **id != self.transfer).map(|(_, stored)| stored.bytes).sum();
        if retained_bytes.saturating_add(other_bytes) > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
        }
        stores.insert(self.transfer, Stored { bytes: retained_bytes, batches });
        Ok(receipt.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
