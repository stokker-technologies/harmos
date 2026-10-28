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

#[cfg(feature = "guest")]
mod guest;

#[cfg(any(feature = "sidecar", feature = "guest"))]
mod inspect;

mod new;

#[cfg(any(feature = "sidecar", feature = "guest"))]
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

    #[command(about = "Build Or Watch Sandboxed Guest Plugins.")]
    #[cfg(feature = "guest")]
    Guest(Box<guest::Arguments>),
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub fn run(self, cancellation: &harmos_packaging::BuildCancellation) -> Result<(), crate::error::Error> {
        #[cfg(not(any(feature = "sidecar", feature = "guest")))]
        let _ = cancellation;
        match self {
            Self::New(arguments) => new::run(arguments).map_err(Into::into),
            Self::Bindings(arguments) => bindings::run(*arguments),
            #[cfg(feature = "sidecar")]
            Self::Sidecar(arguments) => sidecar::run(*arguments, cancellation).map_err(Into::into),
            #[cfg(feature = "guest")]
            Self::Guest(arguments) => guest::run(*arguments, cancellation).map_err(Into::into),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
