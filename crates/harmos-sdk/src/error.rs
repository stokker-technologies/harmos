/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Lifecycle Errors

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::operations::ServingError;
use crate::plugin::Plugin;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(thiserror::Error)]
pub enum Error<P: Plugin> {
    #[error(transparent)]
    Dependency(Arc<crate::DependencyError>),

    #[error("invalid plugin configuration: {0}")]
    Configuration(#[source] Arc<crate::ConfigError>),

    #[error("plugin lifecycle machinery failed: {0}")]
    Plugin(#[source] Arc<P::Error>),

    #[error("plugin operation inventory failed: {0}")]
    Inventory(#[source] Arc<crate::inventory::operations::Error>),

    #[error("plugin operation table failed: {0}")]
    Serving(#[source] Arc<ServingError>),

    #[error("SDK generation context must be empty or exactly 32 nonzero bytes")]
    GenerationContext,

    #[error("plugin lifecycle state is unavailable for this phase")]
    Phase,

    #[error("plugin lifecycle machinery panicked")]
    Panicked,

    #[error("plugin lifecycle state lock is poisoned")]
    Poisoned,
}

/* === Traits === */

/* === Implementations === */

impl<P: Plugin> std::fmt::Debug for Error<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency(error) => formatter.debug_tuple("Dependency").field(error).finish(),
            Self::Configuration(error) => formatter.debug_tuple("Configuration").field(error).finish(),
            Self::Plugin(error) => formatter.debug_tuple("Plugin").field(error).finish(),
            Self::Inventory(error) => formatter.debug_tuple("Inventory").field(error).finish(),
            Self::Serving(error) => formatter.debug_tuple("Serving").field(error).finish(),
            Self::GenerationContext => formatter.write_str("GenerationContext"),
            Self::Phase => formatter.write_str("Phase"),
            Self::Panicked => formatter.write_str("Panicked"),
            Self::Poisoned => formatter.write_str("Poisoned"),
        }
    }
}

impl<P: Plugin> Clone for Error<P> {
    fn clone(&self) -> Self {
        match self {
            Self::Dependency(error) => Self::Dependency(Arc::clone(error)),
            Self::Configuration(error) => Self::Configuration(Arc::clone(error)),
            Self::Plugin(error) => Self::Plugin(Arc::clone(error)),
            Self::Inventory(error) => Self::Inventory(Arc::clone(error)),
            Self::Serving(error) => Self::Serving(Arc::clone(error)),
            Self::GenerationContext => Self::GenerationContext,
            Self::Phase => Self::Phase,
            Self::Panicked => Self::Panicked,
            Self::Poisoned => Self::Poisoned,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
