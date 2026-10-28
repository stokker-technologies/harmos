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
use super::sandbox::{CountsOperation, ReadOperation, Stage};
use super::schematic::model::{Connection, Point, Snapshot};
use super::{GuestLanes, corner, route, runtime, sandbox, support};
use harmos_runtime::Message;
use harmos_runtime::operations::ComponentError;
use schematic_operations::schematic::{analyze, propose, validate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

use support::Lanes;

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn pending_settings() -> harmos_runtime::config::PluginSettings {
    let host_operations = ["host.publish", "provider.echo"];
    let host_operations = host_operations.map(|id| harmos_runtime::Identifier::new(id).unwrap());
    harmos_runtime::config::PluginSettings {
        configuration: serde_json::to_vec("shutdown-pending").unwrap(),
        shutdown_timeout: std::time::Duration::from_millis(20),
        host_operations: host_operations.to_vec(),
        ..Default::default()
    }
}

#[tokio::test]
async fn inert_plan_denies() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let definitions = lifecycle.candidate(()).await;
    let invalid = definitions.definitions().cloned().map(|mut definition| {
        if definition.id == "schematic_tools" {
            definition.configuration = b"{}".to_vec();
        }
        definition
    });
    let mut manager = Manager::new(harmos_runtime::plugin::PluginDefinitions::new(invalid, []).unwrap()).unwrap();
    assert!(manager.caller("schematic_tools").is_err());
    assert!(matches!(runtime.operation(route(4)).run().await, Err(harmos_runtime::Error::NotRegistered { domain: "operation", .. })));
    assert!(manager.initialize(&mut lifecycle).await.is_err(), "a unit configuration refuses an object");

    manager.shutdown(&mut lifecycle).await.unwrap();

    let plan = manager.plan_reload(lifecycle.candidate(()).await).unwrap();
    assert!(manager.caller("schematic_tools").is_err(), "collection and planning retain an inert original store");
    manager.apply_reload(plan, &mut lifecycle).await.unwrap();
    assert!(manager.caller("schematic_tools").is_ok());
    let response = runtime.operation(route(3)).run().await.unwrap();
    assert_eq!(response.points, corner(3));

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn declared_operation_roundtrip() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let answered = runtime.operation(route(3)).run().await.unwrap();
    assert_eq!(answered.points, corner(3));

    // A Straight Route Collapses Its Corner Onto The Start
    let operation = ["schematic_tools", "schematic", "route"].join(".");
    let encoded = runtime
        .operations
        .call_encoded(
            &operation,
            propose::ProposeRoute {
                from: Point { x: 0, y: 0 },
                to: Point { x: 0, y: 4 },
            }
            .encode(),
        )
        .await
        .unwrap();
    assert_eq!(propose::Route::decode(encoded.as_slice()).unwrap().points, [Point { x: 0, y: 0 }, Point { x: 0, y: 4 }]);
    assert!(matches!(
        runtime.operations.call_encoded("schematic.unknown", vec![]).await,
        Err(harmos_runtime::Error::NotRegistered { domain: "operation", .. })
    ));

    let validated = runtime
        .operation(validate::ValidateSchematic {
            schematic: Snapshot {
                symbols: vec![1, 1],
                connections: vec![Connection { from: 1, to: 2 }],
            },
        })
        .run()
        .await
        .unwrap();
    assert_eq!(validated.diagnostics, ["Duplicate Symbol IDs", "Connection 1 To 2 References A Missing Symbol"]);

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn wrong_catalog_refused() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    serve_host(&runtime, GuestLanes::Open).await;
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let bytes = sandbox();
    let loaded = host.load(&bytes).unwrap();
    let refused = runtime.plugin.register_guest::<()>(loaded, pending_settings(), GuestLanes::Open).await.unwrap_err();
    assert!(matches!(refused, harmos_runtime::Error::InvalidInput { field: "guest operation catalog", .. }));
    assert!(runtime.plugin.status("sandbox_guest").is_none());
    runtime.shutdown().await.unwrap();
}

