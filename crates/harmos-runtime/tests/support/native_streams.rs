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
mod plugin;

/* === Re-Exports === */

pub use plugin::{Acknowledged, Payload};

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos_runtime::operations::OperationDeclaration;
use harmos_runtime::plugin::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos_runtime::sidecar::{Attach, Executable, Notice, Supervised};
use harmos_runtime::{Message, Runtime, services::ServiceContext, services::ServiceStatus};
use plugin::Receiver;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Resident<T> = Supervised<Model, T, fn(&ServiceContext<Model>, Notice)>;
type Managed<T> = harmos_runtime::sidecar::Managed<Model, T, fn(&ServiceContext<Model>, Notice), Lanes>;
type Constructor<T> = fn(Resident<T>, PluginDefinition, Lanes, NonZeroUsize, NonZeroUsize) -> harmos_runtime::Result<Managed<T>>;

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _lease: harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>,
}

/* === Enums === */

#[harmos_runtime::streams]
pub enum Streams {
    Payload(Payload),
}

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _setup: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = Streams;
}

impl Fixture {
    pub async fn new() -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let source = harmos_sidecar::PluginNative::<Receiver>::new(capacity).unwrap();
        Self::attached(
            source,
            PluginArtifact::LocalSidecar { revision: "stream-consumption".into() },
            "1.0.0".parse().unwrap(),
            harmos_runtime::sidecar::Managed::new,
        )
        .await
    }

    pub async fn executable(path: &std::path::Path) -> Self {
        let path = std::fs::canonicalize(path).unwrap();
        let source = Executable::at(&path).unwrap();
        let version = source.declaration().version.clone();
        let artifact = PluginArtifact::Sidecar {
            sha256: Sha256::digest(std::fs::read(&path).unwrap()).into(),
            path,
            arguments: Vec::new(),
            directory: None,
            environment: Default::default(),
        };
        Self::attached(source, artifact, version, harmos_runtime::sidecar::Managed::new_executable).await
    }

    async fn attached<T: Attach>(source: T, artifact: PluginArtifact, version: semver::Version, construct: Constructor<T>) -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let definition = PluginDefinition {
            id: source.owner().into(),
            version,
            artifact,
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
        let resident = Supervised::new(source, notices as fn(&ServiceContext<Model>, Notice));
        let runtime = Runtime::<Model>::launch_with((), (), |setup| {
            setup.settings().register(Streams::catalog()).operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(32 * 1024 * 1024).unwrap()),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        runtime
            .operations
            .register_service(
                harmos_runtime::services::ServiceDefinition {
                    id: "consumer".into(),
                    policy: harmos_runtime::ServicePolicy {
                        restart: harmos_runtime::operations::RestartPolicy::Never,
                        ..harmos_runtime::services::POLICY
                    },
                },
                move |_: Arc<harmos_sdk::ExecutionContext>| {
                    construct(resident.clone(), definition.clone(), Lanes::Open, capacity, NonZeroUsize::new(4096).unwrap())
                        .unwrap()
                        .stream_listeners(|_, bindings| bindings.stream::<Payload>(0))
                },
            )
            .unwrap();
        let lease = runtime.operations.submit_resident("consumer").await.unwrap();
        let mut statuses = runtime.operations.watch_residents();
        loop {
            let status = statuses
                .borrow_and_update()
                .get(&harmos_runtime::operations::resident::Key::root("consumer"))
                .and_then(|item| item.health.clone());
            match status {
                Some(ServiceStatus::Running { .. }) => break,
                Some(ServiceStatus::Stopped) => {
                    panic!("consumer failed to start: {status:?}")
                }
                _ => {}
            }
            statuses.changed().await.unwrap();
        }
        Self { runtime, _lease: lease }
    }

    pub async fn acknowledged(&self, rows: u64) -> Acknowledged {
        let bytes =
            self.runtime.operations.call_encoded(&format!("consumption.{}", plugin::ProgressDeclaration::ID), rows.encode()).await.unwrap();
        Acknowledged::decode(&bytes).unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn notices(_: &ServiceContext<Model>, _: Notice) {}

/* === Tests === */
