/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Application File Hashing and the Benchmark Child-Call Bridge

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use harmos_operations::execution::Failure;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::files::FilesHash;
use harmos_operations::execution::operations::{
    ExecutionContext, FailureKind, OperationPolicy, ProtobufDependency, ProtobufOperation, ReplaySafety, RetryPolicy, ServingError,
    Visibility,
};
use schematic_protocol::{FileHash, Hash as HashRequest};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    cancellation_timeout: Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Hash;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Hash {
    type Request = HashRequest;
    type Response = FileHash;
    type Endpoints = ();
    const ID: &'static str = "benchmark.hash.file";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const OPERATIONS: &'static [ProtobufDependency] = &[ProtobufDependency::of::<FilesHash>()];

    async fn run(context: ExecutionContext, request: HashRequest, _: &()) -> Result<FileHash, ServingError> {
        let request = request.request.ok_or_else(|| invalid("Hash Through Plugin: Missing Request"))?;
        let response = context.call_protobuf::<FilesHash>(request).await?;
        Ok(FileHash {
            read_bytes: response.read_bytes,
            sha256: response.sha256.clone(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(message: &str) -> ServingError {
    ServingError::Handler(Failure::new(FailureKind::Invalid, message))
}

/* === Tests === */
