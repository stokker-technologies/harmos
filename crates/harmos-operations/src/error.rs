/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("host state access failed: {0}")]
    Access(#[source] harmos_sdk::operations::access::AccessError),
    #[error(transparent)]
    Storage(#[from] harmos_storage::Error),
    #[error(transparent)]
    Message(#[from] harmos_domain::MessageError),
    #[error("{domain} is not ready")]
    NotReady { domain: &'static str },
    #[error("{resource} capacity exhausted")]
    ResourceExhausted { resource: &'static str },
    #[error("invalid {field}: {reason}")]
    InvalidInput { field: &'static str, reason: &'static str },
    #[error(transparent)]
    Dependency(#[from] harmos_sdk::DependencyError),
    #[error("operation instance `{0}` is not registered for this caller")]
    UnknownInstance(u64),
    #[error(transparent)]
    Stream(#[from] harmos_sdk::operations::StreamError),
}
