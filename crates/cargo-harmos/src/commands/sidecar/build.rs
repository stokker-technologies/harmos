/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prepare And Package Sidecar Artifacts Through The Harmos Facade.

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
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(
    arguments: Arguments,
    cancellation: &harmos_packaging::BuildCancellation,
    package: bool,
) -> Result<(), harmos_packaging::Error> {
    let json = arguments.build.json;
    let settings = arguments
        .build
        .settings(harmos_packaging::config::BuildBackend::Sidecar);
    let result = if package {
        harmos_packaging::package_with_cancellation(&settings, cancellation)?
    } else {
        harmos_packaging::build_with_cancellation(&settings, cancellation)?
    };
    crate::output::build(&result, json)
}

/* === Private Functions === */

/* === Tests === */
