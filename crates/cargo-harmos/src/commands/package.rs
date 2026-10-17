/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Package Stamped Build Outputs Without Launching Cargo.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::Read as _;
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use harmos_packaging::{Build, BuildCancellation, Error, config::BuildBackend};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Args)]
pub struct Arguments {
    #[arg(long, help = "A Stamped Build Receipt; Packaging Never Runs Cargo.")]
    build_json: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, requires = "relocate_to")]
    relocate_from: Option<PathBuf>,
    #[arg(long, requires = "relocate_from")]
    relocate_to: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(
    arguments: Arguments,
    backend: BuildBackend,
    cancellation: &BuildCancellation,
) -> Result<(), Error> {
    let file = std::fs::File::open(&arguments.build_json)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::Settings(
            "build receipt exceeds its byte limit".into(),
        ));
    }
    let mut build: Build = serde_json::from_slice(&bytes)?;
    if build.backend != backend {
        return Err(Error::Settings(
            "build receipt belongs to another backend".into(),
        ));
    }
    if let (Some(from), Some(to)) = (&arguments.relocate_from, &arguments.relocate_to) {
        build.relocate(from, to)?;
    }
    let packages =
        harmos_packaging::package_with_cancellation(&build, &arguments.output, cancellation)?;
    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&packages)?);
    } else {
        for package in packages {
            println!("{}: {}", package.producer, package.path.display());
        }
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
