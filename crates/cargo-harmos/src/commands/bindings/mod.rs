/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose Producer Binding Commands.

/* === Modules === */
mod python;

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
    #[command(help = "Generate A Pure Python Package From One Selected Contract Export.")]
    Python(python::Arguments),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(arguments: Arguments) -> Result<(), crate::error::Error> {
    match arguments.command {
        Command::Python(arguments) => python::run(arguments),
    }
}

/* === Private Functions === */

/* === Tests === */
