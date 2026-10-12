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
use std::time::Duration;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos::load::{PluginArtifact, PluginContributions, PluginDefinition};
use harmos::operations::OperationDeclaration;
use harmos::sidecar::{Attach, Executable, Notice, Supervised};
use harmos::{Message, Runtime, Service, ServiceStatus};
use plugin::{Progress, Receiver};

/* === Constants === */
/* === Macros === */

/* === Type Aliases === */
type Resident<T> = Supervised<Model, T, fn(&Service<Model>, Notice)>;
type Managed<T> = harmos::sidecar::Managed<Model, T, fn(&Service<Model>, Notice), Lanes>;
type Constructor<T> = fn(
    Resident<T>,
    PluginDefinition,
    Lanes,
    NonZeroUsize,
    NonZeroUsize,
) -> harmos::Result<Managed<T>>;

/* === Structs === */
pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    _lease: harmos::services::ServiceLease<Model>,
}

/* === Enums === */
#[harmos::streams]
pub enum Streams {
    Payload(Payload),
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
    type Transaction = harmos::Never;
    type Record = ();
    type Streams = Streams;
    type Resources = ();
}

impl Fixture {
    pub async fn new() -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let source = harmos_sidecar::PluginNative::<Receiver>::new(capacity).unwrap();
        Self::attached(
            source,
            PluginArtifact::LocalSidecar {
                revision: "stream-consumption".into(),
            },
            "1.0.0".parse().unwrap(),
            harmos::sidecar::Managed::new,
            None,
        )
        .await
    }

    pub async fn executable(path: &std::path::Path) -> Self {
        Self::executable_paused(path, None).await
    }

    pub async fn executable_paused(path: &std::path::Path, pause_after: Option<u64>) -> Self {
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
        Self::attached(
            source,
            artifact,
            version,
            harmos::sidecar::Managed::new_executable,
            pause_after,
        )
        .await
    }

    async fn attached<T: Attach>(
        source: T,
        artifact: PluginArtifact,
        version: semver::Version,
        construct: Constructor<T>,
        pause_after: Option<u64>,
    ) -> Self {
        let capacity = NonZeroUsize::new(8).unwrap();
        let definition = PluginDefinition {
            id: source.owner().into(),
            version,
            artifact,
            configuration: serde_json::to_vec(&pause_after).unwrap(),
            requires: vec![],
            contributions: PluginContributions {
                operations: source.operations().unwrap(),
                wire_contracts: source.operation_contracts().unwrap().unwrap(),
                required_contracts: source.required_contracts().unwrap(),
                lifecycle: source.sdk_lifecycle().unwrap(),
                streams: source.publications(),
                observers: source.observers(),
                observer_controls: source.observer_controls().unwrap(),
                listeners: source.listener_declarations().unwrap().unwrap(),
                ..PluginContributions::default()
            },
        };
        let resident = Supervised::new(source, notices as fn(&Service<Model>, Notice));
        let runtime = Runtime::<Model>::builder((), ())
            .register(Streams::catalog())
            .register(Lanes::catalog())
            .operation_preparation_bytes(NonZeroUsize::new(32 * 1024 * 1024).unwrap())
            .runtime()
            .await
            .unwrap();
        runtime
            .services
            .register(
                harmos::services::ServiceDefinition {
                    id: "consumer".into(),
                    policy: harmos::ServicePolicy {
                        credit: 0,
                        ..Default::default()
                    },
                },
                move |_: Arc<()>| {
                    construct(
                        resident.clone(),
                        definition.clone(),
                        Lanes::Open,
                        capacity,
                        NonZeroUsize::new(4096).unwrap(),
                    )
                    .unwrap()
                    .stream_listeners(|_, bindings| bindings.stream::<Payload>(0))
                },
            )
            .unwrap();
        let lease = runtime
            .services
            .activate("consumer", &runtime.services.root_scope())
            .await
            .unwrap();
        let mut statuses = runtime.services.subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = statuses
                    .borrow_and_update()
                    .get(&harmos::services::ServiceKey::root("consumer"))
                    .map(|item| item.status.clone());
                match status {
                    Some(ServiceStatus::Running { .. }) => break,
                    Some(ServiceStatus::Stopped) => {
                        panic!("consumer failed to start: {status:?}")
                    }
                    _ => {}
                }
                statuses.changed().await.unwrap();
            }
        })
        .await
        .expect("native consumer becomes ready");
        Self {
            runtime,
            _lease: lease,
        }
    }

    pub async fn acknowledged(&self, rows: u64) -> Acknowledged {
        let bytes = tokio::time::timeout(
            Duration::from_secs(5),
            self.runtime
                .operations
                .call_encoded(Progress::ID, rows.encode_to_vec()),
        )
        .await
        .expect("consumer acknowledges rows")
        .unwrap();
        Acknowledged::decode(&bytes).unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Model>, _: Notice) {}

/* === Tests === */
