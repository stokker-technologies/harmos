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
use super::lifecycle::{GuestOperations, Lifecycle, Manager, serve_host};
use super::sandbox::{Counts, Read, Stage};
use super::schematic::model::{Connection, Point, Snapshot};
use super::schematic::operations::schematic::{analyze, propose, validate};
use super::{GuestLanes, corner, route, runtime, sandbox};
use harmos_runtime::Message;
use harmos_runtime::operations::{ComponentError, ComponentOperation, FailureKind, OperationDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos_runtime::lanes]
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
fn pending_settings() -> harmos_runtime::config::PluginSettings {
    harmos_runtime::config::PluginSettings {
        configuration: serde_json::to_vec("shutdown-pending").unwrap(),
        shutdown_timeout: std::time::Duration::from_millis(20),
        host_operations: ["host.publish", "provider.echo"].map(|id| harmos_runtime::Identifier::new(id).unwrap()).to_vec(),
        ..Default::default()
    }
}

#[tokio::test]
async fn inert_plan_denies() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let definitions = lifecycle.candidate(()).await;
    let invalid = definitions.definitions().cloned().map(|mut definition| {
        if definition.id == "schematic-tools" {
            definition.configuration = b"{}".to_vec();
        }
        definition
    });
    let mut manager = Manager::new(harmos_runtime::plugin::PluginDefinitions::new(invalid, []).unwrap()).unwrap();
    assert!(manager.caller("schematic-tools").is_err());
    assert!(matches!(runtime.operations.call(route(4)).await, Err(harmos_runtime::Error::NotRegistered { domain: "operation", .. })));
    assert!(manager.initialize(&mut lifecycle).await.is_err(), "a unit configuration refuses an object");
    manager.shutdown(&mut lifecycle).await.unwrap();

    let plan = manager.plan_reload(lifecycle.candidate(()).await).unwrap();
    assert!(manager.caller("schematic-tools").is_err(), "collection and planning retain an inert original store");
    manager.apply_reload(plan, &mut lifecycle).await.unwrap();
    assert!(manager.caller("schematic-tools").is_ok());
    let response = runtime.operations.call(route(3)).await.unwrap();
    assert_eq!(response.points, corner(3));
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn declared_operation_roundtrip() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let answered = runtime.operations.call(route(3)).await.unwrap();
    assert_eq!(answered.points, corner(3));

    // A Straight Route Collapses Its Corner Onto The Start
    let operation = ["schematic", "route"].join(".");
    let encoded = runtime
        .operations
        .call_encoded(&operation, propose::ProposeRoute { from: Point { x: 0, y: 0 }, to: Point { x: 0, y: 4 } }.encode_to_vec())
        .await
        .unwrap();
    assert_eq!(propose::Route::decode(encoded.as_slice()).unwrap().points, [Point { x: 0, y: 0 }, Point { x: 0, y: 4 }]);
    assert!(matches!(
        runtime.operations.call_encoded("schematic.unknown", vec![]).await,
        Err(harmos_runtime::Error::NotRegistered { domain: "operation", .. })
    ));

    let validated = runtime
        .operations
        .call(validate::ValidateSchematic { schematic: Snapshot { symbols: vec![1, 1], connections: vec![Connection { from: 1, to: 2 }] } })
        .await
        .unwrap();
    assert_eq!(validated.diagnostics, ["Duplicate Symbol IDs", "Connection 1 To 2 References A Missing Symbol"]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn wrong_catalog_refused() {
    let runtime = runtime().await;
    serve_host(&runtime, GuestLanes::Open);
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let refused =
        runtime.plugin.register_guest::<()>(host.load(&sandbox()).unwrap(), pending_settings(), GuestLanes::Open).await.unwrap_err();
    assert!(matches!(refused, harmos_runtime::Error::InvalidInput { field: "guest operation catalog", .. }));
    assert!(runtime.plugin.status("sandbox-guest").is_none());
    runtime.stop().await.unwrap();
}

// A Pending Guest Stop Retains Cleanup After Its Deadline
#[tokio::test]
async fn shutdown_retains_cleanup() {
    let runtime = runtime().await;
    serve_host(&runtime, GuestLanes::Open);
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    runtime.plugin.register_guest::<GuestOperations>(host.load(&sandbox()).unwrap(), pending_settings(), GuestLanes::Open).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status("sandbox-guest").unwrap();
    assert_eq!(ready.attempts, 1);

    // The Guest Never Finishes Stopping, So Its Shutdown Deadline Elapses
    let error = runtime.plugin.stop("sandbox-guest").await.unwrap_err();
    assert!(format!("{error:?}").contains("guest plugin shutdown timeout"), "{error:?}");
    let failed = runtime.plugin.status("sandbox-guest").unwrap();
    assert_eq!(failed.state, harmos_runtime::plugin::PluginState::Failed);
    assert_eq!(failed.generation, ready.generation);
    assert_eq!(failed.attempts, 1);
    assert!(failed.next_retry.is_none());
    assert!(failed.cleanup_pending);
    assert!(runtime.operations.call(Read { value: "fenced".into() }).await.is_err());

    // Stopping Again Awaits The Retained Cleanup Without Another Attempt
    runtime.plugin.stop("sandbox-guest").await.unwrap();
    assert_eq!(runtime.plugin.status("sandbox-guest").unwrap().attempts, 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn overlay_preserves_parent() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert_portable(Stage { id: "parent".into() }).unwrap();
    let caller = runtime.operations.with_context(context.clone());
    let answer = caller.call(Read { value: "context-parent".into() }).await.unwrap();
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
    assert_eq!(runtime.operations.call(Read { value: "value".into() }).await.unwrap().value.as_str(), "value:2");

    // Retain Both Original Receipts While Their Physical Cleanup Owns The Table
    let mut first = runtime.operations.submit(Read { value: "cleanup".into() }).unwrap();
    let first_error = (&mut first).await.unwrap_err();
    let mut second = runtime.operations.submit(Read { value: "cleanup".into() }).unwrap();
    let second_error = (&mut second).await.unwrap_err();
    assert_ne!(first.id(), second.id());
    for error in [first_error, second_error] {
        let harmos_runtime::Error::OperationFinalization { operation, outcome, source } = error else {
            panic!("failed cleanup must retain the original operation owner")
        };
        assert_eq!(operation, Read::ID);
        assert!(outcome.is_none(), "the original retry completed before physical cleanup failed");
        let harmos_runtime::Error::OperationFailed { source, .. } = source.as_ref() else {
            panic!("the original finalization handler error is preserved")
        };
        assert!(matches!(source.downcast_ref::<ComponentError>(),
        Some(ComponentError::Remote(failure))
            if failure.code() == FailureKind::Transient
                && failure.cause().contains("first cleanup retained")));
    }
    let error = runtime.operations.call(Read { value: "later".into() }).await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = error else {
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
        .register_operation_id::<ComponentOperation<Read>>("retirement-probe", "retired.read", GuestLanes::Open, original.clone())
        .unwrap();
    let retired = runtime.operations.submit_at::<Read>("retired.read", Read { value: "later".into() }).unwrap().await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = retired else { panic!("the retained generation must refuse execution") };
    assert!(matches!(source.downcast_ref::<ComponentError>(), Some(ComponentError::Unavailable)));
    runtime.operations.unregister("retired.read").await.unwrap();
    manager.reload(lifecycle.candidate("normal").await, &mut lifecycle).await.unwrap();
    assert_eq!(runtime.operations.call(Read { value: "later".into() }).await.unwrap().value.as_str(), "later:2");
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
            refused_catalog |= failure.cause().contains("carrier handler definitions differ from the selected operation catalog");
        }
        source = if let Some(harmos_runtime::Error::OperationFailed { source, .. }) = error.downcast_ref::<harmos_runtime::Error>() {
            Some(source.as_ref())
        } else {
            error.source()
        };
    }
    assert!(refused_catalog, "SDK initialization must refuse the actual missing binding: {error}");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn children_retire_authority() {
    for serial in [false, true] {
        let runtime = harmos_runtime::Runtime::<crate::guest::support::Bank>::builder(crate::guest::support::Ledger::default(), ())
            .operation_settings(harmos_runtime::operations::Settings {
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
        let result = runtime.operations.call(Read { value: "parent".into() }).await;
        if serial {
            assert!(matches!(result, Err(harmos_runtime::Error::OperationFailed { .. })));
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
    let mut invocation = runtime.operations.submit(Read { value: "initialize-fail".into() }).unwrap();
    let error = (&mut invocation).await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("original initialization error") };
    assert!(
        matches!(source.downcast_ref::<ComponentError>(), Some(ComponentError::Preparation(failure)) if failure.code() == FailureKind::Transient && failure.cause().contains("partial initialization failed"))
    );
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Settled);
    let counts = runtime.operations.call(Counts { value: "counts".into() }).await.unwrap();
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
    let result = runtime.operations.call(Read { value: "initialize-child".into() }).await.unwrap();
    assert_eq!(result.value, "initialize-child:2");
    let counts = runtime.operations.call(Counts { value: "counts".into() }).await.unwrap();
    assert_eq!(counts.value, "2:3:2:2");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn wire_alias_refused() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();

    // A Wire Declaration Keeps Its Original Identity Under Any Other Operation Id
    let refused = runtime
        .operations
        .register_operation_id::<ComponentOperation<propose::ProposeRoute>>(
            "alias-probe",
            "aliased.route",
            GuestLanes::Open,
            lifecycle.component(),
        )
        .unwrap_err();
    assert!(
        matches!(
            refused,
            harmos_runtime::Error::InvalidInput {
                field: "operation wire declaration",
                reason: "wire projection must match the original typed declaration and identity",
            }
        ),
        "{refused:?}"
    );
    assert!(matches!(
        runtime.operations.submit_at::<propose::ProposeRoute>("aliased.route", route(3)),
        Err(harmos_runtime::Error::NotRegistered { .. })
    ));
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_original_owner() {
    use std::num::NonZeroUsize;
    use std::time::Duration;
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    assert!(runtime.grpc().methods().is_empty());
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(runtime.grpc().methods().len(), 3);
    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        operations: harmos_runtime::grpc::server::Settings::default(),
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
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let settings = harmos_runtime::grpc::server::Settings::default();
    let codec = harmos_runtime::grpc::Codec::<
        harmos_runtime::grpc::Transcoded,
        propose::Route,
        harmos_runtime::operations::Response<propose::ProposeRoute>,
    >::new(settings.decode, settings.encode)
    .unwrap();
    let response = client
        .unary(
            tonic::Request::new(harmos_runtime::operations::Response::new(route(3))),
            tonic::codegen::http::uri::PathAndQuery::from_static("/schematic.v1.Schematic/ProposeRoute"),
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
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let settings = harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(4096).unwrap(),
    };
    let request =
        analyze::AnalyzeSchematic { schematic: Snapshot { symbols: vec![1, 2], connections: vec![Connection { from: 1, to: 2 }] } };
    let mut download = runtime.operations.download::<analyze::AnalyzeSchematic>(request, settings).unwrap();
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
