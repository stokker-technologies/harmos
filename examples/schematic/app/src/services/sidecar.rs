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
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

// Third Party Imports

// Local Imports
use crate::app::Runtime;
use harmos_protocol::client::{
    PluginPlacement, PluginRemote, PluginState, PluginValue, PluginsCreateRequest, PluginsDeleteRequest, PluginsGetRequest,
    PluginsWatchRequest, plugin_change::Kind, plugin_placement, plugins_watch_response::Event,
};
use harmos_runtime::clock::Clock;
use harmos_runtime::plugin::manager::{Management, Manager, PluginManagerSettings};
use harmos_runtime::{Error, Result};

/* === Constants === */

pub const NAME: &str = "schematic_tools";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Running {
    manager: Arc<Manager>,
    settings: Arc<PluginManagerSettings>,
    clock: Clock,
    child: Child,
    output: JoinHandle<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Running {
    pub async fn stop(mut self) -> Result<()> {
        let entry =
            self.manager.get(PluginsGetRequest { name: NAME.into(), at: None }).await.map_err(failed)?.entry.ok_or(Error::NotReady {
                domain: "Stop Schematic Plugin: Missing Entry",
            })?;
        self.manager
            .delete(PluginsDeleteRequest {
                name: NAME.into(),
                expected_revision: entry.revision,
            })
            .await
            .map_err(failed)?;
        let status = self
            .clock
            .timeout(self.settings.placement.stop_grace, self.child.wait())
            .await
            .map_err(|_| Error::NotReady {
                domain: "Stop Schematic Plugin: Process Exit Timed Out",
            })?
            .map_err(failed)?;
        self.output.await.map_err(failed)?;
        if !status.success() {
            return Err(Error::NotReady {
                domain: "Stop Schematic Plugin: Process Failed",
            });
        }
        Ok(())
    }
}

/* === Public Functions === */

pub async fn start(runtime: &Runtime, path: &Path, directory: &Path) -> Result<Running> {
    let executable = std::fs::canonicalize(path).map_err(failed)?;
    let directory = std::fs::canonicalize(directory).map_err(failed)?;

    // Keep Operation Streaming in the Host
    let resources = runtime.operations.context().resource::<schematic_operations::schematic::upload::AnalysisResources>()?;
    let streams = runtime.streams.clone();
    resources.progress.get_or_init(|| Arc::new(move |row| streams.publish(row)));
    let registration = harmos_runtime::operations::Registration::new(harmos_runtime::operations::Lane::default())
        .protobuf::<schematic_operations::AnalyzeSchematic>()?
        .protobuf::<schematic_operations::AnalyzeUpload>()?
        .protobuf::<schematic_operations::AnalyzeExchange>()?
        .protobuf::<schematic_operations::transfer::Upload>()?
        .protobuf::<schematic_operations::transfer::Download>()?
        .protobuf::<schematic_operations::transfer::Put>()?
        .protobuf::<schematic_operations::transfer::Take>()?;
    runtime.register_batch(registration).await?;

    // Launch the Example's Process and Register Its Entry With Library-Owned Settings
    let manager = runtime.operations.context().resource::<Manager>()?;
    let settings = runtime.operations.context().resource::<PluginManagerSettings>()?;
    let clock = runtime.operations.clock().clone();
    let mut command = Command::new(executable);
    command
        .env("SCHEMATIC_DIRECTORY", directory)
        .env("HARMOS_ADDRESS", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(failed)?;
    let mut lines = BufReader::new(child.stdout.take().ok_or(Error::NotReady {
        domain: "Start Schematic Plugin: Missing Output",
    })?)
    .lines();
    let endpoint = async {
        loop {
            let line = lines.next_line().await.map_err(failed)?.ok_or(Error::NotReady {
                domain: "Start Schematic Plugin: Missing Endpoint",
            })?;
            if let Some(address) = line.strip_prefix("harmos listening ") {
                break Ok::<_, Error>(format!("http://{address}"));
            }
        }
    };
    let address = clock.timeout(settings.placement.connect_timeout, endpoint).await.map_err(|_| Error::NotReady {
        domain: "Start Schematic Plugin: Endpoint Timed Out",
    })??;
    let mut watch = manager.watch(PluginsWatchRequest { name: NAME.into(), at: None }).await.map_err(failed)?;
    manager
        .create(PluginsCreateRequest {
            request_id: "schematic-tools".into(),
            name: NAME.into(),
            placement: Some(PluginPlacement {
                kind: Some(plugin_placement::Kind::Remote(PluginRemote { url: address })),
            }),
            value: Some(PluginValue { enabled: true }),
        })
        .await
        .map_err(failed)?;
    loop {
        let response = watch.recv().await.ok_or(Error::NotReady {
            domain: "Start Schematic Plugin: Entry Watch Closed",
        })?;
        let entry = match response.event {
            Some(Event::Snapshot(snapshot)) => snapshot.entries.into_iter().find(|entry| entry.name == NAME),
            Some(Event::Change(change)) => match change.kind {
                Some(Kind::Entry(entry)) => Some(entry),
                _ => None,
            },
            _ => None,
        };
        let Some(entry) = entry else {
            continue;
        };
        if let Some(failure) = entry.failure {
            return Err(failed(std::io::Error::other(failure.message)));
        }
        if entry.state == PluginState::Ready as i32 {
            break;
        }
    }
    let output = tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
    Ok(Running { manager, settings, clock, child, output })
}

/* === Private Functions === */

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: "Manage Schematic Plugin".into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
