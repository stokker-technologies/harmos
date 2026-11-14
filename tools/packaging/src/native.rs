/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read Native Declarationss Through the Plugin Protocol.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

// Third Party Imports

use harmos_protocol::plugin::{InstanceGetRequest, InstanceGetResponse, InstanceStopRequest, instance_client::InstanceClient};

// Local Imports
use crate::{BuildCancellation, Error, process::Process};

/* === Constants === */

const TRANSPORT_GUARD: Duration = Duration::from_secs(30);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn describe(path: &Path, cancellation: &BuildCancellation) -> Result<InstanceGetResponse, Error> {
    std::thread::scope(|scope| {
        scope.spawn(|| collect(path, cancellation)).join().map_err(|_| std::io::Error::other("Plugin Declarations Worker Panicked"))?
    })
}

/* === Private Functions === */

fn collect(path: &Path, cancellation: &BuildCancellation) -> Result<InstanceGetResponse, Error> {
    // Start the Exact Stamped Executable in Managed Mode
    let mut command = Command::new(std::path::absolute(path)?);
    command.env_remove("HARMOS_ADDRESS").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut process = Process::spawn(command, cancellation)?;
    let stdout = process.stdout();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let result = reader.by_ref().take(4097).read_line(&mut line);
            match result {
                Ok(0) => {
                    let _ = sender.send(Err(std::io::Error::other("Plugin Exited Before Readiness")));
                    break;
                }
                Ok(_) if line.len() > 4096 => {
                    let _ = sender.send(Err(std::io::Error::other("Plugin Readiness Line Too Long")));
                    break;
                }
                Ok(_) => {
                    if let Some(address) = line.trim().strip_prefix("harmos listening ") {
                        let _ = sender.send(Ok(address.to_owned()));
                        std::io::copy(&mut reader, &mut std::io::sink())?;
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
        Ok::<_, std::io::Error>(())
    });

    // Get The Declarations, Then Stop Before Closing The Managed Process
    let result = (|| {
        let address = receiver.recv_timeout(TRANSPORT_GUARD).map_err(|error| Error::Settings(error.to_string()))??;
        let address: std::net::SocketAddr = address.parse().map_err(|_| Error::Settings("Invalid Plugin Address".into()))?;
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(Error::Settings("Managed Plugin Must Bind Loopback".into()));
        }
        cancellation.check()?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        runtime.block_on(async {
            tokio::time::timeout(TRANSPORT_GUARD, async {
                let mut client =
                    InstanceClient::connect(format!("http://{address}")).await.map_err(|error| Error::Settings(error.to_string()))?;
                client = client.max_decoding_message_size(crate::bundle::Limits::default().manifest_bytes);
                let request = InstanceGetRequest {
                    revision: 1,
                    host: "harmos-packaging".into(),
                    host_version: env!("CARGO_PKG_VERSION").into(),
                };
                let description = client.get(request).await.map_err(|error| Error::Settings(error.to_string()))?.into_inner();
                client
                    .stop(InstanceStopRequest { reason: "Declarations Collected".into() })
                    .await
                    .map_err(|error| Error::Settings(error.to_string()))?;
                if description.revision < 1 || description.id.is_empty() || description.declarations.is_none() {
                    return Err(Error::Settings("Get Plugin returned incomplete identity or declarations".into()));
                }
                semver::Version::parse(&description.version).map_err(|error| Error::Settings(error.to_string()))?;
                let declarations = description.declarations.as_ref().unwrap();
                if declarations.digest != harmos_protocol::schema::declarations_digest(declarations) {
                    return Err(Error::Settings("Invalid Plugin Declarations Digest".into()));
                }
                Ok(description)
            })
            .await
            .map_err(|_| Error::Settings("Plugin Declarations Timed Out".into()))?
        })
    })();

    // Retain Process and Reader Ownership on Both Success and Failure
    let cleanup = process.kill().and_then(|_| process.wait().map(|_| ()));
    let drained = reader.join().map_err(|_| std::io::Error::other("Plugin Readiness Reader Panicked"));
    cleanup?;
    drained??;
    cancellation.check()?;
    result
}

/* === Tests === */
