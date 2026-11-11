/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash Transfer Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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

#[derive(harmos_operations::Message, Debug)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = FileHash)]
#[derive(Clone)]
pub struct Hash {
    pub request: <harmos_guest::builtins::storage::HashFile as harmos_operations::RequestShape>::Request,
}

pub static HASH_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<Hash>("benchmark.hash.file", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Idempotent);

static HASH_CALLEES: [harmos_operations::operations::WireDeclaration; 1] =
    [harmos_guest::builtins::storage::HASH_FILE_DESCRIPTION.copied()];
pub const HASH_OPERATION: harmos_operations::operations::OperationSpec<Hash> =
    harmos_operations::operations::OperationSpec::<Hash>::new(&HASH_DESCRIPTION).calls(&HASH_CALLEES);

#[derive(harmos_operations::Message, Clone, Debug, Default)]
pub struct FileHash {
    pub read_bytes: u64,
    pub sha256: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn hash_handler(
        request: Hash, ctx: harmos_operations::execution::operations::FunctionContext<Hash>,
    ) -> Result<harmos_operations::Response<FileHash>, crate::Error> {
        // Hash the File
        let request = request.request;
        let response = ctx.operation(harmos_guest::builtins::storage::HASH_FILE_OPERATION, request).await?;
        let response = FileHash {
            read_bytes: response.bytes_read,
            sha256: response.sha256.to_vec(),
        };
        Ok(response.into())
    }

    pub fn hash_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(HASH_OPERATION, const { &[] }, hash_handler, |_| {
            harmos_operations::operations::FailureKind::Permanent
        })
    }
}

#[cfg(feature = "runtime")]
pub use execution::hash_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
