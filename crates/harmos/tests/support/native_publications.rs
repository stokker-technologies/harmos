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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use harmos::operations::{
    Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility,
    WireOperation, WireTable,
};
use harmos::plugin::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos::sidecar::{Attach, Notice, Refusal, Supervised, Supervisor};
use harmos::{Message, Runtime, services::Service, services::ServiceStatus};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(60),
    attempt_timeout: Duration::from_secs(60),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */
type Source = harmos_sidecar::PluginNative<Producer>;
type Resident = Supervised<Model, Source, fn(&Service<Model>, Notice)>;

/* === Structs === */
pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _lease: harmos::operations::OperationInstance<harmos::operations::resident::Activation>,
}

#[harmos::stream(id = "publication.sample", window = 1024)]
#[harmos::message]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    pub value: u64,
}

// One Producer Run: `rows` Rows In Calls Of `batch` Rows Each.
#[harmos::message]
pub struct Burst {
    pub rows: u32,
    pub batch: u32,
}

// Little-Endian Nanoseconds Of Every Publication Call, In Call Order.
#[harmos::message]
pub struct Timings {
    pub calls: Vec<u8>,
}

pub struct Producer;

pub struct Publish;

struct Publishing;

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    Publish(Publish),
}

#[harmos::lanes]
pub enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Model {
    type State = ();
    type Transaction = harmos::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl harmos::operations::lifecycle::Policy for Producer {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(description = "Publishes Timed Sample Rows Through Host Publication.", id = "publication-producer", provides = Operations, requires(harmos::operations::host::Publish), streams(Sample))]
impl harmos::Plugin for Producer {
    type Config = ();
    type Context = Supervisor;
    type Resources = ();
    type Error = Refusal;
    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self)
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        Arc::new(())
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<Publishing>(self.resources())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl OperationDeclaration for Publish {
    type Request = Burst;
    type Response = Timings;
    const ID: &'static str = "publication.publish";
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = POLICY;
}

impl WireOperation for Publish {
    const VERSION: u32 = 1;
}

impl Operation for Publishing {
    type Declaration = Publish;
    type Resources = ();
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        host: Self::Context<'a>,
        burst: &mut Burst,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Timings>, Refusal> {
        let result: ::core::result::Result<Timings, Self::Error> = async move {
            let publisher = host.publisher::<Producer>();
            let size = burst.batch.max(1) as usize;
            let rows: Vec<_> = (0..u64::from(burst.rows))
                .map(|value| Sample { value })
                .collect();
            let mut calls = Vec::with_capacity(rows.len().div_ceil(size) * 8);
            for chunk in rows.chunks(size) {
                let started = Instant::now();
                if size == 1 {
                    publisher
                        .publish_on_context::<Sample>(&chunk[0], Some("bench"))
                        .await
                        .map_err(Refusal::of)?;
                } else {
                    publisher
                        .publish_batch_on_context::<Sample>(chunk, Some("bench"))
                        .await
                        .map_err(Refusal::of)?;
                }
                let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                calls.extend_from_slice(&elapsed.to_le_bytes());
            }
            Ok(Timings { calls })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Timings {
    pub fn durations(&self) -> Vec<Duration> {
        self.calls
            .as_chunks::<8>()
            .0
            .iter()
            .map(|bytes| Duration::from_nanos(u64::from_le_bytes(*bytes)))
            .collect()
    }
}

impl Fixture {
    pub async fn new() -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let source = Source::new(capacity).unwrap();
        let definition = PluginDefinition {
            id: source.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar {
                revision: "publication".into(),
            },
            configuration: serde_json::to_vec(&()).unwrap(),
            requires: vec![],
            contributions: PluginContributions {
                operations: source.operations().unwrap(),
                wire_contracts: source.operation_contracts().unwrap().unwrap(),
                required_contracts: source.required_contracts().unwrap(),
                lifecycle: source.sdk_lifecycle().unwrap(),
                streams: source.publications(),
                observers: source.observers(),
                observer_control: source.observer_control().unwrap(),
                listeners: source.listener_declarations().unwrap().unwrap(),
                ..PluginContributions::default()
            },
        };
        let resident: Resident = Supervised::new(source, notices as fn(&Service<Model>, Notice));
        let runtime = Runtime::<Model>::builder((), ())
            .register(Lanes::catalog())
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(32 * 1024 * 1024).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        runtime
            .operations
            .register_wire_operation::<harmos::sidecar::Publish<_, _, _>>(
                "native-publication",
                Lanes::Open,
                Arc::new(resident.clone()),
            )
            .unwrap();
        runtime
            .operations
            .register_service(
                harmos::services::ServiceDefinition {
                    id: "producer".into(),
                    policy: harmos::operations::OperationPolicy {
                        restart: harmos::operations::RestartPolicy::Never,
                        ..harmos::services::POLICY
                    },
                },
                move |_: Arc<()>| {
                    harmos::sidecar::Managed::new(
                        resident.clone(),
                        definition.clone(),
                        Lanes::Open,
                        capacity,
                        NonZeroUsize::new(4096).unwrap(),
                    )
                    .unwrap()
                },
            )
            .unwrap();
        let lease = runtime
            .operations
            .submit_resident("producer")
            .await
            .unwrap();
        let mut statuses = runtime.operations.watch_residents();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = statuses
                    .borrow_and_update()
                    .get(&harmos::operations::resident::Key::root("producer"))
                    .and_then(|item| item.health.clone());
                match status {
                    Some(ServiceStatus::Running { .. }) => break,
                    Some(ServiceStatus::Stopped) => {
                        panic!("producer failed to start: {status:?}")
                    }
                    _ => {}
                }
                statuses.changed().await.unwrap();
            }
        })
        .await
        .expect("native producer becomes ready");
        Self {
            runtime,
            _lease: lease,
        }
    }

    // Publish `rows` rows in calls of `batch` rows; answers each call's latency.
    pub async fn publish(&self, rows: u32, batch: u32) -> Vec<Duration> {
        let bytes = tokio::time::timeout(
            Duration::from_secs(60),
            self.runtime
                .operations
                .call_encoded(Publish::ID, Burst { rows, batch }.encode_to_vec()),
        )
        .await
        .expect("producer publishes within the bench deadline")
        .unwrap();
        Timings::decode(&bytes).unwrap().durations()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Model>, _: Notice) {}

/* === Tests === */
