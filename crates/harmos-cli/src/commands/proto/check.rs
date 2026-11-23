/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Validate Protocol Custom-Option Contracts.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use crate::descriptor::Schema;
use crate::error::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Descriptor set to validate, compiled with its imports.
    #[arg(long)]
    descriptors: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    let schema = Schema::load(&[args.descriptors])?;
    println!("Protocol Callers: {} Annotated Operations", schema.callers().len());
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
