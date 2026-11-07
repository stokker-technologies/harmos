/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// An Explicit Experimental Executable; The Shipping Sidecar Entry Point Is Unchanged.

/* === Modules === */

mod measurement;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing_subscriber::prelude::*;
// Local Imports
use harmos_sidecar::kit::prototype::{Settings, serve};

/* === Constants === */

static FLOW_STALLS: AtomicU64 = AtomicU64::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct FlowProfile;

#[derive(Default)]
struct Capacity {
    buffered: u64,
    window: Option<u64>,
    connection: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl tracing::field::Visit for Capacity {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}").parse().ok();
        match field.name() {
            "buffered" => self.buffered = value.unwrap_or(0),
            "window" => self.window = value,
            "conn" => self.connection = value,
            _ => {}
        }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for FlowProfile {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut capacity = Capacity::default();
        event.record(&mut capacity);
        if capacity.buffered > 0 && (capacity.window == Some(0) || capacity.connection == Some(0)) {
            let stalls = FLOW_STALLS.fetch_add(1, Ordering::Relaxed) + 1;
            if stalls == 1 || stalls.is_multiple_of(4096) {
                eprintln!(
                    "PROFILE_H2 pid={} zero_capacity_events={stalls} stream_window={:?} connection={:?}",
                    std::process::id(),
                    capacity.window,
                    capacity.connection
                );
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("HARMOS_PROFILE_TRANSPORT").is_some() {
        let filter = tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target() == "h2::proto::streams::prioritize" && metadata.fields().field("additional").is_some()
        });
        tracing_subscriber::registry().with(FlowProfile.with_filter(filter)).init();
    }
    if std::env::args().any(|argument| argument == "--measure") {
        return measurement::run().await;
    }
    if std::env::args().any(|argument| argument == "--framed-arrow") {
        return harmos_sidecar::kit::prototype::framed_arrow().await.map_err(Into::into);
    }
    // Receive Credentials On The Private Parent Pipe, Never Arguments Or Environment.
    let mut input = BufReader::new(tokio::io::stdin());
    let mut configuration = String::new();
    input.read_line(&mut configuration).await?;
    let (token, generation): (String, u64) = serde_json::from_str(&configuration)?;

    // Bind Before Advertising Readiness.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let mut stdout = tokio::io::stdout();
    stdout.write_all(format!("{address}\n").as_bytes()).await?;
    stdout.flush().await?;

    // The Fixture Consumer Owns Its Budgets.
    let settings = Settings {
        message_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
        queued_batches: NonZeroUsize::new(2).unwrap(),
        window_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        connection_window_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
        calls: NonZeroUsize::new(16).unwrap(),
    };
    serve::<schematic_sidecar::Plugin>(listener, token, generation, settings).await?;
    Ok(())
}

/* === Tests === */
