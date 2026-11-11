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

pub struct Take;

/* === Implementations === */

impl ProtobufOperation for Take {
    type Request = crate::protocol::Take;
    type Response = crate::protocol::Batch;
    type Endpoints = ();
    const ID: &'static str = "benchmark.take";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety = harmos_operations::execution::operations::ReplaySafety::Unknown;
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::Store>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, _endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let store = ctx.resource::<super::Store>()?;
            let mut stores = store.batches.lock().unwrap();
            let Some(stored) = stores.get_mut(&request.transfer) else {
                return Ok(crate::protocol::Batch::default());
            };
            let batch = stored.batches.pop_front().unwrap_or_default();
            stored.bytes = stored.bytes.saturating_sub(super::store::memory(&batch));
            if stored.batches.is_empty() {
                stores.remove(&request.transfer);
            }
            Ok(batch)
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
