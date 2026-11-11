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

// Standard Library Imports

// Third Party Imports
use schematic_domain::model::transfer::Batch;

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
#[request(response = Batch)]
#[derive(Clone)]
pub struct Take {
    pub transfer: u64,
}

pub static TAKE_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<Take>("benchmark.take", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Unknown);

pub const TAKE_OPERATION: harmos_operations::operations::OperationSpec<Take> =
    harmos_operations::operations::OperationSpec::<Take>::new(&TAKE_DESCRIPTION);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn take_handler(
        request: Take, ctx: harmos_operations::execution::operations::FunctionContext<Take>,
    ) -> Result<harmos_operations::Response<Batch>, crate::Error> {
        let store = ctx.resource::<Store>().expect("declared resource is registered");

        let mut stores = store.batches.lock().unwrap();
        let Some(stored) = stores.get_mut(&request.transfer) else {
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
            stores.remove(&request.transfer);
        }
        Ok(batch.into())
    }

    pub fn take_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            TAKE_OPERATION,
            const { &[harmos_operations::Resource::of::<Store>()] },
            take_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::take_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
