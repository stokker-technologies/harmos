/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Put Transfer Contracts and Execution.

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

#[harmos_operations::operation(resources = [Store],
    id = "benchmark.put", response = Receipt,
    policy = POLICY,
    visibility = Public,
    replay = harmos_operations::operations::ReplaySafety::Unknown,
)]
pub struct Put {
    pub transfer: u64,
    pub first: bool,
    pub batch: Batch,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos_operations::Operation for Put {
    type Error = crate::Error;

    async fn run(mut self, ctx: harmos_operations::Context<'_, Self>) -> Result<harmos_operations::Response<Receipt>, Self::Error> {
        let store = ctx.resource::<Store>();

        let mut stores = store.batches.lock().unwrap();
        if self.first {
            stores.remove(&self.transfer);
        }
        let retained_bytes: usize = stores.values().map(|stored| stored.bytes).sum();
        let bytes = retained_bytes.saturating_add(self.batch.memory_size());
        if bytes > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos_operations::execution::operations::StreamError::Capacity));
        }
        let empty = Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: Vec::new(),
        };
        let batch = std::mem::replace(&mut self.batch, empty);
        let receipt = Receipt {
            records: batch.records.len() as u64,
            samples: batch.samples.len() as u64,
            bytes: batch.bytes.len() as u64,
        };
        let stored = stores.entry(self.transfer).or_default();
        stored.bytes += batch.memory_size();
        stored.batches.push_back(batch);
        Ok(receipt.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
