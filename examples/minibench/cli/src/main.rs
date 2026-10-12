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
pub use minibench_cli::error::{Error, Result};

/* === Imports === */
// Standard Library Imports
use std::path::PathBuf;
use std::process::ExitCode;
// Third Party Imports
use clap::Parser;

// Local Imports
use minibench_app::config::Config;
use minibench_app::error::ConfigError;

/* === Constants === */
const DEVICE: &str = "sim://probe-a";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Parser)]
#[command(
    version,
    about = "Run The Native Probe Example",
    args_conflicts_with_subcommands = true
)]
struct Args {
    bridge: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

/* === Enums === */
#[derive(clap::Subcommand)]
enum Command {
    Serve(minibench_cli::serve::Args),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[tokio::main]
#[tracing::instrument(level = "debug", skip_all)]
async fn main() -> ExitCode {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    match dispatch(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("minibench: {error}");
            if matches!(error, Error::Config(_)) {
                ExitCode::from(2)
            } else {
                ExitCode::from(3)
            }
        }
    }
}

async fn dispatch(args: Args) -> Result<()> {
    if let Some(Command::Serve(args)) = args.command {
        return minibench_cli::serve::run(args).await;
    }
    let config = configure(args.bridge)?;
    print!("{}", minibench_cli::run(&config).await?);
    Ok(())
}

fn configure(bridge: Option<PathBuf>) -> std::result::Result<Config, ConfigError> {
    match bridge {
        Some(bridge) => Ok(Config::new(bridge, DEVICE)),
        None => Config::from_env(),
    }
}

/* === Tests === */
