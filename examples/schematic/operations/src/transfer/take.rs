/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Take Transfer Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

#[cfg(feature = "runtime")]
use super::store::Store;
#[cfg(feature = "runtime")]
use harmos::MemorySize;
use schematic_domain::model::transfer::Batch;

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
    id = "benchmark.take", response = Batch,
    policy = POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Take {
    pub transfer: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for Take {
    type Error = crate::Error;

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<Batch>, Self::Error> {
        let store = ctx.resource::<Store>();

        let mut stores = store.batches.lock().unwrap();
        let Some(stored) = stores.get_mut(&self.transfer) else {
            return Ok(Batch {
                records: Vec::new(),
                samples: Vec::new(),
                bytes: Vec::new(),
            }
            .into());
        };
        let batch = stored.batches.pop_front().unwrap_or_else(|| Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: Vec::new(),
        });
        stored.bytes = stored.bytes.saturating_sub(batch.memory_size());
        if stored.batches.is_empty() {
            stores.remove(&self.transfer);
        }
        Ok(batch.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
