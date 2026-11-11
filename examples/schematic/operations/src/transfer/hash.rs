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

#[harmos_operations::operation(operations = [harmos_operations::storage::HashFile],
    id = "benchmark.hash.file", response = FileHash,
    policy = POLICY,
    visibility = Public,
    replay = harmos_operations::operations::ReplaySafety::Idempotent,
)]
pub struct Hash {
    pub request: <harmos_operations::storage::HashFile as harmos_operations::OperationDeclaration>::Request,
}

#[harmos_operations::message]
pub struct FileHash {
    pub read_bytes: u64,
    pub sha256: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos_operations::Operation for Hash {
    type Error = crate::Error;

    async fn run(self, ctx: harmos_operations::Context<'_, Self>) -> Result<harmos_operations::Response<FileHash>, Self::Error> {
        // Hash the File
        let request = self.request;
        let response = ctx.operation(request).await?;
        let response = FileHash {
            read_bytes: response.bytes_read,
            sha256: response.sha256.to_vec(),
        };
        Ok(response.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