// A Pending Guest Stop Retains Cleanup After Its Deadline
#[tokio::test]
async fn shutdown_retains_cleanup() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    serve_host(&runtime, GuestLanes::Open).await;
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let bytes = sandbox();
    let loaded = host.load(&bytes).unwrap();
    runtime.plugin.register_guest::<GuestOperations>(loaded, pending_settings(), GuestLanes::Open).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(ready.attempts, 1);

    // The Guest Never Finishes Stopping, So Its Shutdown Deadline Elapses
    let error = runtime.plugin.stop("sandbox_guest").await.unwrap_err();
    assert!(format!("{error:?}").contains("guest plugin shutdown timeout"), "{error:?}");
    let failed = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(failed.state, harmos_runtime::plugin::PluginState::Failed);
    assert_eq!(failed.generation, ready.generation);
    assert_eq!(failed.attempts, 1);
    assert!(failed.next_retry.is_none());
    assert!(failed.cleanup_pending);
    assert!(runtime.operation(ReadOperation { value: "fenced".into() }).run().await.is_err());

    // Each Later Stop Resumes The Retained Cleanup Until The Guest's Own Shutdown Deadline Settles It, Without Another Attempt
    let mut settled = false;
    while let Err(pending) = runtime.plugin.stop("sandbox_guest").await {
        let pending = format!("{pending:?}");
        assert!(!settled, "the stop after the guest deadline completes cleanup: {pending}");
        settled = pending.contains("OperationRefused(TimedOut)");
        assert!(settled || pending.contains("guest plugin shutdown timeout"), "{pending}");
    }
    assert!(settled, "the guest's own shutdown deadline settled the retained invocation");
    assert_eq!(runtime.plugin.status("sandbox_guest").unwrap().attempts, 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn overlay_preserves_parent() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime).await;
    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();

    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert_portable(Stage { id: "parent".into() }).unwrap();
    let caller = runtime.operations.with_context(context.clone());
    let answer = caller.call::<ReadOperation>(ReadOperation { value: "context-parent".into() }).await.unwrap();
    assert_eq!(answer.value, "parent:child");
    assert_eq!(context.value::<Stage>().unwrap().unwrap().id, "parent");

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn request_clones_and_resource_attempts() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime).await;
    let mut manager = Manager::new(lifecycle.candidate("normal").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();

    for value in ["first", "second", "third"] {
        let response = runtime.operation(ReadOperation { value: value.into() }).run().await.unwrap();
        assert_eq!(response.value, format!("{value}:2"));
    }
    let counts = runtime.operation(CountsOperation { value: "counts".into() }).run().await.unwrap();
    assert_eq!(counts.value, "6");

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn exact_live_bindings() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime).await;
    let mut manager = Manager::new(lifecycle.candidate("mismatch").await).unwrap();
    let error = manager.initialize(&mut lifecycle).await.unwrap_err();

    let mut source = Some(&error as &dyn std::error::Error);
    let mut refused_catalog = false;
    while let Some(error) = source {
        let remote = error.downcast_ref::<ComponentError>();
        refused_catalog |= matches!(remote, Some(ComponentError::Remote(failure))
            if failure.cause().contains("carrier handler definitions differ from the selected operation catalog"));
        source = if let Some(harmos_runtime::Error::OperationFailed { source, .. }) = error.downcast_ref::<harmos_runtime::Error>() {
            Some(source.as_ref())
        } else {
            error.source()
        };
    }
    assert!(refused_catalog, "SDK initialization must refuse the actual missing binding: {error}");

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn children_retire_authority() {
    let _report = crate::report::system();
    for serial in [false, true] {
        children_in_lane(serial).await;
    }
}

async fn children_in_lane(serial: bool) {
    let preparation_bytes = std::num::NonZeroUsize::new(4096);
    let runtime = harmos_runtime::Runtime::<support::Bank>::launch_with(support::Ledger::default(), (), |setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings { preparation_bytes, ..Default::default() });
    })
    .await
    .unwrap();
    let lane = if serial { Lanes::Serial } else { Lanes::Open };
    let mut lifecycle = Lifecycle::sandbox_in(&runtime, lane).await;
    let mut manager = Manager::new(lifecycle.candidate("children").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let result = runtime.operation(ReadOperation { value: "parent".into() }).run().await;
    if serial {
        assert!(matches!(result, Err(harmos_runtime::Error::OperationFailed { .. })));
    } else {
        assert_eq!(result.unwrap().value.as_str(), "parent:2:stale:leaf:1");
    }

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn grpc_original_owner() {
    let _report = crate::report::system();
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
    let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap();
    let channel = endpoint.connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();

    let settings = harmos_runtime::grpc::server::Settings::default();
    let codec = harmos_runtime::grpc::Codec::<
        harmos_runtime::grpc::Transcoded,
        propose::Route,
        harmos_runtime::operations::Response<propose::ProposeRoute>,
    >::new(settings.decode, settings.encode)
    .unwrap();
    let request = harmos_runtime::operations::Response::new(route(3));
    let request = tonic::Request::new(request);
    let response = client
        .unary(request, tonic::codegen::http::uri::PathAndQuery::from_static("/schematic.v1.Schematic/ProposeRoute"), codec)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.points, corner(3));

    manager.shutdown(&mut lifecycle).await.unwrap();
    assert!(runtime.grpc().methods().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn stream_lifecycle_preserved() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::schematic(&runtime);
    let mut manager = Manager::new(lifecycle.candidate(()).await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();

    let settings = harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(4096).unwrap(),
    };
    let request = analyze::AnalyzeSchematic {
        schematic: Snapshot {
            symbols: vec![1, 2],
            connections: vec![Connection { from: 1, to: 2 }],
        },
    };
    let mut download = runtime.operations.download::<analyze::AnalyzeSchematic>(request, settings).unwrap();
    for expected in ["2 symbols", "1 connections", "Analysis complete"] {
        assert_eq!(download.next().await.unwrap().unwrap().text, expected);
    }
    assert!(download.next().await.unwrap().is_none());
    assert_eq!(download.join().await.unwrap().checked, 3);

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

/* === Tests === */
