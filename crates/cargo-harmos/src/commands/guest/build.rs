/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Build Or Collect Guest Artifacts Through Harmos Packaging.

#[derive(clap::Args)]
pub struct Arguments {
    #[command(flatten)]
    build: crate::input::BuildArguments,

    #[command(flatten)]
    collection: crate::input::CollectionArguments,
}

pub fn run(arguments: Arguments, cancellation: &harmos_packaging::BuildCancellation) -> Result<(), harmos_packaging::Error> {
    let json = arguments.build.json;
    let settings = arguments.build.settings(harmos_packaging::config::BuildBackend::Guest);
    let result = arguments.collection.build(&settings, cancellation)?;
    crate::output::build(&result, json)
}
