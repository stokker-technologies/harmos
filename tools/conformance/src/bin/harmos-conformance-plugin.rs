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
use std::{
    env, fs, future,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::Path,
    process,
};

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
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let remote = arguments.first().is_some_and(|argument| argument == "--remote") && arguments.len() <= 2;
    let address = remote.then(|| arguments.get(1).cloned()).flatten();
    let interpreter = arguments.first().is_some_and(|argument| argument == "--bundle-script");
    if interpreter {
        if arguments.len() != 3 || arguments[2] != "$literal argument with spaces" {
            return Err(harmos_conformance::Error::Invalid("Pass the Script and Literal Argument to the Bundle Interpreter".into()));
        }
        let script = fs::read(&arguments[1])?;
        if script != b"serve conformance" || !Path::new("manifest.json").is_file() {
            return Err(harmos_conformance::Error::Invalid("Start the Interpreter in the Unpacked Bundle Root".into()));
        }
    }
    if !arguments.is_empty() && !remote && !interpreter {
        return Err(harmos_conformance::Error::Invalid("Usage: harmos-conformance-plugin [--remote [address]]".into()));
    }
    // A Bundle May Supply an Explicit Startup Control Endpoint for Lifecycle Stories.
    let control = env::current_exe()?.parent().expect("executable directory").join("startup-control");
    if control.exists() {
        let address = fs::read_to_string(control)?;
        let mut connection = TcpStream::connect(address.trim())?;
        writeln!(connection, "{}", process::id())?;
        let mut command = String::new();
        BufReader::new(connection).read_line(&mut command)?;
        if command.trim() == "fail" {
            // The Host Refuses the Address and Fails the Launch; the Process Waits to Be Reaped.
            let _ = harmos_conformance::announce_address("invalid".into()).await;
            future::pending::<()>().await;
        }
    }
    harmos_conformance::serve(remote, address).await
}

/* === Private Functions === */

/* === Tests === */
