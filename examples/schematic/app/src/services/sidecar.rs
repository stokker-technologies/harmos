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
use std::sync::Arc;
use std::{env, error, fs, io};

// Third Party Imports

// Local Imports
use crate::app::Runtime;
use harmos_packaging::bundle::{self, Identity, Source};
use harmos_protocol::host::{
    PluginManaged, PluginPlacement, PluginState, PluginValue, PluginsCreateRequest, PluginsDeleteRequest, PluginsGetRequest,
    PluginsWatchRequest, plugin_change::Kind, plugin_placement, plugins_watch_response::Event,
};
use harmos_runtime::operations::{Lane, Registration};
use harmos_runtime::plugin::manager::{Management, Manager};
use harmos_runtime::{Error, Identifier, Result};
use harmos_storage::{FileMetadata, StorageManager};
use schematic_operations::schematic::upload::AnalysisResources;
use schematic_operations::{AnalyzeExchange, AnalyzeSchematic, AnalyzeUpload, transfer};

/* === Constants === */

pub const NAME: &str = "schematic";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Running {
    manager: Arc<Manager>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Running {
    pub async fn stop(self) -> Result<()> {
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
        Ok(())
    }
}

/* === Public Functions === */

pub async fn start(runtime: &Runtime, path: &Path, directory: &Path) -> Result<Running> {
    let executable = fs::canonicalize(path).map_err(failed)?;
    let directory = fs::canonicalize(directory).map_err(failed)?;

    // Keep Operation Streaming in the Host
    let resources = runtime.operations.context().resource::<AnalysisResources>()?;
    let streams = runtime.streams.clone();
    resources.progress.get_or_init(|| Arc::new(move |row| streams.publish(row)));
    let registration = Registration::new(Lane::default())
        .protobuf::<AnalyzeSchematic>()?
        .protobuf::<AnalyzeUpload>()?
        .protobuf::<AnalyzeExchange>()?
        .protobuf::<transfer::Upload>()?
        .protobuf::<transfer::Download>()?
        .protobuf::<transfer::Put>()?
        .protobuf::<transfer::Take>()?;
    runtime.register_batch(registration).await?;

    // Package the Sidecar for the Existing Managed Registry
    let bytes = fs::read(executable).map_err(failed)?;
    let program = format!("schematic-sidecar{}", env::consts::EXE_SUFFIX);
    let directory = directory.to_str().ok_or(Error::InvalidInput {
        field: "schematic directory",
        reason: "select a directory with a UTF-8 path",
    })?;
    let launch = vec![format!("./{program}"), directory.into()];
    let bundle = bundle::build(
        Identity {
            name: NAME.into(),
            version: env!("CARGO_PKG_VERSION").parse().map_err(failed)?,
            description: "Schematic Tools".into(),
            package: Default::default(),
        },
        &[Source {
            path: &program,
            bytes: &bytes,
            executable: None,
        }],
        Some(&launch),
        Default::default(),
    )
    .map_err(failed)?;

    // Publish Immutable Content Through the Host's Storage Owner
    let storage = runtime.operations.context().resource::<StorageManager>()?;
    let mut production = storage
        .produce_file(FileMetadata {
            id: Identifier::new("schematic-tools").map_err(failed)?,
            scope: None,
            media_type: "application/x-tar".into(),
            expected_length: Some(bundle.bytes().len() as u64),
            expected_sha256: None,
        })
        .await
        .map_err(failed)?;
    production.write_all(bundle.bytes().to_vec()).await.map_err(failed)?;
    let file = production.finish().await.map_err(failed)?;
    storage.flush_until(&file.position).await.map_err(failed)?;
    let sha256 = file.sha256.to_vec();

    // Let the Registry Own Launch, Settings and Process Settlement
    let manager = runtime.operations.context().resource::<Manager>()?;
    let mut watch = manager.watch(PluginsWatchRequest { name: Some(NAME.into()), at: None }).await.map_err(failed)?;
    manager
        .create(PluginsCreateRequest {
            origin: None,
            request_id: "schematic-tools".into(),
            name: NAME.into(),
            placement: Some(PluginPlacement {
                kind: Some(plugin_placement::Kind::Managed(PluginManaged { content: Some(file.into()), sha256 })),
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
            return Err(failed(io::Error::other(failure.message)));
        }
        if entry.state == PluginState::Ready as i32 {
            break;
        }
    }
    Ok(Running { manager })
}

/* === Private Functions === */

fn failed(source: impl error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: "Manage Schematic Plugin".into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
