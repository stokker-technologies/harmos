/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Parse Cargo Subcommand Arguments And Report One Command Outcome.

/* === Modules === */
mod commands;
mod error;
#[cfg(any(feature = "sidecar", feature = "guest"))]
mod input;
#[cfg(any(feature = "sidecar", feature = "guest"))]
mod output;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use clap::Parser;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Parser)]
#[command(
    name = "cargo harmos",
    version,
    about = "Build And Inspect Harmos Plugin Artifacts"
)]
struct Arguments {
    #[command(subcommand)]
    command: commands::Command,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() -> std::process::ExitCode {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "harmos")
    {
        arguments.remove(1);
    }
    let arguments = Arguments::parse_from(arguments);
    let cancellation = harmos::packaging::BuildCancellation::default();
    #[cfg(any(feature = "sidecar", feature = "guest"))]
    let startup = {
        let stopping = cancellation.clone();
        ctrlc::try_set_handler(move || stopping.cancel())
            .map_err(|error| harmos::packaging::Error::Io(std::io::Error::other(error.to_string())))
    };
    #[cfg(not(any(feature = "sidecar", feature = "guest")))]
    let startup: Result<(), harmos::packaging::Error> = Ok(());
    let result = startup
        .map_err(error::Error::from)
        .and_then(|()| arguments.command.run(&cancellation));
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let code = error.exit_code();
            eprintln!("{error}");
            std::process::ExitCode::from(code)
        }
    }
}

/* === Tests === */
