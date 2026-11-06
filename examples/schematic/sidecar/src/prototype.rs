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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
// Third Party Imports
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
// Local Imports
use harmos_sidecar::kit::prototype::{Settings, serve};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        calls: NonZeroUsize::new(16).unwrap(),
    };
    serve::<schematic_sidecar::Plugin>(listener, token, generation, settings).await?;
    Ok(())
}

/* === Tests === */
