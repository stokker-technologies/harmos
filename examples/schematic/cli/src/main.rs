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
use tracing::Instrument;
use tracing_perfetto_file::{FlushGuard, PerfettoLayer};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
    let report = match capture() {
        Ok(report) => report,
        Err(error) => {
            eprintln!("schematic trace: {error}");
            return ExitCode::FAILURE;
        }
    };
    let story = tracing::debug_span!("system.story", application = "schematic-cli", process = std::process::id());
    let mut status = match dispatch(args).instrument(story).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("schematic: {}", harmos::failure::Report(&error));
            ExitCode::FAILURE
        }
    };
    let flushed = report.as_ref().map(FlushGuard::flush).transpose();
    if let Err(error) = flushed {
        eprintln!("schematic trace: {error}");
        status = ExitCode::FAILURE;
    }
    status
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

fn capture() -> std::io::Result<Option<FlushGuard>> {
    let Some(directory) = std::env::var_os("STKR_TEST_REPORT_DIR") else {
        return Ok(None);
    };
    let directory = PathBuf::from(directory).join("traces");
    std::fs::create_dir_all(&directory)?;
    let process = std::process::id();
    let prefix = format!("schematic-cli-{process}-");
    let temporary = tempfile::Builder::new().prefix(&prefix).suffix(".pftrace").tempfile_in(directory)?;
    let (file, _) = temporary.keep().map_err(|failure| failure.error)?;

    let (layer, guard) =
        PerfettoLayer::builder(file).with_debug_annotations().with_source_locations().with_poll_slices().with_counters().build();
    let layer = layer.with_filter(filter_fn(instrumented));
    tracing_subscriber::registry().with(layer).try_init().map_err(std::io::Error::other)?;
    Ok(Some(guard))
}

fn instrumented(metadata: &tracing::Metadata<'_>) -> bool {
    if !metadata.is_span() {
        return matches!(metadata.target(), "harmos::operations" | "harmos::state" | "harmos::carrier" | "harmos::grpc");
    }
    let prefixes = ["operations.", "state.", "storage.", "streams.", "carrier.", "grpc.", "guest.", "sidecar.", "system.story"];
    prefixes.iter().any(|prefix| metadata.name().starts_with(prefix))
}

/* === Tests === */
