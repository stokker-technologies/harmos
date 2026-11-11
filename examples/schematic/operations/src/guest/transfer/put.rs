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

#[harmos_operations::message(harmos_path = harmos_operations)]
#[derive(harmos_operations::Request)]
#[harmos_path(harmos_operations)]
#[request(response = Receipt)]
#[derive(Clone)]
pub struct Put {
    pub transfer: u64,
    pub first: bool,
    pub batch: Batch,
}

pub static PUT_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<Put>("benchmark.put", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Unknown);

pub const PUT_OPERATION: harmos_operations::operations::OperationSpec<Put> =
    harmos_operations::operations::OperationSpec::<Put>::new(&PUT_DESCRIPTION);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn put_handler(
        mut request: Put, ctx: harmos_operations::execution::operations::FunctionContext<Put>,
    ) -> Result<harmos_operations::Response<Receipt>, crate::Error> {
        let store = ctx.resource::<Store>().expect("declared resource is registered");

        let mut stores = store.batches.lock().unwrap();
        if request.first {
            stores.remove(&request.transfer);
        }
        let retained_bytes: usize = stores.values().map(|stored| stored.bytes).sum();
        let bytes = retained_bytes.saturating_add(request.batch.memory_size());
        if bytes > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos_operations::execution::operations::StreamError::Capacity));
        }
        let empty = Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: Vec::new(),
        };
        let batch = std::mem::replace(&mut request.batch, empty);
        let receipt = Receipt {
            records: batch.records.len() as u64,
            samples: batch.samples.len() as u64,
            bytes: batch.bytes.len() as u64,
        };
        let stored = stores.entry(request.transfer).or_default();
        stored.bytes += batch.memory_size();
        stored.batches.push_back(batch);
        Ok(receipt.into())
    }

    pub fn put_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            PUT_OPERATION,
            const { &[harmos_operations::Resource::of::<Store>()] },
            put_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::put_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
