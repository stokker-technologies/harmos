/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Harmos Product Tooling: Protocol Export, Protocol Checks and SDK Binding Generation.

/* === Modules === */

mod commands;
mod descriptor;
mod error;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::process::ExitCode;

// Third Party Imports
use clap::Parser;

// Local Imports
use crate::commands::Command;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Parser)]
#[command(name = "harmos", version, about = "Export Harmos Protocols, Check Their Contracts and Generate SDK Bindings")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("harmos: {error}");
            error.exit_code()
        }
    }
}

/* === Tests === */
