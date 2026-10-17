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

// Third Party Imports

// Local Imports
use super::publications::{GuestOperations, Lifecycle, Manager};
use super::sandbox::{Counts, Read, Stage, Value};
use super::schematic::operations::{Operations, analyze, propose};
use super::{corner, route, runtime, sandbox, schematic};
use harmos::operations::{ComponentError, ComponentOperation, FailureKind, OperationDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
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
#[tokio::test]
async fn shutdown_retains_cleanup() {
    use std::time::Duration;
    let runtime = runtime().await;
    super::publications::serve_host(&runtime, super::GuestLanes::Open);
    let host = harmos::guest::Host::new(&runtime).unwrap();
    let loaded = host.load(&sandbox()).unwrap();
    let settings = harmos::config::PluginSettings {
        configuration: serde_json::to_vec("shutdown-pending").unwrap(),
        shutdown_timeout: Duration::from_millis(20),
        host_operations: ["host.publish", "provider.echo"]
            .map(|id| harmos::Identifier::new(id).unwrap())
            .to_vec(),
        ..Default::default()
    };
    let wrong_catalog = runtime
        .plugin
        .register_guest::<()>(
            host.load(&sandbox()).unwrap(),
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
    assert!(runtime.plugin.status("sandbox-guest").is_none());
    runtime
        .plugin
        .register_guest::<GuestOperations>(loaded, settings, super::GuestLanes::Open)
        .await
        .unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status("sandbox-guest").unwrap();
    assert_eq!(ready.attempts, 1);
    let stopped = tokio::time::timeout(
        Duration::from_millis(300),
        runtime.plugin.stop("sandbox-guest"),
    )
    .await;
    let error = stopped.unwrap().unwrap_err();
    assert!(
        format!("{error:?}").contains("guest plugin shutdown timeout"),
        "{error:?}"
    );
    let failed = runtime.plugin.status("sandbox-guest").unwrap();
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
            if runtime.plugin.stop("sandbox-guest").await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(runtime.plugin.status("sandbox-guest").unwrap().attempts, 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn overlay_preserves_parent() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let mut context = harmos::operations::CallContext::default();
    context
        .insert_portable(Stage {
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
    assert_eq!(context.value::<Stage>().unwrap().unwrap().id, "parent");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn phases_retain_capacity() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
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
        Some(ComponentError::Remote(failure))
            if failure.code() == FailureKind::Transient
                && failure.cause().contains("first cleanup retained")));
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
    Some(ComponentError::Preparation(failure))
        if failure.cause() == "carrier invocation capacity is exhausted"));

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
}

#[tokio::test]
async fn exact_live_bindings() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("mismatch").await).unwrap();
    let error = manager.initialize(&mut lifecycle).await.unwrap_err();
    let mut source = Some(&error as &dyn std::error::Error);
    let mut refused_catalog = false;
    while let Some(error) = source {
        if let Some(ComponentError::Remote(failure)) = error.downcast_ref::<ComponentError>() {
            refused_catalog |= failure
                .cause()
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
}

#[tokio::test]
async fn children_retire_authority() {
    for serial in [false, true] {
        let runtime =
            harmos::Runtime::<crate::support::Bank>::builder(crate::support::Ledger::default(), ())
                .operation_settings(harmos::operations::Settings {
                    preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                    ..Default::default()
                })
                .register(Lanes::catalog())
                .runtime()
                .await
                .unwrap();
        let lane = if serial { Lanes::Serial } else { Lanes::Open };
        let mut lifecycle = Lifecycle::sandbox_in(&runtime, lane);
        let mut manager = Manager::new(lifecycle.candidate("children").await).unwrap();
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
}

#[tokio::test]
async fn initialization_cleans_failure() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
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
        matches!(source.downcast_ref::<ComponentError>(), Some(ComponentError::Preparation(failure)) if failure.code() == FailureKind::Transient && failure.cause().contains("partial initialization failed"))
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
}

#[tokio::test]
async fn initialization_children_retired() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("initialize-child").await).unwrap();
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
}

#[tokio::test]
async fn dropped_store_refuses() {
    let runtime =
        harmos::Runtime::<crate::support::Bank>::builder(crate::support::Ledger::default(), ())
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .register(Lanes::catalog())
            .runtime()
            .await
            .unwrap();
    let mut lifecycle =
        Lifecycle::<Operations, _>::for_component(&runtime, schematic(), Lanes::Open);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let handle = lifecycle.component();
    assert_eq!(
        runtime
            .operations
            .call_as::<propose::ProposeRoute>(route(3))
            .await
            .unwrap()
            .points,
        corner(3)
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    runtime
        .operations
        .register_operation_id::<ComponentOperation<propose::ProposeRoute>>(
            "retirement-probe",
            "retired.route",
            Lanes::Open,
            handle.clone(),
        )
        .unwrap();
    let retired = runtime
        .operations
        .submit_at::<propose::ProposeRoute>("retired.route", route(3))
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
    runtime
        .operations
        .unregister("retired.route")
        .await
        .unwrap();
    assert!(
        runtime
            .operations
            .call_as::<propose::ProposeRoute>(route(3))
            .await
            .is_err()
    );
    drop((handle, manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_original_owner() {
    use std::{num::NonZeroUsize, time::Duration};
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    assert!(runtime.grpc().methods().is_empty());
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(runtime.grpc().methods().len(), 3);
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
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let settings = harmos::grpc::server::Settings::default();
    let codec = harmos::grpc::Codec::<
        propose::Response,
        harmos::operations::Response<propose::Request>,
    >::new(settings.decode, settings.encode)
    .unwrap();
    let response = client
        .unary(
            tonic::Request::new(harmos::operations::Response::new(route(3))),
            tonic::codegen::http::uri::PathAndQuery::from_static(
                "/schematic.v1.Schematic/ProposeRoute",
            ),
            codec,
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.points, corner(3));
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert!(runtime.grpc().methods().is_empty());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn stream_lifecycle_preserved() {
    use super::schematic::model::{Connection, Snapshot};
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let settings = harmos::operations::StreamSettings {
        items: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(4096).unwrap(),
    };
    let request = analyze::Request {
        schematic: Snapshot {
            symbols: vec![1, 2],
            connections: vec![Connection { from: 1, to: 2 }],
        },
    };
    let mut download = runtime
        .operations
        .download::<analyze::AnalyzeSchematic>(request, settings)
        .unwrap();
    for expected in ["2 symbols", "1 connections", "Analysis complete"] {
        assert_eq!(download.next().await.unwrap().unwrap().text, expected);
    }
    assert!(download.next().await.unwrap().is_none());
    assert_eq!(download.join().await.unwrap().checked, 3);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

/* === Tests === */
