/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::NonZeroUsize;
use std::path::PathBuf;

use tracing::Instrument;
use tracing_perfetto_file::{FlushGuard, PerfettoLayer};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = capture()?;
    let story = tracing::debug_span!("system.story", application = "schematic-sidecar", process = std::process::id());
    let outcome = harmos::sidecar::run_plugin::<schematic_sidecar::Plugin>(NonZeroUsize::new(4).unwrap()).instrument(story).await;
    let flushed = report.as_ref().map(FlushGuard::flush).transpose();
    outcome?;
    flushed?;
    Ok(())
}

fn capture() -> std::io::Result<Option<FlushGuard>> {
    let Some(directory) = std::env::var_os("STKR_TEST_REPORT_DIR") else {
        return Ok(None);
    };
    let directory = PathBuf::from(directory).join("traces");
    std::fs::create_dir_all(&directory)?;
    let process = std::process::id();
    let prefix = format!("schematic-sidecar-{process}-");
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
