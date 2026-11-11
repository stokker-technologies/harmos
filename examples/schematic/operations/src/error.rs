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
    #[error("invalid analysis input")]
    InvalidInput,

    #[error("analysis resources are unavailable")]
    Unavailable,

    #[error(transparent)]
    Resource(#[from] harmos_operations::ResourceError),

    #[error(transparent)]
    Invocation(#[from] harmos_operations::execution::operations::InvocationError),

    #[error(transparent)]
    Output(#[from] harmos_operations::Error),

    #[error(transparent)]
    Stream(#[from] harmos_operations::execution::operations::StreamError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("directory name is not UTF-8: {0:?}")]
    DirectoryName(std::ffi::OsString),

    #[error(transparent)]
    Host(Box<dyn std::error::Error + Send + Sync>),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
