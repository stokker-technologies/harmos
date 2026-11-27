/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Command Tree.

/* === Modules === */

mod codegen;
mod proto;

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
    /// Export or check the authoritative Harmos protocols.
    #[command(subcommand)]
    Proto(proto::Command),

    /// Generate SDK bindings from descriptor sets.
    #[command(subcommand)]
    Codegen(Box<codegen::Command>),
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub(crate) fn run(self) -> Result<()> {
        match self {
            Self::Proto(command) => command.run(),
            Self::Codegen(command) => command.run(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
