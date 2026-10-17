/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose Guest Build And Watch Commands.

/* === Modules === */
mod build;
mod dev;

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
    #[command(subcommand)]
    command: Command,
}

/* === Enums === */
#[derive(clap::Subcommand)]
enum Command {
    #[command(about = "Inspect Embedded Plugin Metadata.")]
    Inspect(super::inspect::Arguments),
    #[command(about = "Package Stamped Plugin Artifacts.")]
    Package(super::package::Arguments),
    #[command(about = "Build And Stamp Guest Artifacts.")]
    Build(build::Arguments),
    #[command(about = "Watch Sources And Dependencies And Rebuild Guest Artifacts.")]
    Dev(dev::Arguments),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(
    arguments: Arguments,
    cancellation: &harmos_packaging::BuildCancellation,
) -> Result<(), harmos_packaging::Error> {
    match arguments.command {
        Command::Inspect(arguments) => super::inspect::run(arguments),
        Command::Package(arguments) => super::package::run(
            arguments,
            harmos_packaging::config::BuildBackend::Guest,
            cancellation,
        ),
        Command::Build(arguments) => build::run(arguments, cancellation),
        Command::Dev(arguments) => dev::run(arguments, cancellation),
    }
}

/* === Private Functions === */

/* === Tests === */
