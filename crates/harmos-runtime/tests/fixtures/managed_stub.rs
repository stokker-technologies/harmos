/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Process Whose Lifetime the Story Controls

/* === Imports === */

// Standard Library Imports
use std::{env, io::Write, process::ExitCode};

// Third Party Imports
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

// Local Imports
use harmos_protocol::plugin::{LaunchesAnnounceRequest, launches_client::LaunchesClient};

/* === Public Functions === */

/// Announce Another Server's Address Through the Launch Credentials, Then Exit When the Story Closes Its Control Socket.
///
/// The optional mode selects a startup the story observes: `announce` (the default) announces and reports
/// `announced` on the control socket; `silent` reports its credentials and never announces; `exit` writes
/// numbered lines to standard error and exits with status 7 before announcing.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let control = arguments.next().expect("control address");
    let announced = arguments.next().expect("announced plugin address");
    let mode = arguments.next().unwrap_or_else(|| "announce".into());
    let endpoint = env::var("HARMOS_HOST_ENDPOINT").expect("the host supplies its launch endpoint");
    let token = env::var("HARMOS_LAUNCH_TOKEN").expect("the host supplies a launch token");
    if mode == "exit" {
        let mut stderr = std::io::stderr();
        for line in 1..=100 {
            writeln!(stderr, "startup failure line {line}").expect("standard error");
        }
        return ExitCode::from(7);
    }
    let mut control = TcpStream::connect(control).await.expect("control connection");
    match mode.as_str() {
        "silent" => control.write_all(format!("{endpoint} {token}\n").as_bytes()).await.expect("report credentials"),
        _ => {
            let mut client = LaunchesClient::connect(format!("http://{endpoint}")).await.expect("launch endpoint");
            client.announce(LaunchesAnnounceRequest { token, address: announced }).await.expect("announcement accepted");
            control.write_all(b"announced\n").await.expect("report announcement");
        }
    }
    let mut byte = [0];
    while control.read(&mut byte).await.is_ok_and(|read| read > 0) {}
    ExitCode::from(3)
}
