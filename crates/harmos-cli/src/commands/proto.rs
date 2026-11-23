/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protocol Commands.

/* === Modules === */

mod check;
mod export;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use clap::Subcommand;

// Local Imports
use crate::error::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Write the authoritative Harmos `.proto` sources this revision bundles.
    Export(export::Args),

    /// Validate the Harmos custom-option contracts of a descriptor set.
    Check(check::Args),
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Self::Export(args) => export::run(args),
            Self::Check(args) => check::run(args),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
