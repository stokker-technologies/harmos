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

pub struct Upload;

/* === Implementations === */

impl ProtobufOperation for Upload {
    type Request = crate::protocol::Upload;
    type Response = crate::protocol::Receipt;
    type Endpoints = harmos_operations::execution::operations::ProtobufInput<crate::protocol::Batch>;
    const ID: &'static str = "benchmark.upload";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::Store>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let store = ctx.resource::<super::Store>()?;
            let mut batches = std::collections::VecDeque::new();
            let mut retained_bytes = 0usize;
            let mut receipt = crate::protocol::Receipt::default();
            while let Some(item) = endpoints.recv().await {
                let (batch, _claims) = item.into_owned_request()?;
                receipt.records += batch.records.len() as u64;
                receipt.samples += batch.samples.len() as u64;
                receipt.bytes += batch.bytes.len() as u64;
                retained_bytes = retained_bytes.saturating_add(super::store::memory(&batch));
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
            stores.insert(request.transfer, super::store::Stored { bytes: retained_bytes, batches });
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
