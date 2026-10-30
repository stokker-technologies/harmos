/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod bindings;

#[cfg(feature = "sidecar")]
mod inspect;

mod new;

#[cfg(feature = "sidecar")]
mod package;

#[cfg(feature = "sidecar")]
mod sidecar;

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

#[derive(clap::Subcommand)]
pub enum Command {
    #[command(about = "Create A Plugin From An Embedded Current Starter.")]
    New(new::Arguments),

    #[command(about = "Generate Producer-Owned Transport Bindings.")]
    Bindings(Box<bindings::Arguments>),

    #[command(about = "Build Or Watch Native Sidecar Plugins.")]
    #[cfg(feature = "sidecar")]
    Sidecar(Box<sidecar::Arguments>),
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub fn run(self, cancellation: &harmos_packaging::BuildCancellation) -> Result<(), crate::error::Error> {
        #[cfg(not(feature = "sidecar"))]
        let _ = cancellation;
        match self {
            Self::New(arguments) => new::run(arguments).map_err(Into::into),
            Self::Bindings(arguments) => bindings::run(*arguments),
            #[cfg(feature = "sidecar")]
            Self::Sidecar(arguments) => sidecar::run(*arguments, cancellation).map_err(Into::into),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
