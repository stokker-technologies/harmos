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

use crate::operation_component_contract as contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::OnceLock;
// Third Party Imports

// Local Imports
use super::publications::{Lifecycle, Manager};
use super::{load, runtime};
use contract::{Counts, Read, Value};
use harmos::operations::{ComponentError, ComponentOperation, Failure, OperationDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::catalog]
enum Operations {
    Read(Read),
    Counts(Counts),
    Upload(contract::Upload),
    Download(contract::Download),
    Exchange(contract::Exchange),
}

#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Serial,
}

/* === Traits === */

/* === Implementations === */
/* === Public Functions === */

/* === Private Functions === */
fn component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| {
            load(
                "operation-component",
                if cfg!(feature = "grpc") {
                    "componentize,kit,grpc,streaming"
                } else {
                    "componentize,kit,streaming"
                },
                None,
            )
        })
        .clone()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("runtime_guest_shutdown_retains_budgeted_cleanup", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    use std::time::Duration;
                    let runtime = runtime().await;
                    let host = harmos::guest::Host::new(&runtime).unwrap();
                    let loaded = host.load(&component()).unwrap();
                    let settings = harmos::config::PluginSettings {
                        configuration: serde_json::to_vec("shutdown-pending").unwrap(),
                        shutdown_timeout: Duration::from_millis(20),
                        ..Default::default()
                    };
                    let wrong_catalog = runtime
                        .plugin
                        .register_guest::<()>(
                            host.load(&component()).unwrap(),
                            settings.clone(),
                            super::GuestLanes::Open,
                        )
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        wrong_catalog,
                        harmos::Error::InvalidInput {
                            field: "guest operation catalog",
                            ..
                        }
                    ));
                    assert!(runtime.plugin.status("operation-guest").is_none());
                    runtime
                        .plugin
                        .register_guest::<Operations>(loaded, settings, super::GuestLanes::Open)
                        .await
                        .unwrap();
                    runtime.plugin.initialize().await.unwrap();
                    let ready = runtime.plugin.status("operation-guest").unwrap();
                    assert_eq!(ready.attempts, 1);
                    let stopped = tokio::time::timeout(
                        Duration::from_millis(300),
                        runtime.plugin.stop("operation-guest"),
                    )
                    .await;
                    let error = stopped.unwrap().unwrap_err();
                    assert!(format!("{error:?}").contains("guest plugin shutdown timeout"));
                    let failed = runtime.plugin.status("operation-guest").unwrap();
                    assert_eq!(failed.state, harmos::plugin::PluginState::Failed);
                    assert_eq!(failed.generation, ready.generation);
                    assert_eq!(failed.attempts, 1);
                    assert!(failed.next_retry.is_none());
                    assert!(failed.cleanup_pending);
                    assert!(
                        runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "fenced".into()
                            })
                            .await
                            .is_err()
                    );
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            if runtime.plugin.stop("operation-guest").await.is_ok() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(
                        runtime.plugin.status("operation-guest").unwrap().attempts,
                        1
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }

    {
        scenario_runtime::check("guest_child_overlay_preserves_parent", || {
            let mut executor = tokio::runtime::Builder::new_current_thread();
            executor.enable_all().build().unwrap().block_on(async {
                let runtime = runtime().await;
                let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                    &runtime,
                    component(),
                    super::GuestLanes::Open,
                );
                let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
                manager.initialize(&mut lifecycle).await.unwrap();
                let mut context = harmos::operations::CallContext::default();
                context
                    .insert_portable(contract::Stage {
                        id: "parent".into(),
                    })
                    .unwrap();
                let caller = runtime.operations.with_context(context.clone());
                let answer = caller
                    .call_as::<Read>(Value {
                        value: "context-parent".into(),
                    })
                    .await
                    .unwrap();
                assert_eq!(answer.value, "parent:child");
                assert_eq!(
                    context.value::<contract::Stage>().unwrap().unwrap().id,
                    "parent"
                );
                manager.shutdown(&mut lifecycle).await.unwrap();
                drop((manager, lifecycle));
                runtime.stop().await.unwrap();
            });
        });
    }
    {
        scenario_runtime::check("phases_retain_capacity", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        super::GuestLanes::Open,
                    );
                    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "value".into()
                            })
                            .await
                            .unwrap()
                            .value
                            .as_str(),
                        "value:2"
                    );

                    // Retain Both Original Receipts While Their Physical Cleanup Owns The Table
                    let mut first = runtime
                        .operations
                        .submit_as::<Read>(Value {
                            value: "cleanup".into(),
                        })
                        .unwrap();
                    let first_error = (&mut first).await.unwrap_err();
                    let mut second = runtime
                        .operations
                        .submit_as::<Read>(Value {
                            value: "cleanup".into(),
                        })
                        .unwrap();
                    let second_error = (&mut second).await.unwrap_err();
                    assert_ne!(first.id(), second.id());
                    for error in [first_error, second_error] {
                        let harmos::Error::OperationFinalization {
                            operation,
                            outcome,
                            source,
                        } = error
                        else {
                            panic!("failed cleanup must retain the original operation owner")
                        };
                        assert_eq!(operation, Read::ID);
                        assert!(
                            outcome.is_none(),
                            "the original retry completed before physical cleanup failed"
                        );
                        let harmos::Error::OperationFailed { source, .. } = source.as_ref() else {
                            panic!("the original finalization handler error is preserved")
                        };
                        assert!(matches!(source.downcast_ref::<ComponentError>(),
            Some(ComponentError::Remote { failure: Failure::Transient, message })
                if message.contains("first cleanup retained")));
                    }
                    let error = runtime
                        .operations
                        .call_as::<Read>(Value {
                            value: "later".into(),
                        })
                        .await
                        .unwrap_err();
                    let harmos::Error::OperationFailed { source, .. } = error else {
                        panic!("both failed cleanup owners must still charge the original capacity")
                    };
                    assert!(matches!(source.downcast_ref::<ComponentError>(),
        Some(ComponentError::Preparation { message, .. })
            if message == "carrier invocation capacity is exhausted"));

                    // The Reserved SDK Slot Settles Those Same Ordinary States Before Retirement
                    let original = lifecycle.component();
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    runtime
                        .operations
                        .register_operation_id::<ComponentOperation<Read>>(
                            "retirement-probe",
                            "retired.read",
                            super::GuestLanes::Open,
                            original.clone(),
                        )
                        .unwrap();
                    let retired = runtime
                        .operations
                        .submit_at::<Read>(
                            "retired.read",
                            Value {
                                value: "later".into(),
                            },
                        )
                        .unwrap()
                        .await
                        .unwrap_err();
                    let harmos::Error::OperationFailed { source, .. } = retired else {
                        panic!("the retained generation must refuse execution")
                    };
                    assert!(matches!(
                        source.downcast_ref::<ComponentError>(),
                        Some(ComponentError::Unavailable)
                    ));
                    runtime.operations.unregister("retired.read").await.unwrap();
                    manager
                        .reload(lifecycle.candidate("normal").await, &mut lifecycle)
                        .await
                        .unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "later".into()
                            })
                            .await
                            .unwrap()
                            .value
                            .as_str(),
                        "later:2"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((first, second, original, manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("exact_live_bindings", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        super::GuestLanes::Open,
                    );
                    let mut manager = Manager::new(lifecycle.candidate("mismatch").await).unwrap();
                    let error = manager.initialize(&mut lifecycle).await.unwrap_err();
                    let mut source = Some(&error as &dyn std::error::Error);
                    let mut refused_catalog = false;
                    while let Some(error) = source {
                        if let Some(ComponentError::Remote { message, .. }) =
                            error.downcast_ref::<ComponentError>()
                        {
                            refused_catalog |= message
                .contains("carrier handler definitions differ from the selected operation catalog");
                        }
                        source = if let Some(harmos::Error::OperationFailed { source, .. }) =
                            error.downcast_ref::<harmos::Error>()
                        {
                            Some(source.as_ref())
                        } else {
                            error.source()
                        };
                    }
                    assert!(
                        refused_catalog,
                        "SDK initialization must refuse the actual missing binding: {error}"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("children_retire_authority", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for serial in [false, true] {
                        let runtime = harmos::Runtime::<crate::support::Bank>::builder(
                            crate::support::Ledger::default(),
                            (),
                        )
                        .operation_settings(harmos::operations::Settings {
                            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                        let lane = if serial { Lanes::Serial } else { Lanes::Open };
                        let mut lifecycle =
                            Lifecycle::<Operations, _>::for_component(&runtime, component(), lane);
                        let mut manager =
                            Manager::new(lifecycle.candidate("children").await).unwrap();
                        manager.initialize(&mut lifecycle).await.unwrap();
                        let result = runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "parent".into(),
                            })
                            .await;
                        if serial {
                            assert!(matches!(result, Err(harmos::Error::OperationFailed { .. })));
                        } else {
                            assert_eq!(result.unwrap().value.as_str(), "parent:2:stale:leaf:1");
                        }
                        manager.shutdown(&mut lifecycle).await.unwrap();
                        drop((manager, lifecycle));
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("dropped_store_refuses", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = harmos::Runtime::<crate::support::Bank>::builder(
                        crate::support::Ledger::default(),
                        (),
                    )
                    .operation_settings(harmos::operations::Settings {
                        preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                        ..Default::default()
                    })
                    .register(Lanes::catalog())
                    .runtime()
                    .await
                    .unwrap();
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        Lanes::Open,
                    );
                    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let handle = lifecycle.component();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "value".into()
                            })
                            .await
                            .unwrap()
                            .value
                            .as_str(),
                        "value:2"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    runtime
                        .operations
                        .register_operation_id::<ComponentOperation<Read>>(
                            "retirement-probe",
                            "retired.read",
                            Lanes::Open,
                            handle.clone(),
                        )
                        .unwrap();
                    let retired = runtime
                        .operations
                        .submit_at::<Read>(
                            "retired.read",
                            Value {
                                value: "later".into(),
                            },
                        )
                        .unwrap()
                        .await
                        .unwrap_err();
                    let harmos::Error::OperationFailed { source, .. } = retired else {
                        panic!("the retained generation must refuse execution")
                    };
                    assert!(matches!(
                        source.downcast_ref::<ComponentError>(),
                        Some(ComponentError::Unavailable)
                    ));
                    runtime.operations.unregister("retired.read").await.unwrap();
                    assert!(
                        runtime
                            .operations
                            .call_as::<Read>(Value {
                                value: "later".into()
                            })
                            .await
                            .is_err()
                    );
                    drop((handle, manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("initialization_cleans_failure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    let mut lifecycle =
        Lifecycle::<Operations, _>::for_component(&runtime, component(), super::GuestLanes::Open);
    let mut manager = Manager::new(lifecycle.candidate("initialize-fail").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let mut invocation = runtime
        .operations
        .submit_as::<Read>(Value {
            value: "initialize-fail".into(),
        })
        .unwrap();
    let error = (&mut invocation).await.unwrap_err();
    let harmos::Error::OperationFailed { source, .. } = error else {
        panic!("original initialization error")
    };
    assert!(
        matches!(source.downcast_ref::<ComponentError>(), Some(ComponentError::Preparation { failure: Failure::Transient, message }) if message.contains("partial initialization failed"))
    );
    assert_eq!(
        invocation.status().phase(),
        harmos::operations::OperationPhase::Settled
    );
    let counts = runtime
        .operations
        .call_as::<Counts>(Value {
            value: "counts".into(),
        })
        .await
        .unwrap();
    // A failed initialization owns its partial acquisitions: no stop or cleanup.
    assert_eq!(counts.value, "1:0:0:0");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("initialization_children_retired", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        super::GuestLanes::Open,
                    );
                    let mut manager =
                        Manager::new(lifecycle.candidate("initialize-child").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let result = runtime
                        .operations
                        .call_as::<Read>(Value {
                            value: "initialize-child".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(result.value, "initialize-child:2");
                    let counts = runtime
                        .operations
                        .call_as::<Counts>(Value {
                            value: "counts".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(counts.value, "2:3:2:2");
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("grpc_original_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::{num::NonZeroUsize, time::Duration};
                    let runtime = runtime().await;
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        super::GuestLanes::Open,
                    );
                    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
                    assert!(runtime.grpc().methods().is_empty());
                    manager.initialize(&mut lifecycle).await.unwrap();
                    assert_eq!(runtime.grpc().methods().len(), 4);
                    let settings = harmos::grpc::Settings {
                        address: "127.0.0.1:0".parse().unwrap(),
                        operations: harmos::grpc::server::Settings::default(),
                        reflection: true,
                        web: false,
                        allowed_origins: Vec::new(),
                        tls: None,
                        connections: NonZeroUsize::new(4).unwrap(),
                        calls_per_connection: NonZeroUsize::new(8).unwrap(),
                        request_timeout: Duration::from_secs(2),
                        shutdown_timeout: Duration::from_secs(2),
                    };
                    let attached = runtime.grpc().attach(settings).await.unwrap();
                    let address = attached.local_addr();
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    let mut client = tonic::client::Grpc::new(channel);
                    client.ready().await.unwrap();
                    let settings = harmos::grpc::server::Settings::default();
                    let codec =
                        harmos::grpc::Codec::<Value, harmos::operations::Response<Value>>::new(
                            settings.decode,
                            settings.encode,
                        )
                        .unwrap();
                    let response = client
                        .unary(
                            tonic::Request::new(harmos::operations::Response::new(Value {
                                value: "value".into(),
                            })),
                            tonic::codegen::http::uri::PathAndQuery::from_static(
                                "/fixture.v1.Guest/Read",
                            ),
                            codec,
                        )
                        .await
                        .unwrap()
                        .into_inner();
                    assert_eq!(response.value, "value:2");
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert!(runtime.grpc().methods().is_empty());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("stream_lifecycle_preserved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = Lifecycle::<Operations, _>::for_component(
                        &runtime,
                        component(),
                        super::GuestLanes::Open,
                    );
                    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let settings = harmos::operations::StreamSettings {
                        items: std::num::NonZeroUsize::MIN,
                        bytes: std::num::NonZeroUsize::new(64).unwrap(),
                    };
                    let mut upload = runtime
                        .operations
                        .upload::<contract::Upload>(5, settings)
                        .unwrap();
                    upload.send(0).await.unwrap();
                    if let Err(error) = upload.send(7).await {
                        upload.finish_input();
                        panic!(
                            "input failed: {error:?}; settlement: {:?}",
                            upload.join().await
                        );
                    }
                    upload.finish_input();
                    assert_eq!(*upload.join().await.unwrap(), 12);

                    let mut download = runtime
                        .operations
                        .download::<contract::Download>(5, settings)
                        .unwrap();
                    assert_eq!(*download.next().await.unwrap().unwrap(), 0);
                    assert_eq!(*download.next().await.unwrap().unwrap(), 5);
                    assert!(download.next().await.unwrap().is_none());
                    assert_eq!(*download.join().await.unwrap(), 5);

                    let mut exchange = runtime
                        .operations
                        .exchange::<contract::Exchange>(5, settings, settings)
                        .unwrap();
                    exchange.send(0).await.unwrap();
                    assert_eq!(*exchange.next().await.unwrap().unwrap(), 5);
                    exchange.send(7).await.unwrap();
                    assert_eq!(*exchange.next().await.unwrap().unwrap(), 12);
                    exchange.finish_input();
                    assert!(exchange.next().await.unwrap().is_none());
                    assert_eq!(*exchange.join().await.unwrap(), 12);
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Counts>(Value {
                                value: String::new()
                            })
                            .await
                            .unwrap()
                            .value,
                        "3:3:3:3"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
