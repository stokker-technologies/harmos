/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{path::Path, process::ExitCode};

// Third Party Imports

// Local Imports

/* === Constants === */
const USAGE: &str = "Usage: harmos-sidecar-export <sidecar> <Cargo.toml> <artifact-name>";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [artifact, manifest, name] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(name) = name.to_str() else {
        eprintln!("artifact-name must be UTF-8");
        return ExitCode::from(2);
    };
    match harmos_sidecar::packaging::export(Path::new(artifact), Path::new(manifest), name) {
        Ok(directory) => {
            println!("{}", directory.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(3)
        }
    }
}

/* === Tests === */
