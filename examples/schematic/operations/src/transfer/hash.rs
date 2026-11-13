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

pub struct Hash;

/* === Implementations === */

impl ProtobufOperation for Hash {
    type Request = crate::protocol::Hash;
    type Response = crate::protocol::FileHash;
    type Endpoints = ();
    const ID: &'static str = "benchmark.hash.file";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const OPERATIONS: &'static [harmos_operations::execution::operations::ProtobufDependency] =
        &[harmos_operations::execution::operations::ProtobufDependency::of::<harmos_operations::storage::context::GetFileHash>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, _endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let request = request.request.ok_or(crate::Error::InvalidInput)?;
            let response = ctx
                .call_protobuf::<harmos_operations::storage::context::GetFileHash>(request)
                .await
                .map_err(|e| crate::Error::Host(Box::new(e)))?;
            let response = response.hash.as_ref().ok_or(crate::Error::InvalidInput)?;
            Ok(crate::protocol::FileHash {
                read_bytes: response.bytes_read,
                sha256: response.sha256.clone(),
            })
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
