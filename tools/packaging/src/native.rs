/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read Native Declarations Through the Plugin Protocol.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

// Third Party Imports
use harmos_protocol::plugin::{InstanceGetRequest, InstanceGetResponse, InstanceStopRequest, instance_client::InstanceClient};
use harmos_runtime::plugin::grpc::Launches;

// Local Imports
use crate::{BuildCancellation, Error, process::Process};

/* === Constants === */

const TRANSPORT_GUARD: Duration = Duration::from_secs(30);

/// How Often the Announcement Wait Checks Whether the Plugin Exited or the Build Was Cancelled.
const EXIT_POLL: Duration = Duration::from_millis(20);

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
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let launches = Launches::new();
    let mut launch = runtime.block_on(launches.reserve(path.display().to_string())).map_err(|error| Error::Settings(error.to_string()))?;

    // Start the Exact Stamped Executable in Managed Mode; It Announces Instead of Printing.
    let mut command = Command::new(std::path::absolute(path)?);
    command.envs(launch.environment()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit());
    let mut process = Process::spawn(command, cancellation)?;

    // Get The Declarations, Then Stop Before Closing The Managed Process
    let result = runtime.block_on(async {
        let waited = tokio::time::Instant::now();
        let address = loop {
            if let Ok(announced) = tokio::time::timeout(EXIT_POLL, launch.announced()).await {
                break announced.map_err(|error| Error::Settings(error.to_string()))?;
            }
            if let Some(status) = process.exited()? {
                return Err(Error::Settings(format!("Plugin Exited With {status} Before Announcing Its Address")));
            }
            cancellation.check()?;
            if waited.elapsed() >= TRANSPORT_GUARD {
                return Err(Error::Settings("Plugin Announcement Timed Out".into()));
            }
        };
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
    });
    drop(launch);
    runtime.block_on(launches.shutdown());

    // Retain Process Ownership on Both Success and Failure
    let cleanup = process.kill().and_then(|_| process.wait().map(|_| ()));
    cleanup?;
    cancellation.check()?;
    result
}

/* === Tests === */
