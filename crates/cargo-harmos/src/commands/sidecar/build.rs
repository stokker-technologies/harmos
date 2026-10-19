/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Build Or Collect Sidecar Artifacts Through Harmos Packaging.

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

#[derive(clap::Args)]
pub struct Arguments {
    #[command(flatten)]
    build: crate::input::BuildArguments,

    #[command(flatten)]
    collection: crate::input::CollectionArguments,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(arguments: Arguments, cancellation: &harmos_packaging::BuildCancellation) -> Result<(), harmos_packaging::Error> {
    let json = arguments.build.json;
    let settings = arguments.build.settings(harmos_packaging::config::BuildBackend::Sidecar);
    let result = arguments.collection.build(&settings, cancellation)?;
    crate::output::build(&result, json)
}

/* === Private Functions === */

/* === Tests === */
