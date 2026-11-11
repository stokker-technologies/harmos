/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, ServingError, Visibility};

/* === Constants === */

const POLICY: harmos_operations::execution::operations::OperationPolicy = harmos_operations::execution::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::execution::operations::RetryPolicy::Never,
};

/* === Structs === */

pub struct Put;

/* === Implementations === */

impl ProtobufOperation for Put {
    type Request = crate::protocol::Put;
    type Response = crate::protocol::Receipt;
    type Endpoints = ();
    const ID: &'static str = "benchmark.put";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety = harmos_operations::execution::operations::ReplaySafety::Unknown;
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::Store>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, _endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let store = ctx.resource::<super::Store>()?;
            let batch = request.batch.ok_or(crate::Error::InvalidInput)?;
            let mut stores = store.batches.lock().unwrap();
            if request.first {
                stores.remove(&request.transfer);
            }
            let retained_bytes: usize = stores.values().map(|stored| stored.bytes).sum();
            let bytes = super::store::memory(&batch);
            if retained_bytes.saturating_add(bytes) > 64 * 1024 * 1024 {
                return Err(crate::Error::Stream(harmos_operations::execution::operations::StreamError::Capacity));
            }
            let receipt = crate::protocol::Receipt {
                records: batch.records.len() as u64,
                samples: batch.samples.len() as u64,
                bytes: batch.bytes.len() as u64,
            };
            let stored = stores.entry(request.transfer).or_default();
            stored.bytes += bytes;
            stored.batches.push_back(batch);
            Ok(receipt)
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> harmos_operations::execution::operations::FailureKind {
        match error.downcast_ref::<crate::Error>() {
            Some(crate::Error::InvalidInput) => harmos_operations::execution::operations::FailureKind::Invalid,
            _ => harmos_operations::execution::operations::FailureKind::Permanent,
        }
    }
}
