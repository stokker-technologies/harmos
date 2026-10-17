/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Watch And Rebuild Guest Artifacts Through Shared Harmos APIs.

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
    #[arg(long, default_value_t = 100)]
    debounce_ms: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(
    arguments: Arguments,
    cancellation: &harmos_packaging::BuildCancellation,
) -> Result<(), harmos_packaging::Error> {
    let json = arguments.build.json;
    let settings = arguments
        .build
        .settings(harmos_packaging::config::BuildBackend::Guest);
    let policy = harmos_packaging::config::DevSettings {
        debounce: std::time::Duration::from_millis(arguments.debounce_ms),
    };
    harmos_packaging::dev(&settings, policy, cancellation, |event| {
        crate::output::dev(event, json)
    })
}

/* === Private Functions === */

/* === Tests === */
