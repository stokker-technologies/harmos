/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "native_streams/plugin.rs"]
#[allow(dead_code, reason = "The Executable Owns SDK Handler Construction")]
mod plugin;

/* === Re-Exports === */

pub use plugin::Acknowledged;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use futures_util::StreamExt;
use harmos::encoding::Message as _;
use tokio::sync::watch;
use tokio::task::JoinHandle;

// Local Imports
use harmos_runtime::plugin::grpc::{Managed, Placement, PlacementSettings};
use harmos_runtime::{Runtime, streams::Item};

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _attached: harmos_runtime::plugin::grpc::Attached,
    failed: watch::Receiver<Option<String>>,
    relay: JoinHandle<()>,
}

#[derive(harmos_runtime::Row, Debug, Default, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Payload {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

/* === Implementations === */

impl harmos_runtime::streams::StreamRow for Payload {
    const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("consumption.payload", 16).label("Payload");
}

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

impl Fixture {
    pub async fn new() -> Self {
        let executable = std::env::current_exe().unwrap();
        let binary = format!("harmos-stream-consumer-fixture{}", std::env::consts::EXE_SUFFIX);
        let compiled = executable.parent().unwrap().parent().unwrap().join(binary);
        let path = crate::support::fixtures::binary("harmos-stream-consumer-fixture", compiled.to_str());
        Self::executable(&path).await
    }

    pub async fn executable(path: &std::path::Path) -> Self {
        // Keep Scenario Time on the Host's Virtual Clock
        let (clock, _) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
        let runtime = crate::support::app::launch::<Model>((), (), |setup| {
            setup.settings().clock(clock).register(stream_definitions()).operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(32 * 1024 * 1024).unwrap()),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let placement = Placement::Managed(Managed {
            executable: std::fs::canonicalize(path).unwrap(),
            arguments: Vec::new(),
            environment: Default::default(),
        });
        let settings = PlacementSettings {
            connect_timeout: std::time::Duration::from_secs(30),
            reconnect_delay: std::time::Duration::from_millis(100),
            stop_grace: std::time::Duration::from_secs(30),
            stream_window: NonZeroUsize::new(8).unwrap(),
        };
        let attached = runtime.attach_grpc(placement, settings).await.unwrap();

        // Forward Typed Host Rows Through Unary Plugin Calls in Stream Order
        let mut rows = runtime.streams.watch::<Payload>();
        let operations = runtime.operations.clone();
        let (failure, failed) = watch::channel(None);
        let relay = tokio::spawn(async move {
            while let Some(item) = rows.next().await {
                let Item::Row(row) = item else {
                    let _ = failure.send(Some("Host Stream Lost Rows".into()));
                    return;
                };
                let request = plugin::Payload {
                    sequence: row.value.sequence,
                    bytes: row.value.bytes.clone(),
                };
                if let Err(error) = operations.call_encoded("consumption.consume", request.encode_to_vec()).await {
                    let _ = failure.send(Some(error.to_string()));
                    return;
                }
            }
        });
        Self {
            runtime,
            _attached: attached,
            failed,
            relay,
        }
    }

    pub async fn acknowledged(&self, rows: u64) -> Acknowledged {
        let request = plugin::Progress { expected: rows };
        let call = self.runtime.operations.call_encoded("consumption.progress", request.encode_to_vec());
        let mut failed = self.failed.clone();
        let bytes = tokio::select! {
            result = call => result.unwrap(),
            _ = failed.changed() => panic!("Host Stream Relay Failed: {:?}", failed.borrow()),
        };
        Acknowledged::decode(bytes.as_slice()).unwrap()
    }

    pub fn publish(&self, sequences: std::ops::Range<u64>, bytes: usize) {
        for sequence in sequences {
            self.runtime.streams.publish(Payload {
                sequence,
                bytes: vec![(sequence % 251) as u8; bytes],
            });
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.relay.abort();
    }
}

/* === Public Functions === */

pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
    vec![<Payload as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap()]
}
