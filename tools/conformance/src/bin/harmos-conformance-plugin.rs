/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve the Hand-Written Rust Conformance Plugin.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{BufRead, Write};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tokio::main]
async fn main() -> harmos_conformance::Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let remote = arguments == ["--remote"];
    if !arguments.is_empty() && !remote {
        return Err(harmos_conformance::Error::Invalid("Usage: harmos-conformance-plugin [--remote]".into()));
    }
    // A Bundle May Supply an Explicit Startup Control Endpoint for Lifecycle Stories.
    let control = std::env::current_exe()?.parent().expect("executable directory").join("startup-control");
    if control.exists() {
        let address = std::fs::read_to_string(control)?;
        let mut connection = std::net::TcpStream::connect(address.trim())?;
        writeln!(connection, "{}", std::process::id())?;
        let mut command = String::new();
        std::io::BufReader::new(connection).read_line(&mut command)?;
        if command.trim() == "fail" {
            println!("harmos listening invalid");
            std::io::stdout().flush()?;
            std::future::pending::<()>().await;
        }
    }
    harmos_conformance::serve(remote).await
}

/* === Private Functions === */

/* === Tests === */
