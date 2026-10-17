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

#[path = "../support/process_graph_contract.rs"]
pub(super) mod contract;
use crate::support::fixtures as prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{HostLanes, assembly, process_definition, service_status};
use crate::support::Bank;
use contract::{Marks, Probe, Step};
use harmos::operations::OperationDeclaration;
use harmos::sidecar::{Executable, Notice, Supervised};
use harmos::{Message, services::Service, services::ServiceStatus};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) async fn marked(root: &Path, name: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !std::fs::read(root.join(name)).is_ok_and(|bytes| bytes.ends_with(b"\n")) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

fn count(root: &Path, name: &str) -> usize {
    std::fs::read_to_string(root.join(name))
        .unwrap()
        .lines()
        .count()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("graph_shutdown_provider", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = tempfile::tempdir().unwrap();
                    let provider_root = root.path().join("provider");
                    let client_root = root.path().join("client");
                    std::fs::create_dir_all(&client_root).unwrap();
                    let client_marks = Marks {
                        root: client_root.clone(),
                    };
                    client_marks.mark("hold-initialization").unwrap();
                    let provider = Executable::at(prepared_fixtures::artifact(
                        "harmos-process-provider-fixture",
                    ))
                    .unwrap();
                    let client = Executable::at(prepared_fixtures::artifact(
                        "harmos-process-client-fixture",
                    ))
                    .unwrap();
                    let mut provider_definition = process_definition(&provider);
                    provider_definition.configuration =
                        serde_json::to_vec(&provider_root.to_str().unwrap()).unwrap();
                    let mut client_definition = process_definition(&client);
                    client_definition.configuration =
                        serde_json::to_vec(&client_root.to_str().unwrap()).unwrap();
                    let definitions = harmos::plugin::PluginDefinitions::new(
                        [client_definition, provider_definition],
                        [],
                    )
                    .unwrap();
                    // Reserve The SDK Encoder's Actual Peak Without Raising The Original Pool.
                    let encoding_bytes = definitions
                        .definitions()
                        .map(|definition| definition.configuration.len() * 2)
                        .max()
                        .unwrap()
                        + std::mem::size_of::<harmos::operations::lifecycle::Initialize>()
                        + 84;
                    assert!(
                        encoding_bytes <= 512,
                        "fixture configuration exceeds its SDK allowance"
                    );
                    let encoding_bytes = NonZeroUsize::new(encoding_bytes).unwrap();
                    let residents = [client, provider].map(|source| {
                        Supervised::new(source, notices as fn(&Service<Bank>, Notice))
                            .grace(Duration::from_millis(50))
                    });
                    let runtime = assembly()
                        .operation_settings(harmos::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_service(
                            harmos::services::ServiceDefinition {
                                id: "process-graph".into(),
                                policy: harmos::operations::OperationPolicy {
                                    restart: harmos::operations::RestartPolicy::Never,
                                    ..harmos::services::POLICY
                                },
                            },
                            move |_: Arc<()>| {
                                harmos::sidecar::Managed::new_executable_graph(
                                    residents.clone(),
                                    definitions.clone(),
                                    HostLanes::Open,
                                    NonZeroUsize::new(2).unwrap(),
                                    encoding_bytes,
                                )
                                .unwrap()
                            },
                        )
                        .unwrap();
                    let lease = runtime
                        .operations
                        .submit_resident("process-graph")
                        .await
                        .unwrap();

                    // The Original Lifecycle Loan Calls Its Provider Before Ordinary Admission.
                    marked(&client_root, "dependency-initialized").await;
                    assert_eq!(count(&provider_root, "initialized"), 1);
                    assert_eq!(count(&provider_root, "read-finalized-1"), 1);
                    let status = service_status(
                        runtime.operations.watch_residents(),
                        "process-graph",
                        |_| true,
                    )
                    .await;
                    assert_eq!(status, ServiceStatus::Starting);
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_encoded(Probe::ID, harmos::Message::encode_to_vec(&Probe { phase: 3 })),
                        Err(harmos::Error::NotReady {
                            domain: "operation plugin"
                        }),
                    ));
                    assert!(!client_root.join("probe-initialized").exists());
                    client_marks.mark("release-initialization").unwrap();
                    service_status(
                        runtime.operations.watch_residents(),
                        "process-graph",
                        |status| matches!(status, ServiceStatus::Running { .. }),
                    )
                    .await;
                    let response = runtime
                        .operations
                        .call_encoded(Probe::ID, harmos::Message::encode_to_vec(&Probe { phase: 3 }))
                        .await
                        .unwrap();
                    assert_eq!(Step::decode(&response).unwrap().phase, 3);
                    assert_eq!(count(&client_root, "probe-initialized"), 1);
                    assert_eq!(count(&client_root, "probe-finalized"), 1);

                    // Reverse Retirement Preserves The Actual Provider Through Child Finalization.
                    tokio::time::timeout(Duration::from_secs(5), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(count(&client_root, "dependency-shutdown"), 1);
                    assert_eq!(count(&provider_root, "read-prepared-2"), 1);
                    assert_eq!(count(&provider_root, "read-started-2"), 1);
                    assert_eq!(count(&provider_root, "read-finalized-2"), 1);
                    for path in [&provider_root, &client_root] {
                        assert_eq!(count(path, "shutdown"), 1);
                        assert_eq!(count(path, "cleaned"), 1);
                    }
                    drop(lease);
                });
        });
    }
}

/* === Tests === */
