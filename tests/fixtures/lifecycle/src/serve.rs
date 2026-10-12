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
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Instant;

// Third Party Imports

// Local Imports
use super::workload::Workload;
use crate::Result;
use schematic_app::{app, config::Config, document::Document};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug, clap::Args)]
pub struct Args {
    pub directory: PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub exercise_shutdown: bool,
    #[arg(long, requires = "exercise_shutdown")]
    pub worker: Option<PathBuf>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(args: Args) -> Result<()> {
    // Register Signals Before Readiness
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()?;

    std::fs::create_dir_all(&args.directory)?;
    let path = args.directory.join("schematic.json");
    let document = if path.exists() {
        Document::open(&path)?
    } else {
        Document::new(&path)
    };
    let mut config = Config::new(args.directory.join("streams"));
    if args.exercise_shutdown {
        config.journal_sync_entries = NonZeroUsize::new(128).unwrap();
    }
    let runtime = app::launch(document.clone(), &config).await?;

    let worker = if let Some(path) = &args.worker {
        match crate::worker::start(&runtime, path).await {
            Ok(worker) => Some(worker),
            Err(error) => {
                runtime
                    .stop_with_timeout(std::time::Duration::from_secs(4))
                    .await?;
                return Err(error.into());
            }
        }
    } else {
        None
    };

    let workload = if args.exercise_shutdown {
        match Workload::start(&runtime).await {
            Ok(workload) => Some(workload),
            Err(error) => {
                runtime
                    .stop_with_timeout(std::time::Duration::from_secs(4))
                    .await?;
                return Err(error);
            }
        }
    } else {
        None
    };

    let readiness = report("ready", args.json);
    if readiness.is_ok() {
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
        #[cfg(windows)]
        interrupt.recv().await;
    }

    // Cleanup Also Runs When Readiness Output Fails
    let signal = Instant::now();
    if let Some(workload) = &workload {
        workload.signal(signal, &runtime);
    }
    runtime
        .stop_with_timeout(std::time::Duration::from_secs(4))
        .await?;
    let settlement = signal.elapsed();
    let mut expected = match workload {
        Some(workload) => Some(workload.verify(settlement).await?),
        None => None,
    };
    if let Some(worker) = worker {
        let exchanges = worker.verify().await?;
        if let Some(expected) = &mut expected {
            expected.timings["worker_exchanges"] = exchanges.into();
        }
    }
    readiness?;
    report("stopped", args.json)?;

    if let Some(expected) = expected {
        {
            let mut output = io::stdout().lock();
            if args.json {
                writeln!(output, "{}", expected.timings)?;
            } else {
                writeln!(
                    output,
                    "Shutdown timings (nanoseconds): {}",
                    expected.timings
                )?;
            }
            output.flush()?;
        }
        // Reconstruct Through The Same JSONL Recovery Path
        let reopened = app::launch(document, &config).await?;
        let recovered = reopened.state.state().read(Clone::clone).await;
        reopened.stop().await?;
        let recovered = recovered?;
        if recovered.position != expected.position || recovered.value != expected.state {
            return Err(harmos::Error::<std::convert::Infallible>::NotReady {
                domain: "shutdown exercise persistence",
            }
            .into());
        }
        report("recovered", args.json)?;
    }
    Ok(())
}

/* === Private Functions === */
fn report(event: &str, json: bool) -> io::Result<()> {
    let mut output = io::stdout().lock();
    if json {
        writeln!(output, "{}", serde_json::json!({"event": event}))?;
    } else {
        writeln!(output, "Schematic {event}")?;
    }
    output.flush()
}

/* === Tests === */
