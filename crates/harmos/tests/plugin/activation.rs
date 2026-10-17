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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct WireClient;
struct WireCall;

/* === Enums === */
#[harmos::catalog]
enum WireClients {
    Run(WireClient),
}

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WireCall {
    type Request = RunRequest;
    type Response = String;
    const ID: &'static str = "client.run";
    const POLICY: OperationPolicy = Run::<1, 100, false>::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl harmos::operations::WireOperation for WireCall {
    const VERSION: u32 = 1;
    #[cfg(feature = "grpc")]
    const GRPC: Option<harmos::operations::Grpc> = Some(harmos::operations::Grpc::new(
        "fixture.embedded.v1",
        "Client",
        "Run",
    ));
    #[cfg(feature = "grpc")]
    fn visit_grpc<V: harmos::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl Operation for WireClient {
    type Declaration = WireCall;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut RunRequest,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let response = context
                .call_as::<Run<0, 100, false>>(RunRequest {
                    value: request.value.clone(),
                })
                .await?;
            Ok(response.as_ref().clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn contract_scenario() {
    scenario_runtime::check("runtime_embedded_wire_contracts", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                use harmos::operations::{AuthoredContracts, Contract};
                use harmos_domain::inventory::contracts::Required;
                let runtime = harmos::Runtime::<Fixture>::builder((), ())
                    .operation_settings(harmos::operations::Settings {
                        preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                        ..Default::default()
                    })
                    .register(Lanes::catalog())
                    .runtime()
                    .await
                    .unwrap();
                let mut provider = plan()
                    .ordered()
                    .iter()
                    .find(|definition| definition.id == "provider")
                    .unwrap()
                    .clone();
                provider.contributions = PluginContributions::default();
                runtime
                    .plugin
                    .register_embedded_wire::<Operations<0, 100, false>, _>(
                        provider,
                        Default::default(),
                        Lanes::Open,
                        Arc::new(Owner),
                    )
                    .await
                    .unwrap();
                let registered = runtime.plugin.status("provider").unwrap();
                assert_eq!(registered.attempts, 0);
                let contracts = AuthoredContracts::of::<Operations<0, 100, false>>().unwrap();
                assert_eq!(
                    registered
                        .definition
                        .as_ref()
                        .unwrap()
                        .contributions
                        .wire_contracts,
                    contracts
                );
                assert!(
                    runtime
                        .operations
                        .call_encoded("provider.run", request_payload())
                        .await
                        .is_err()
                );

                let mut client = plan()
                    .ordered()
                    .iter()
                    .find(|definition| definition.id == "client")
                    .unwrap()
                    .clone();
                client.artifact = PluginArtifact::Embedded {
                    revision: "wire-client".into(),
                };
                client.contributions.required_contracts = Some(vec![Required {
                    contract: contracts[0].clone(),
                    grants: vec![],
                }]);
                runtime
                    .plugin
                    .register_embedded_wire::<WireClients, _>(
                        client,
                        Default::default(),
                        Lanes::Open,
                        Arc::new(()),
                    )
                    .await
                    .unwrap();
                runtime.plugin.initialize().await.unwrap();
                let ready = runtime.plugin.status("provider").unwrap();
                #[cfg(feature = "grpc")]
                let grpc_generation = runtime.grpc().methods()[0].generation;

                #[cfg(feature = "grpc")]
                {
                    let methods = runtime.grpc().methods();
                    assert_eq!(methods.len(), 1);
                    assert_eq!(methods[0].owner.as_str(), "client");
                    assert_eq!(
                        methods[0].contract.grpc().unwrap().path(),
                        "/fixture.embedded.v1.Client/Run"
                    );
                    let attached = runtime.grpc().attach(Default::default()).await.unwrap();
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    let mut client = tonic::client::Grpc::new(channel);
                    client.ready().await.unwrap();
                    let settings = harmos::grpc::server::Settings::default();
                    let codec = harmos::grpc::Codec::<
                        String,
                        harmos::operations::Response<RunRequest>,
                    >::new(settings.decode, settings.encode)
                    .unwrap();
                    let response: tonic::Response<harmos::operations::StreamItem<String>> = client
                        .unary(
                            tonic::Request::new(harmos::operations::Response::new(RunRequest {
                                value: "grpc embedded provider".into(),
                            })),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.embedded.v1.Client/Run",
                            ),
                            codec,
                        )
                        .await
                        .unwrap();
                    assert_eq!(response.into_inner().as_str(), "grpc embedded provider");
                }

                let request = harmos::Message::encode_to_vec(&RunRequest {
                    value: "original provider".into(),
                });
                let response = runtime
                    .operations
                    .call_encoded("client.run", request)
                    .await
                    .unwrap();
                assert_eq!(
                    <String as harmos::Message>::decode(&response).unwrap(),
                    "original provider"
                );

                // Wire Version Drift Is Rejected Without Replacing The Live Generation
                let mut replacement = ready.definition.clone().unwrap();
                let original = &contracts[0];
                replacement.contributions.wire_contracts = vec![
                    Contract::new(
                        original.operation().clone(),
                        2,
                        original.codec(),
                        original.schema().clone(),
                    )
                    .unwrap(),
                ];
                assert!(matches!(
                    runtime
                        .plugin
                        .reload_embedded_wire::<Operations<0, 100, false>, _>(
                            replacement,
                            Default::default(),
                            Lanes::Open,
                            Arc::new(Owner)
                        )
                        .await,
                    Err(Error::InvalidInput {
                        field: "embedded plugin contracts",
                        ..
                    })
                ));
                assert_eq!(runtime.plugin.status("provider").unwrap(), ready);

                let mut replacement = ready.definition.clone().unwrap();
                replacement.artifact = PluginArtifact::Embedded {
                    revision: "wire-provider-v2".into(),
                };
                runtime
                    .plugin
                    .reload_embedded_wire::<Operations<0, 100, false>, _>(
                        replacement,
                        Default::default(),
                        Lanes::Open,
                        Arc::new(Owner),
                    )
                    .await
                    .unwrap();
                let reloaded = runtime.plugin.status("provider").unwrap();
                assert_ne!(reloaded.generation, ready.generation);
                assert_eq!(reloaded.attempts, 2);
                #[cfg(feature = "grpc")]
                assert_ne!(runtime.grpc().methods()[0].generation, grpc_generation);

                assert_eq!(runtime.plugin.status("client").unwrap().attempts, 2);
                let response = runtime
                    .operations
                    .call_encoded("client.run", request_payload())
                    .await
                    .unwrap();
                assert_eq!(<String as harmos::Message>::decode(&response).unwrap(), "");
                runtime.stop().await.unwrap();
                #[cfg(feature = "grpc")]
                assert!(runtime.grpc().methods().is_empty());

                assert!(
                    runtime
                        .operations
                        .call_encoded("client.run", request_payload())
                        .await
                        .is_err()
                );
                assert!(matches!(
                    runtime.plugin.cleanup().status(),
                    PluginCleanupStatus::Settled
                ));
            });
    });
    scenario_runtime::check("runtime_owned_embedded_plugins", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let runtime = harmos::Runtime::<Fixture>::builder((), ())
                .register(Lanes::catalog())
                .runtime()
                .await
                .unwrap();
            let handle = runtime.handle();
            assert!(runtime.plugin.same_instance(&handle.plugin));
            assert!(handle.plugin.same_instance(&handle.clone().plugin));
            let mut definition = plan()
                .ordered()
                .iter()
                .find(|definition| definition.id == "provider")
                .unwrap()
                .clone();
            definition.artifact = PluginArtifact::Embedded {
                revision: "runtime-owned".into(),
            };
            definition.contributions = PluginContributions::default();
            handle
                .plugin
                .register_embedded::<Operations<0, 100, false>, _>(
                    definition,
                    Default::default(),
                    Lanes::Open,
                    Arc::new(Owner),
                )
                .await
                .unwrap();
            let registered = runtime.plugin.status("provider").unwrap();
            assert_eq!(registered.state, PluginState::Registered);
            assert_eq!(registered.attempts, 0);
            assert!(matches!(
                runtime
                    .operations
                    .call_as::<Run<0, 100, false>>(RunRequest {
                        value: "before".into()
                    })
                    .await,
                Err(Error::NotRegistered { .. })
            ));
            runtime.plugin.initialize().await.unwrap();
            handle.plugin.initialize().await.unwrap();
            let ready = handle.plugin.status("provider").unwrap();
            assert_eq!(ready.state, PluginState::Ready);
            assert_eq!(ready.attempts, 1);
            assert_eq!(
                runtime
                    .operations
                    .call_as::<Run<0, 100, false>>(RunRequest {
                        value: "shared runtime owner".into()
                    })
                    .await
                    .unwrap()
                    .as_str(),
                "shared runtime owner"
            );
            let generation = runtime
                .plugin
                .status("provider")
                .unwrap()
                .generation
                .unwrap();
            runtime.plugin.reload("provider").await.unwrap();
            let reloaded = runtime.plugin.status("provider").unwrap();
            assert_ne!(reloaded.generation, Some(generation));
            assert_eq!(reloaded.attempts, 2);
            assert_eq!(
                runtime
                    .operations
                    .call_as::<Run<0, 100, false>>(RunRequest {
                        value: "after reload".into()
                    })
                    .await
                    .unwrap()
                    .as_str(),
                "after reload"
            );
            let mut replacement = reloaded.definition.clone().unwrap();
            replacement.artifact = PluginArtifact::Embedded {
                revision: "runtime-owned-v2".into(),
            };
            handle
                .plugin
                .reload_embedded::<Operations<0, 100, false>, _>(
                    replacement.clone(),
                    Default::default(),
                    Lanes::Open,
                    Arc::new(Owner),
                )
                .await
                .unwrap();
            let updated = runtime.plugin.status("provider").unwrap();
            assert_eq!(updated.attempts, 3);
            assert_ne!(updated.generation, reloaded.generation);
            assert_eq!(updated.active_definition, Some(replacement.clone()));
            assert!(
                runtime
                    .plugin
                    .reload_embedded::<Operations<1, 100, false>, _>(
                        replacement,
                        Default::default(),
                        Lanes::Open,
                        Arc::new(Owner),
                    )
                    .await
                    .is_err()
            );
            assert_eq!(runtime.plugin.status("provider").unwrap(), updated);
            assert_eq!(
                runtime
                    .operations
                    .call_as::<Run<0, 100, false>>(RunRequest {
                        value: "after replacement".into()
                    })
                    .await
                    .unwrap()
                    .as_str(),
                "after replacement"
            );
            runtime.plugin.remove("provider").await.unwrap();
            assert!(runtime.plugin.status("provider").is_none());
            assert!(matches!(
                runtime
                    .operations
                    .call_as::<Run<0, 100, false>>(RunRequest {
                        value: "removed".into()
                    })
                    .await,
                Err(Error::NotRegistered { .. })
            ));
            runtime.stop().await.unwrap();
            assert!(handle.plugin.initialize().await.is_err());
            assert!(matches!(
                handle.plugin.cleanup().status(),
                PluginCleanupStatus::Settled
            ));
            assert!(
                handle
                    .plugin
                    .statuses()
                    .iter()
                    .all(|status| status.generation.is_none())
            );
        });
    });
    scenario_runtime::check("inert_registration_preserves_live_graph", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let mut session = session();
            let mut host = Host::new().await;
            session.initialize(&mut host).await.unwrap();
            let provider = session.status("provider").unwrap();
            host.control.lock().unwrap().events.clear();
            let mut added = session.definition("other").unwrap().clone();
            added.id = "registered-later".into();
            added.contributions = PluginContributions::default();
            let definitions: Vec<_> = session
                .definitions()
                .cloned()
                .chain([added.clone()])
                .collect();
            let frozen = session.plan_reload(candidate([])).unwrap();
            session
                .register(PluginDefinitions::new(definitions.clone(), []).unwrap())
                .unwrap();
            assert!(host.control.lock().unwrap().events.is_empty());
            assert_eq!(session.status("provider").unwrap(), provider);
            let inert = session.status(&added.id).unwrap();
            assert_eq!(inert.state, PluginState::Registered);
            assert_eq!(inert.attempts, 0);
            assert!(inert.generation.is_none());
            assert!(
                host.operations()
                    .call_encoded("provider.run", request_payload())
                    .await
                    .is_ok()
            );
            assert!(matches!(
                session
                    .apply_reload(frozen, &mut host)
                    .await
                    .map_err(failure),
                Err(PluginFailure::StalePlan { .. })
            ));
            let before: Vec<_> = session.statuses().collect();
            let mut replaced = definitions.clone();
            replaced
                .iter_mut()
                .find(|definition| definition.id == "provider")
                .unwrap()
                .configuration = b" null ".to_vec();
            assert!(
                session
                    .register(PluginDefinitions::new(replaced, []).unwrap())
                    .is_err()
            );
            assert!(
                session
                    .register(
                        PluginDefinitions::new(
                            definitions
                                .into_iter()
                                .filter(|definition| definition.id != "client"),
                            []
                        )
                        .unwrap()
                    )
                    .is_err()
            );
            assert_eq!(session.statuses().collect::<Vec<_>>(), before);
            assert!(host.control.lock().unwrap().events.is_empty());
            session.shutdown(&mut host).await.unwrap();
        });
    });
    {
        scenario_runtime::check("activation_failure_cleans", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host::new().await;
    session.initialize(&mut host).await.unwrap();
    assert_eq!(session.status("client").unwrap().attempts, 1);
    session.initialize(&mut host).await.unwrap();
    assert_eq!(session.status("client").unwrap().attempts, 1);
    host.control.lock().unwrap().events.clear();
    host.fail_start = Some("client".to_owned());
    let next = plan();
    assert!(
        matches!(session.reload_parts(next.clone(), inventories(&next), ["provider".to_owned()], &mut host).await.map_err(failure),
        Err(PluginFailure::Start { artifact, cleanup, .. }) if artifact == "client" && cleanup.is_empty())
    );
    assert_eq!(session.ready().collect::<Vec<_>>(), ["other"]);
    assert_eq!(
        host.control.lock().unwrap().events,
        [
            "fence:client,provider",
            "stop:client",
            "stop:provider",
            "start:provider",
            "start:client",
            "stop:client",
            "stop:provider"
        ]
    );
    assert!(matches!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    host.fail_start = None;
    session.initialize(&mut host).await.unwrap();
    assert_eq!(session.ready().count(), 3);
    let recovered = session.status("client").unwrap();
    assert_eq!(recovered.attempts, 3);
    assert!(recovered.last_failure.is_some());
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("start_preserves_providers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let mut host = Host {
        fail_start: Some("client".into()),
        ..Host::new().await
    };
    assert!(
        matches!(session.initialize(&mut host).await.map_err(failure),
        Err(PluginFailure::Start { artifact, cleanup, .. }) if artifact == "client" && cleanup.is_empty())
    );
    assert!(session.ready().any(|id| id == "provider"));
    assert!(!session.ready().any(|id| id == "client"));
    assert!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await
            .is_ok()
    );
    assert!(matches!(
        host.operations()
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    assert!(
        !host
            .control
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "stop:provider")
    );

    // The product may remove a failed optional plugin. The next admission
    // starts remaining branches without replaying its healthy provider.
    let next = PluginGraph::resolve(
        plan()
            .ordered()
            .iter()
            .filter(|artifact| artifact.id != "client")
            .cloned(),
    )
    .unwrap();
    session
        .reload_parts(next.clone(), inventories(&next), [], &mut host)
        .await
        .unwrap();
    assert_eq!(
        host.control
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.as_str() == "start:provider")
            .count(),
        1
    );
    assert_eq!(session.ready().collect::<Vec<_>>(), ["other", "provider"]);
    assert!(
        !session
            .router()
            .graph()
            .ordered()
            .iter()
            .any(|definition| definition.id == "client")
    );
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("initialization_retains_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut session = session();
    let (started, mut starting) = tokio::sync::watch::channel(String::new());
    let mut host = Host {
        block: Some("provider".to_owned()),
        started: Some(started),
        ..Host::new().await
    };
    {
        let initialization = session.initialize(&mut host);
        tokio::pin!(initialization);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                outcome = &mut initialization => panic!("initialization completed before cancellation: {outcome:?}"),
                reached = starting.wait_for(|owner| owner == "provider") => { reached.unwrap(); }
            }
        }).await.unwrap();
    }
    let original = session.status("provider").unwrap().generation.unwrap();
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("provider"), None);
        assert_eq!(control.backend_drops.get("provider"), None);
    }
    assert!(
        host.control
            .lock()
            .unwrap()
            .callers
            .contains_key("provider")
    );
    assert!(!session.ready().any(|id| id == "provider"));
    assert!(matches!(
        host.operations()
            .call_encoded("provider.run", request_payload())
            .await,
        Err(Error::NotReady {
            domain: "operation plugin"
        })
    ));
    host.block = None;
    session.initialize(&mut host).await.unwrap();
    assert_ne!(
        session.status("provider").unwrap().generation.unwrap(),
        original
    );
    {
        let control = host.control.lock().unwrap();
        assert_eq!(control.stop_calls.get("provider"), Some(&1));
        assert_eq!(control.backend_drops.get("provider"), Some(&1));
        let transitions: Vec<_> = control
            .events
            .iter()
            .filter(|event| matches!(event.as_str(), "start:provider" | "stop:provider"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            transitions,
            ["start:provider", "stop:provider", "start:provider"]
        );
    }
    session.shutdown(&mut host).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("start_retains_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut manager = session();
    let (started, mut starting) = tokio::sync::watch::channel(String::new());
    let mut host = Host {
        block: Some("client".into()),
        started: Some(started),
        ..Host::new().await
    };
    {
        let start = manager.initialize(&mut host);
        tokio::pin!(start);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                outcome = &mut start => panic!("initialization completed before cancellation: {outcome:?}"),
                reached = starting.wait_for(|owner| owner == "client") => { reached.unwrap(); }
            }
        }).await.unwrap();
    }

    let retained = manager.status("client").unwrap();
    assert_eq!(retained.state, PluginState::Initializing);
    assert!(retained.generation.is_some());
    assert!(retained.active_definition.is_some());
    assert!(host.control.lock().unwrap().callers.contains_key("client"));
    let caller = host.control.lock().unwrap().callers["client"].clone();
    let operations = host.operations().clone();
    assert!(matches!(
        operations
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotReady {
            domain: "operation plugin"
        })
    ));
    assert!(
        host.call(&caller, "provider.run", request_payload())
            .await
            .is_ok()
    );

    // Final Manager Drop Settles Partial Setup Before Invalidating Callers.
    let cleanup = manager.cleanup();
    drop(manager);
    cleanup.join().await.unwrap();
    {
        let control = host.control.lock().unwrap();
        assert!(!control.callers.contains_key("client"));
        for owner in ["client", "provider"] {
            assert_eq!(control.stop_calls.get(owner), Some(&1));
            assert_eq!(control.backend_drops.get(owner), Some(&1));
        }
    }
    assert!(matches!(
        operations
            .call_encoded("client.run", request_payload())
            .await,
        Err(Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));
    assert!(matches!(
        host.call(&caller, "provider.run", request_payload()).await,
        Err(Error::OperationCaller {
            source: DispatchError::Unavailable
        })
    ));

});
        });
    }
    {
        scenario_runtime::check("inventory_refuses_commit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let graph = plan();
                    let candidate = candidate_parts(&graph, inventories(&graph), []).unwrap();
                    let mut definitions: Vec<_> = candidate.definitions().cloned().collect();
                    let client = definitions
                        .iter_mut()
                        .find(|definition| definition.id == "client")
                        .unwrap();
                    client
                        .contributions
                        .streams
                        .push(harmos_domain::StreamDeclaration {
                            id: "client.discovery".into(),
                            schema: 1,
                            window: 4,
                            metadata: harmos::Metadata::new(
                                harmos::Identifier::new("client.discovery").unwrap(),
                                "Discovery",
                                None,
                            ),
                            row_metadata: Vec::new(),
                            row: harmos::schema::graph::Schema {
                                root: 0,
                                definitions: vec![vec![]],
                            },
                        });
                    let mut manager =
                        PluginManager::new(PluginDefinitions::new(definitions, []).unwrap())
                            .unwrap();
                    let mut host = Host::new().await;
                    let PluginFailure::Start {
                        artifact,
                        source,
                        cleanup,
                    } = failure(manager.initialize(&mut host).await.unwrap_err())
                    else {
                        panic!("expected retained initialization failure");
                    };
                    assert_eq!(artifact, "client");
                    assert!(cleanup.is_empty());
                    assert!(matches!(failure(source), PluginFailure::Routing(
        DispatchError::IncompleteStreams { owner }) if owner == "client"));
                    let status = manager.status("client").unwrap();
                    assert_eq!(status.state, PluginState::Failed);
                    assert_eq!(status.attempts, 1);
                    assert!(status.last_failure.is_some());
                    assert!(!status.cleanup_pending);
                    assert!(
                        !manager
                            .status("client")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(
                        host.control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:client")
                    );
                    assert!(manager.caller("provider").is_ok());
                    assert!(
                        !host
                            .operations()
                            .definitions()
                            .iter()
                            .any(|definition| definition.id.as_str() == "client.run")
                    );
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("mismatch_cleans_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut manager = session();
                    let mut host = Host {
                        wrong_backend: Some("client".into()),
                        ..Host::new().await
                    };
                    let error = manager.initialize(&mut host).await.unwrap_err();
                    assert!(matches!(
                        failure(error),
                        PluginFailure::Start {
                            source: Error::InvalidInput {
                                field: "plugin backend",
                                ..
                            },
                            ..
                        }
                    ));
                    assert!(
                        !host
                            .control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "start:client")
                    );
                    assert!(
                        host.control
                            .lock()
                            .unwrap()
                            .events
                            .iter()
                            .any(|event| event == "stop:client")
                    );
                    assert_eq!(manager.status("client").unwrap().state, PluginState::Failed);
                    assert!(
                        !manager
                            .status("client")
                            .is_some_and(|status| status.generation.is_some())
                    );
                    assert!(manager.caller("provider").is_ok());
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("outbound_caller_ready", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let graph = plan();
                    let mut inventory = inventories(&graph);
                    inventory
                        .iter_mut()
                        .find(|(id, _)| id == "client")
                        .unwrap()
                        .1
                        .clear();
                    let mut manager =
                        PluginManager::new(candidate_parts(&graph, inventory, []).unwrap())
                            .unwrap();
                    let mut host = Host::new().await;
                    manager.initialize(&mut host).await.unwrap();
                    let caller = manager.caller("client").unwrap();
                    assert_eq!(
                        Some(caller.generation()),
                        manager.status("client").unwrap().generation
                    );
                    assert!(
                        host.call(&caller, "provider.run", request_payload())
                            .await
                            .is_ok()
                    );
                    assert!(matches!(
                        host.operations()
                            .call_encoded("client.run", request_payload())
                            .await,
                        Err(Error::NotRegistered {
                            domain: "operation",
                            ..
                        })
                    ));
                    manager.shutdown(&mut host).await.unwrap();
                });
        });
    }
}
