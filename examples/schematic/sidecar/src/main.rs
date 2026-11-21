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
use std::error::Error;
use std::path::PathBuf;
use std::{env, fs, io, process};

// Third Party Imports
use tempfile::Builder;
use tracing::Metadata;
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<(), Box<dyn Error>> {
    let report = capture()?;
    let directory = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| ".".into());
    let outcome = schematic_sidecar::plugin(directory).run();
    let flushed = report.as_ref().map(FlushGuard::flush).transpose();
    outcome?;
    flushed?;
    Ok(())
}

fn capture() -> io::Result<Option<FlushGuard>> {
    let Some(directory) = env::var_os("STKR_TEST_REPORT_DIR") else {
        return Ok(None);
    };
    let directory = PathBuf::from(directory).join("traces");
    fs::create_dir_all(&directory)?;
    let process = process::id();
    let prefix = format!("schematic-sidecar-{process}-");
    let temporary = Builder::new().prefix(&prefix).suffix(".pftrace").tempfile_in(directory)?;
    let (file, _) = temporary.keep().map_err(|failure| failure.error)?;

    let (layer, guard) =
        PerfettoLayer::builder(file).with_debug_annotations().with_source_locations().with_poll_slices().with_counters().build();
    let layer = layer.with_filter(filter_fn(instrumented));
    tracing_subscriber::registry().with(layer).try_init().map_err(io::Error::other)?;
    Ok(Some(guard))
}

fn instrumented(metadata: &Metadata<'_>) -> bool {
    if !metadata.is_span() {
        return matches!(metadata.target(), "harmos::operations" | "harmos::state" | "harmos::carrier" | "harmos::grpc");
    }
    let prefixes = ["operations.", "state.", "storage.", "streams.", "carrier.", "grpc.", "guest.", "sidecar.", "system.story"];
    prefixes.iter().any(|prefix| metadata.name().starts_with(prefix))
}

/* === Tests === */
