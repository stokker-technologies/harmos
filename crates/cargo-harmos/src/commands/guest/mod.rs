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
    #[command(help = "Build And Package Inspected Guest Artifacts.")]
    Build(build::Arguments),
    #[command(help = "Watch Sources And Dependencies And Rebuild Guest Artifacts.")]
    Dev(dev::Arguments),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(
    arguments: Arguments,
    cancellation: &harmos::packaging::BuildCancellation,
) -> Result<(), harmos::packaging::Error> {
    match arguments.command {
        Command::Build(arguments) => build::run(arguments, cancellation),
        Command::Dev(arguments) => dev::run(arguments, cancellation),
    }
}

/* === Private Functions === */

/* === Tests === */
