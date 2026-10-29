/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod python;

#[derive(clap::Args)]
pub struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    #[command(about = "Generate A Pure Python Package From One Selected Contract Export.")]
    Python(python::Arguments),
}

pub fn run(arguments: Arguments) -> Result<(), crate::error::Error> {
    match arguments.command {
        Command::Python(arguments) => python::run(arguments),
    }
}
