/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

#[path = "../support/process_contract.rs"]
mod contract;
use crate::support::fixtures as prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{HostLanes, assembly, process_definition, service_status};
use crate::support::Bank;
use contract::{Blob, Echo, Nested, Operations, Probe};
use harmos::operations::{Context, Operation};
use harmos::plugin::sidecar::{Executable, Notice, Supervised};
use harmos::{Runtime, services::Service, services::ServicePolicy, services::ServiceStatus};

/* === Constants === */
const CAPACITY: usize = 2;

/* === Macros === */

/* === Type Aliases === */
type Resident = Supervised<Bank, Executable, fn(&Service<Bank>, Notice)>;
type PublicationCall =
    harmos::plugin::sidecar::Publish<Bank, Executable, fn(&Service<Bank>, Notice)>;

/* === Structs === */
struct Callback {
    resident: Arc<Resident>,
    mode: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

struct Fixture {
    runtime: Runtime<Bank>,
    _lease: harmos::services::ServiceLease<Bank>,
    mode: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    root: tempfile::TempDir,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

struct CallbackPublication {
    owner: Arc<Callback>,
    publication: PublicationCall,
}

impl Operation for CallbackPublication {
    type Declaration = harmos::operations::host::Publish;
    type Resources = Callback;
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(owner: Arc<Callback>) -> harmos::Result<Self> {
        let publication = harmos::plugin::sidecar::Publish::new(Arc::clone(&owner.resident))?;
        Ok(Self { owner, publication })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut harmos::operations::host::Publication,
        _endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<()> {
        self.owner.calls.fetch_add(1, Ordering::SeqCst);
        let id = match self.owner.mode.load(Ordering::SeqCst) {
            1 => 0,
            2 => u64::MAX,
            _ => request.sequence.unwrap(),
        };
        let echoed = context
            .call::<Echo>(Blob {
                id,
                data: request.payload.clone(),
            })
            .await?;
        assert_eq!(
            echoed.data, request.payload,
            "the original connection carried the nested response"
        );
        self.publication.run(context, request, _endpoints).await
    }
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = Executable::at(prepared_fixtures::artifact("harmos-process-fixture")).unwrap();
        let mut definition = process_definition(&source);
        definition.configuration = serde_json::to_vec(root.path().to_str().unwrap()).unwrap();
        let resident = Supervised::new(source, notices as fn(&Service<Bank>, Notice));
        let runtime = assembly()
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(
                    NonZeroUsize::new(harmos_sidecar::Frame::LIMIT * 4).unwrap(),
                ),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        let mode = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        runtime
            .operations
            .register_wire_operation::<CallbackPublication>(
                "process-core",
                HostLanes::Open,
                Arc::new(Callback {
                    resident: Arc::new(resident.clone()),
                    mode: Arc::clone(&mode),
                    calls: Arc::clone(&calls),
                }),
            )
            .unwrap();
        runtime
            .services
            .register(
                harmos::services::ServiceDefinition {
                    id: "process".into(),
                    policy: ServicePolicy {
                        credit: 0,
                        ..ServicePolicy::default()
                    },
                },
                move |_: Arc<()>| {
                    harmos::plugin::sidecar::Managed::new_executable_catalog::<Operations>(
                        resident.clone(),
                        definition.clone(),
                        HostLanes::Open,
                        NonZeroUsize::new(CAPACITY).unwrap(),
                        NonZeroUsize::new(4096).unwrap(),
                    )
                    .unwrap()
                },
            )
            .unwrap();
        let lease = runtime
            .services
            .activate("process", &runtime.services.root_scope())
            .await
            .unwrap();
        service_status(runtime.services.subscribe(), "process", |status| {
            matches!(status, ServiceStatus::Running { .. })
        })
        .await;
        Self {
            runtime,
            _lease: lease,
            mode,
            calls,
            root,
        }
    }
    fn path(&self) -> &Path {
        self.root.path()
    }
    async fn finish(self) {
        tokio::time::timeout(Duration::from_secs(5), self.runtime.stop())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(count(self.path(), "shutdown"), 1);
        assert_eq!(count(self.path(), "cleaned"), 1);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

fn count(root: &Path, name: &str) -> usize {
    std::fs::read_to_string(root.join(name))
        .unwrap()
        .lines()
        .count()
}

async fn marked(root: &Path, name: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !std::fs::read(root.join(name)).is_ok_and(|bytes| bytes.ends_with(b"\n")) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

fn blob(id: u64, bytes: usize) -> Blob {
    Blob {
        id,
        data: vec![42; bytes],
    }
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("nested_calls_reenter", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new().await;
                    let request = blob(41, 256 * 1024);
                    let result = tokio::time::timeout(
                        Duration::from_secs(5),
                        fixture.runtime.operations.call::<Nested>(request.clone()),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(*result, request);
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(count(fixture.path(), "echo-prepared-1"), 1);
                    assert_eq!(count(fixture.path(), "echo-dropped-1"), 1);
                    assert_eq!(count(fixture.path(), "echo-finalized-1"), 1);
                    assert_eq!(count(fixture.path(), "nested-finalized-41"), 1);
                    let rows = fixture
                        .runtime
                        .streams
                        .publication("process.receipt", 1)
                        .unwrap();
                    assert_eq!(rows.query(..).await.unwrap().count(), 1);
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_settles_slots", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new().await;
                    fixture.mode.store(1, Ordering::SeqCst);
                    let pending = fixture
                        .runtime
                        .operations
                        .submit::<Nested>(blob(42, 512))
                        .unwrap();
                    marked(fixture.path(), "echo-started-0").await;
                    pending.cancel();
                    assert!(
                        tokio::time::timeout(Duration::from_secs(5), pending)
                            .await
                            .unwrap()
                            .is_err()
                    );
                    marked(fixture.path(), "echo-finalized-0").await;
                    marked(fixture.path(), "nested-finalized-42").await;
                    assert_eq!(count(fixture.path(), "echo-dropped-0"), 1);
                    assert_eq!(count(fixture.path(), "echo-finalized-0"), 1);
                    assert_eq!(count(fixture.path(), "nested-finalized-42"), 1);
                    fixture.mode.store(0, Ordering::SeqCst);
                    assert_eq!(
                        *fixture
                            .runtime
                            .operations
                            .call::<Nested>(blob(43, 512))
                            .await
                            .unwrap(),
                        blob(43, 512)
                    );
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("deadline_settles_nested", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new().await;
                    fixture.mode.store(1, Ordering::SeqCst);
                    let pending = fixture
                        .runtime
                        .operations
                        .submit::<Nested>(blob(48, 512))
                        .unwrap();
                    marked(fixture.path(), "echo-started-0").await;

                    let result = tokio::time::timeout(
                        contract::POLICY.timeout + Duration::from_secs(5),
                        pending,
                    )
                    .await
                    .expect("the original operation deadline bounds settlement");
                    assert!(matches!(
                        result,
                        Err(harmos::Error::OperationRefused(harmos::Refusal::TimedOut))
                    ));
                    marked(fixture.path(), "echo-finalized-0").await;
                    marked(fixture.path(), "nested-finalized-48").await;
                    assert_eq!(count(fixture.path(), "echo-prepared-0"), 1);
                    assert_eq!(count(fixture.path(), "echo-dropped-0"), 1);
                    assert_eq!(count(fixture.path(), "echo-finalized-0"), 1);
                    assert_eq!(count(fixture.path(), "nested-initialized"), 1);
                    assert_eq!(count(fixture.path(), "nested-finalized-48"), 1);
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
                    assert_eq!(count(fixture.path(), "started"), 1);

                    fixture.mode.store(0, Ordering::SeqCst);
                    assert_eq!(
                        *fixture
                            .runtime
                            .operations
                            .call::<Nested>(blob(49, 512))
                            .await
                            .unwrap(),
                        blob(49, 512)
                    );
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("oversized_request_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new().await;
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .call::<Probe>(blob(44, harmos_sidecar::Frame::LIMIT + 1))
                            .await
                            .is_err()
                    );
                    let prepared = fixture
                        .runtime
                        .operations
                        .call::<Probe>(blob(u64::MAX, 0))
                        .await
                        .unwrap();
                    assert_eq!(
                        prepared.id, 1,
                        "only this diagnostic query reached remote construction"
                    );
                    let request = blob(45, 256 * 1024);
                    assert_eq!(
                        *fixture
                            .runtime
                            .operations
                            .call::<Probe>(request.clone())
                            .await
                            .unwrap(),
                        request
                    );
                    fixture.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("disconnect_no_replay", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new().await;
                    fixture.mode.store(2, Ordering::SeqCst);
                    assert!(
                        tokio::time::timeout(
                            Duration::from_secs(5),
                            fixture.runtime.operations.call::<Nested>(blob(46, 512))
                        )
                        .await
                        .unwrap()
                        .is_err()
                    );
                    service_status(fixture.runtime.services.subscribe(), "process", |status| {
                        matches!(status, ServiceStatus::Inactive { .. })
                    })
                    .await;
                    assert_eq!(count(fixture.path(), "started"), 1);
                    assert_eq!(
                        count(fixture.path(), &format!("echo-started-{}", u64::MAX)),
                        1
                    );
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
                    assert!(
                        tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
                            .await
                            .unwrap()
                            .is_err()
                    );
                });
        });
    }
    {
        scenario_runtime::check("shutdown_sdk_publication", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = tempfile::tempdir().unwrap();
                    std::fs::write(root.path().join("shutdown-publication"), []).unwrap();
                    let source =
                        Executable::at(prepared_fixtures::artifact("harmos-process-fixture"))
                            .unwrap();
                    let mut definition = process_definition(&source);
                    definition.configuration =
                        serde_json::to_vec(root.path().to_str().unwrap()).unwrap();
                    let delivered = Arc::new(AtomicUsize::new(0));
                    let expected = Arc::new(std::sync::Mutex::new(None));
                    let notices = Arc::clone(&delivered);
                    let generation = Arc::clone(&expected);
                    let resident =
                        Supervised::new(source, move |_: &Service<Bank>, notice: Notice| {
                            if let Some(publication) = notice.published() {
                                assert_eq!(publication.owner(), "process");
                                assert_eq!(publication.stream(), "process.receipt");
                                assert_eq!(publication.schema(), 1);
                                assert_eq!(publication.generation(), *generation.lock().unwrap());
                                assert_eq!(publication.wire_sequence(), 1);
                                assert_eq!(publication.sequence(), 1);
                                assert_eq!(publication.missed(), 0);
                                assert_eq!(
                                    <Blob as harmos::Message>::decode(publication.payload())
                                        .unwrap(),
                                    Blob {
                                        id: 47,
                                        data: b"shutdown".to_vec()
                                    }
                                );
                                notices.fetch_add(1, Ordering::SeqCst);
                            }
                        });
                    let calls = resident.clone();
                    let runtime = assembly()
                        .operation_settings(harmos::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<harmos::plugin::sidecar::Publish<_, _, _>>(
                            "shutdown-core",
                            HostLanes::Open,
                            Arc::new(resident.clone()),
                        )
                        .unwrap();
                    runtime
                        .services
                        .register(
                            harmos::services::ServiceDefinition {
                                id: "process".into(),
                                policy: ServicePolicy {
                                    credit: 0,
                                    ..ServicePolicy::default()
                                },
                            },
                            move |_: Arc<()>| {
                                harmos::plugin::sidecar::Managed::new_executable_catalog::<Operations>(
                                    resident.clone(),
                                    definition.clone(),
                                    HostLanes::Open,
                                    NonZeroUsize::new(CAPACITY).unwrap(),
                                    NonZeroUsize::new(4096).unwrap(),
                                )
                                .unwrap()
                            },
                        )
                        .unwrap();
                    let _lease = runtime
                        .services
                        .activate("process", &runtime.services.root_scope())
                        .await
                        .unwrap();
                    service_status(runtime.services.subscribe(), "process", |status| {
                        matches!(status, ServiceStatus::Running { .. })
                    })
                    .await;
                    let native = calls.operations().unwrap();
                    let scope = native.scope().unwrap();
                    *expected.lock().unwrap() = Some(scope.generation().get());
                    let background = scope.publication("process.receipt", 1).unwrap();
                    assert_eq!(delivered.load(Ordering::SeqCst), 0);

                    tokio::time::timeout(Duration::from_secs(5), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(delivered.load(Ordering::SeqCst), 1);
                    assert_eq!(count(root.path(), "shutdown-publication-acknowledged"), 1);
                    assert_eq!(count(root.path(), "shutdown"), 1);
                    assert_eq!(count(root.path(), "cleaned"), 1);
                    assert!(scope.publication("process.receipt", 1).is_err());
                    assert!(
                        background
                            .publish(
                                harmos::Message::encode_to_vec(&Blob {
                                    id: 48,
                                    data: b"late".to_vec(),
                                }),
                                0
                            )
                            .is_err()
                    );
                    assert_eq!(delivered.load(Ordering::SeqCst), 1);
                });
        });
    }
}

/* === Tests === */
