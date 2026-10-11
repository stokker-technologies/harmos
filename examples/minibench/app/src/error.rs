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
pub type LaunchError = Error;

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("{variable} is not set")]
    Missing { variable: &'static str },
    #[error("{variable} is set to an empty value")]
    Empty { variable: &'static str },
    #[error("{variable} is not valid unicode")]
    NotUnicode { variable: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Executable(#[from] harmos::sidecar::ExecutableError),
    #[error(transparent)]
    Declaration(#[from] harmos::sidecar::inventory::DeclarationError),
    #[error(transparent)]
    Settings(#[from] harmos::sidecar::ConfigError),
    #[error(transparent)]
    Incompatible(#[from] harmos::Incompatible),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("probe activation failed ({activation}); cleanup remains pending: {source}")]
    Cleanup {
        #[source]
        source: Box<harmos::Error>,
        activation: harmos::Error,
        running: Box<crate::app::Running>,
    },
    #[error(transparent)]
    Runtime(#[from] harmos::Error),
    #[error(transparent)]
    Sidecar(#[from] harmos::sidecar::SidecarError),
    #[error("the probe sidecar readiness channel closed: {0}")]
    Readiness(#[from] tokio::sync::watch::error::RecvError),

    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error(transparent)]
    Incompatible(#[from] harmos::Incompatible),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
