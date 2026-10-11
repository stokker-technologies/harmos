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
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use crate::Result;
use minibench_app::{
    app,
    config::Config,
    operations::{ReadSample, Sample},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug, clap::Args)]
pub struct Args {
    pub bridge: PathBuf,
    #[arg(long, default_value = "sim://probe-a")]
    pub device: String,
    #[arg(long)]
    pub json: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(args: Args) -> Result<()> {
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()?;

    let config = Config::new(args.bridge, args.device);
    let mut runtime = app::launch(&config).await?;
    let sample = runtime
        .operations
        .call::<ReadSample>(Sample { channel: 1 })
        .await;
    if let Err(error) = sample {
        runtime.stop().await?;
        return Err(error.into());
    }
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
    runtime.stop().await?;
    readiness?;
    report("stopped", args.json)?;
    Ok(())
}

/* === Private Functions === */
fn report(event: &str, json: bool) -> io::Result<()> {
    let mut output = io::stdout().lock();
    if json {
        writeln!(output, "{}", serde_json::json!({"event": event}))?;
    } else {
        writeln!(output, "Probe {event}")?;
    }
    output.flush()
}

/* === Tests === */
