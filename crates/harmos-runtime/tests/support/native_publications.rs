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
use harmos_runtime::operations::{Handler, OperationDeclaration, OperationPolicy, RetryPolicy, Visibility, WireTable};
use harmos_runtime::plugin::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos_runtime::sidecar::{Attach, Notice, Refusal, Supervised, Supervisor};
use harmos_runtime::{Message, Runtime, services::ServiceContext, services::ServiceStatus};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(60),
    attempt_timeout: Duration::from_secs(60),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

type Source = harmos_sidecar::PluginNative<Producer>;
type Resident = Supervised<Model, Source, fn(&ServiceContext<Model>, Notice)>;

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _lease: harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>,
}

#[harmos_runtime::stream(id = "publication.sample", window = 1024)]
#[harmos_runtime::message]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    pub value: u64,
}

// One Producer Run: `rows` Rows In Calls Of `batch` Rows Each.
#[harmos_runtime::operation(id = "publication_publish",
    policy = POLICY,
    response = Timings,
    visibility = Visibility::Shared,
)]
pub struct PublishDeclaration {
    pub rows: u32,
    pub batch: u32,
}

// Little-Endian Nanoseconds Of Every Publication Call, In Call Order.
#[harmos_runtime::message]
pub struct Timings {
    pub calls: Vec<u8>,
}

pub struct Producer;

struct Publishing;

/* === Enums === */

#[harmos_runtime::catalog]
pub enum Operations {
    PublishDeclaration(PublishDeclaration),
}

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Application for Model {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl harmos_operations::lifecycle::Policy for Producer {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos_runtime::plugin(description = "Publishes Timed Sample Rows Through Host Publication.", id = "publication_producer", provides = Operations, requires(harmos_operations::streams::Publish), streams(Sample))]
impl harmos_runtime::Plugin for Producer {
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
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new()).implementation::<PublishDeclaration, Publishing>(),
        )
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl Handler for Publishing {
    type Request = PublishDeclaration;
    type Response = <PublishDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(_: harmos_runtime::OperationContext) -> Result<Self, Refusal> {
        Ok(Self)
    }
    async fn run(
        &mut self, host: harmos_runtime::OperationContext, burst: &mut PublishDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Timings>, Refusal> {
        let host = host.resource::<Supervisor>().expect("carrier context");
        let result: ::core::result::Result<Timings, Self::Error> = async move {
            let publisher = host.publisher::<Producer>();
            let size = burst.batch.max(1) as usize;
            let rows: Vec<_> = (0..u64::from(burst.rows)).map(|value| Sample { value }).collect();
            let mut calls = Vec::with_capacity(rows.len().div_ceil(size) * 8);
            let mut batch = publisher.batch::<Sample>();
            for chunk in rows.chunks(size) {
                // Rows Are Produced And Stamped Before The Measured Call
                batch.clear();
                for row in chunk {
                    batch.push(*row);
                }
                let started = Instant::now();
                if size == 1 {
                    publisher.publish_on_context::<Sample>(&chunk[0], Some("bench")).await.map_err(Refusal::from)?;
                } else {
                    publisher.publish_batch_on_context::<Sample>(&batch, Some("bench")).await.map_err(Refusal::from)?;
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
        self.calls.as_chunks::<8>().0.iter().map(|bytes| Duration::from_nanos(u64::from_le_bytes(*bytes))).collect()
    }
}

impl Fixture {
    pub async fn new() -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let source = Source::new(capacity).unwrap();
        let definition = PluginDefinition {
            id: source.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar { revision: "publication".into() },
            configuration: serde_json::to_vec(&()).unwrap(),
            requires: vec![],
            contributions: PluginContributions {
                operations: source.operations().unwrap(),
                wire_contracts: source.operation_contracts().unwrap().unwrap(),
                required_contracts: source.required_contracts().unwrap(),
                lifecycle: source.sdk_lifecycle().unwrap(),
                streams: source.publications(),
                observers: source.observers(),
                listeners: source.listener_declarations().unwrap().unwrap(),
                ..PluginContributions::default()
            },
        };
        let resident: Resident = Supervised::new(source, notices as fn(&ServiceContext<Model>, Notice));
        let runtime = Runtime::<Model>::builder((), ())
            .register(Lanes::catalog())
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(32 * 1024 * 1024).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(Arc::new(resident.clone())).unwrap();
                    dependencies
                })
                .operation::<harmos_operations::streams::Publish>(),
                Lanes::Open,
            ))
            .await
            .unwrap();
        runtime
            .operations
            .register_service(
                harmos_runtime::services::ServiceDefinition {
                    id: "producer".into(),
                    policy: harmos_runtime::ServicePolicy {
                        restart: harmos_runtime::operations::RestartPolicy::Never,
                        ..harmos_runtime::services::POLICY
                    },
                },
                move |_: Arc<()>| {
                    harmos_runtime::sidecar::Managed::new(
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
        let lease = runtime.operations.submit_resident("producer").await.unwrap();
        let mut statuses = runtime.operations.watch_residents();
        loop {
            let status = statuses
                .borrow_and_update()
                .get(&harmos_runtime::operations::resident::Key::root("producer"))
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
        Self { runtime, _lease: lease }
    }

    // PublishDeclaration `rows` rows in calls of `batch` rows; answers each call's latency.
    pub async fn publish(&self, rows: u32, batch: u32) -> Vec<Duration> {
        let bytes = self
            .runtime
            .operations
            .call_encoded(&format!("publication_producer.{}", PublishDeclaration::ID), PublishDeclaration { rows, batch }.encode())
            .await
            .unwrap();
        Timings::decode(&bytes).unwrap().durations()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn notices(_: &ServiceContext<Model>, _: Notice) {}

/* === Tests === */
