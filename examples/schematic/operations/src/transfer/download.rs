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

pub struct Download;

/* === Implementations === */

impl ProtobufOperation for Download {
    type Request = crate::protocol::Download;
    type Response = crate::protocol::Receipt;
    type Endpoints = harmos_operations::execution::operations::ProtobufOutput<crate::protocol::Batch>;
    const ID: &'static str = "benchmark.download";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety = harmos_operations::execution::operations::ReplaySafety::Unknown;
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::Store>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let store = ctx.resource::<super::Store>()?;
            let batches = store.batches.lock().unwrap().remove(&request.transfer).unwrap_or_default();
            let mut receipt = crate::protocol::Receipt::default();
            for batch in batches.batches {
                receipt.records += batch.records.len() as u64;
                receipt.samples += batch.samples.len() as u64;
                receipt.bytes += batch.bytes.len() as u64;
                endpoints.send(batch).await?;
            }
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
