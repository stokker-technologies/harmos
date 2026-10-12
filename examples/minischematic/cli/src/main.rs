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
pub use minischematic_cli::error::{Error, Result};

/* === Imports === */
// Standard Library Imports
use std::path::{Path, PathBuf};
use std::process::ExitCode;
// Third Party Imports
use clap::Parser;

// Local Imports
use minischematic_app::config::Config;
use minischematic_app::document::Document;
use minischematic_app::error::{ConfigError, DocumentError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Parser)]
#[command(
    version,
    about = "Run The Schematic Editor Example",
    args_conflicts_with_subcommands = true
)]
struct Args {
    #[arg(long)]
    guest: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

/* === Enums === */
#[derive(clap::Subcommand)]
enum Command {
    Serve(minischematic_cli::serve::Args),
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
            eprintln!("minischematic: {error}");
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
        return minischematic_cli::serve::run(args).await;
    }

    let example = scratch();
    let config = configure(&example)?;
    if let Some(guest) = args.guest {
        print!(
            "{}",
            minischematic_cli::run_autoroute(&guest, &config).await?
        );
    } else {
        let document = open(&example.join("schematic.json"))?;
        minischematic_cli::run(document, &config).await?;
    }
    Ok(())
}

fn configure(example: &Path) -> std::result::Result<Config, ConfigError> {
    match Config::from_env() {
        Err(ConfigError::Missing { .. }) => Ok(Config::new(example.join("streams"))),
        resolved => resolved,
    }
}

fn open(path: &Path) -> std::result::Result<Document, DocumentError> {
    if path.exists() {
        Document::open(path)
    } else {
        Ok(Document::new(path))
    }
}

fn scratch() -> PathBuf {
    let directory = std::env::temp_dir().join("harmos-example-schematic");

    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the temporary directory is writable");

    directory
}

/* === Tests === */
