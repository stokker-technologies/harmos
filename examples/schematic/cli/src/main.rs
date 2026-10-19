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
use std::path::PathBuf;
use std::process::ExitCode;

// Third Party Imports
use clap::Parser;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Parser)]
#[command(version, about = "Run The Schematic Editor Walkthrough")]
struct Args {
    #[arg(long, conflicts_with = "sidecar", num_args = 0..=1)]
    guest: Option<Option<PathBuf>>,

    #[arg(long)]
    sidecar: Option<PathBuf>,

    #[arg(long)]
    advanced: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    match dispatch(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("schematic: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn dispatch(args: Args) -> schematic_cli::Result<()> {
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or_else(|| std::io::Error::other("schematic executable has no directory"))?;
    let bundled = directory.join("plugins").join(format!("schematic-sidecar{}", std::env::consts::EXE_SUFFIX));
    let sibling = if bundled.is_file() { bundled } else { directory.join(format!("schematic-sidecar{}", std::env::consts::EXE_SUFFIX)) };
    let sidecar = args.sidecar.unwrap_or(sibling);
    let guest = args.guest.map(|path| path.unwrap_or_else(|| directory.join("plugins/schematic-guest.wasm")));
    let directory = std::env::current_dir()?;
    schematic_cli::run(guest.as_deref(), &sidecar, &directory, args.advanced).await
}

/* === Tests === */
