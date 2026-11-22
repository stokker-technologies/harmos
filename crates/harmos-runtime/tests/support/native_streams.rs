/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[allow(dead_code, reason = "The Executable Owns SDK Handler Construction")]
mod stream_consumer;

/* === Re-Exports === */

pub use stream_consumer::Acknowledged;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// Third Party Imports
use harmos::encoding::Message;
use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;

// Local Imports
use harmos_runtime::Runtime;
use harmos_runtime::plugin::grpc::{Managed, Placement, PlacementSettings};

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _attached: harmos_runtime::plugin::grpc::Attached,
    failed: watch::Receiver<Option<String>>,
    consumer: JoinHandle<()>,
}

#[derive(harmos_runtime::Row, Debug, Default, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Payload {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

struct Admission {
    marker: Arc<Notify>,
    seen: Arc<AtomicBool>,
}

/* === Implementations === */

impl harmos_runtime::operations::middleware::Middleware for Admission {
    fn enter<'a>(
        &'a self, call: &'a mut harmos_runtime::operations::middleware::Invocation,
    ) -> harmos_runtime::operations::middleware::Enter<'a> {
        Box::pin(async move {
            if call.operation().id.as_str() == "harmos.plugin.v1.Subscriptions/Subscribe" {
                assert!(Arc::ptr_eq(call.context().get::<Arc<Notify>>().unwrap(), &self.marker));
                assert_eq!(call.context().get::<harmos_runtime::Scope>().unwrap().as_str(), "unit/inherited");
                self.seen.store(true, Ordering::Release);
            }
            Ok(())
        })
    }
}

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
        let marker = Arc::new(Notify::new());
        let seen = Arc::new(AtomicBool::new(false));
        let admission = Arc::new(Admission {
            marker: marker.clone(),
            seen: seen.clone(),
        });
        let runtime = crate::support::app::start::<Model>((), (), |setup| {
            setup.settings().operation_middleware(admission).clock(clock).register(stream_definitions()).operation_settings(
                harmos_runtime::operations::Settings {
                    preparation_bytes: Some(NonZeroUsize::new(32 * 1024 * 1024).unwrap()),
                    ..Default::default()
                },
            );
        })
        .await
        .unwrap();
        let placement = Placement::Managed(Managed {
            executable: std::fs::canonicalize(path).unwrap(),
            directory: None,
            arguments: Vec::new(),
            environment: Default::default(),
        });
        let settings = PlacementSettings {
            reconnect_delay: std::time::Duration::from_millis(100),
            stop_grace: std::time::Duration::from_secs(30),
            stream_window: NonZeroUsize::new(8).unwrap(),
            interval_window: std::time::Duration::from_secs(1),
            interval_lag: Default::default(),
        };
        let attached = runtime.attach_grpc(placement, settings).await.unwrap();

        // Open the Plugin's Admitted Subscription Before Publishing.
        let request = stream_consumer::Progress { expected: 0 };
        let mut context = harmos_runtime::operations::CallContext::default();
        context.insert(marker).unwrap();
        context.insert(harmos_runtime::Scope::new("unit/inherited").unwrap()).unwrap();
        let consuming = runtime.operations.with_context(context).submit_encoded("consumption.consume", request.encode_to_vec()).unwrap();
        let (failure, failed) = watch::channel(None);
        let consumer = tokio::spawn(async move {
            if let Err(error) = consuming.await {
                let _ = failure.send(Some(format!("{error:?}")));
            }
        });
        let fixture = Self {
            runtime,
            _attached: attached,
            failed,
            consumer,
        };
        fixture.acknowledged(0).await;
        assert!(seen.load(Ordering::Acquire));
        fixture
    }

    pub async fn acknowledged(&self, rows: u64) -> Acknowledged {
        let request = stream_consumer::Progress { expected: rows };
        let call = self.runtime.operations.call_encoded("consumption.progress", request.encode_to_vec());
        let mut failed = self.failed.clone();
        let bytes = tokio::select! {
            result = call => result.unwrap(),
            _ = failed.changed() => panic!("Host Stream Subscription Failed: {:?}", failed.borrow()),
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
        self.consumer.abort();
    }
}

/* === Public Functions === */

pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
    vec![<Payload as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap()]
}
