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

const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(operations = [harmos::builtins::storage::HashFile],
    id = "benchmark.hash.file", response = FileHash,
    policy = POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Hash {
    pub request: <harmos::builtins::storage::HashFile as harmos::OperationDeclaration>::Request,
}

#[harmos::message]
pub struct FileHash {
    pub read_bytes: u64,
    pub sha256: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for Hash {
    type Error = crate::Error;

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<FileHash>, Self::Error> {
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
