/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Domain(#[from] harmos_domain::Error),

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
    Dependency(#[from] harmos_sdk::ResourceError),

    #[error("operation instance `{0}` is not registered for this caller")]
    UnknownInstance(u64),

    #[error(transparent)]
    Stream(#[from] harmos_sdk::operations::StreamError),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
