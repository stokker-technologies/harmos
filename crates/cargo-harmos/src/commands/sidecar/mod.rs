/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod build;
mod dev;

#[derive(clap::Args)]
pub struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    #[command(about = "Inspect Embedded Plugin Metadata.")]
    Inspect(super::inspect::Arguments),

    #[command(about = "Package Stamped Plugin Artifacts.")]
    Package(super::package::Arguments),

    #[command(about = "Build And Stamp Sidecar Artifacts.")]
    Build(build::Arguments),

    #[command(about = "Watch Sources And Dependencies And Rebuild Sidecar Artifacts.")]
    Dev(dev::Arguments),
}

pub fn run(arguments: Arguments, cancellation: &harmos_packaging::BuildCancellation) -> Result<(), harmos_packaging::Error> {
    match arguments.command {
        Command::Inspect(arguments) => super::inspect::run(arguments),
        Command::Package(arguments) => super::package::run(arguments, harmos_packaging::config::BuildBackend::Sidecar, cancellation),
        Command::Build(arguments) => build::run(arguments, cancellation),
        Command::Dev(arguments) => dev::run(arguments, cancellation),
    }
}
