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

pub struct ListDirectories;

/* === Implementations === */

impl ProtobufOperation for ListDirectories {
    type Request = crate::protocol::ListDirectories;
    type Response = crate::protocol::Directories;
    type Endpoints = ();
    const ID: &'static str = "schematic.list.directories";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety = harmos_operations::execution::operations::ReplaySafety::Unknown;
    const GRPC: Option<harmos_operations::execution::operations::Grpc> =
        Some(harmos_operations::execution::operations::Grpc::new("schematic.v1", "Schematic", "ListDirectories"));
    async fn run(_ctx: ExecutionContext, request: Self::Request, _endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let _ = request;
            let mut directories = Vec::new();
            for entry in std::fs::read_dir(".")? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    directories.push(entry.file_name().into_string().map_err(crate::Error::DirectoryName)?);
                }
            }
            directories.sort();
            Ok(crate::protocol::Directories { directories })
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
