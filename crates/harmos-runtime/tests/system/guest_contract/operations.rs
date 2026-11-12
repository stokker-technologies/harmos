/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::lifecycle::{Lifecycle, Manager, guest_catalog, serve_host};
use super::sandbox::{CountsOperation, ReadOperation, Stage};
use super::schematic::model::{Connection, Point, Snapshot};
use super::{corner, route, runtime, sandbox, support};
use harmos_runtime::operations::ComponentError;
use schematic_operations::guest::schematic::{analyze, propose, validate};

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
    let mut manager = Manager::new(harmos_runtime::guest::PluginDefinitions::new(invalid, []).unwrap()).unwrap();
    assert!(manager.caller("schematic_tools").is_err());
    assert!(matches!(
        runtime.operation(route(4)).await,
        Err(harmos_runtime::Error::NotRegistered {
            domain: "unambiguous operation request",
            ..
        })
    ));
    assert!(manager.initialize(&mut lifecycle).await.is_err(), "a unit configuration refuses an object");

    manager.shutdown(&mut lifecycle).await.unwrap();

    let candidate = lifecycle.candidate(()).await;
    assert!(manager.caller("schematic_tools").is_err(), "collection retains an inert original store");
    manager.reload(candidate, &mut lifecycle).await.unwrap();
    assert!(manager.caller("schematic_tools").is_ok());
    let response = runtime.operation(route(3)).await.unwrap();
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
    let answered = runtime.operation(route(3)).await.unwrap();
    assert_eq!(answered.points, corner(3));

    // A Straight Route Collapses Its Corner Onto the Start
    let operation = ["schematic_tools", "schematic", "route"].join(".");
    let request = propose::ProposeRoute {
        from: Point { x: 0, y: 0 },
        to: Point { x: 0, y: 4 },
    };
    let mut payload = Vec::new();
    harmos_operations::execution::grpc::encode(&request, &mut payload).unwrap();
    let encoded = runtime.operations.call_encoded(&operation, payload).await.unwrap();
    let route = harmos_operations::execution::grpc::decode::<propose::Route>(harmos_operations::execution::grpc::DecodeInput::new(
        encoded.as_slice(),
    ))
    .unwrap();
    assert_eq!(route.points, [Point { x: 0, y: 0 }, Point { x: 0, y: 4 }]);
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
        .await
        .unwrap();
    assert_eq!(validated.diagnostics, ["Duplicate Symbol IDs", "Connection 1 to 2 References a Missing Symbol"]);

    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn wrong_catalog_refused() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    serve_host(&runtime, harmos_runtime::operations::Lane::default()).await;
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let bytes = sandbox();
    let loaded = host.load(&bytes).unwrap();
    let refused = runtime
        .plugin
        .register_guest(
            harmos_runtime::operations::ComponentCatalog::new(),
            loaded,
            pending_settings(),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(refused, harmos_runtime::Error::InvalidInput { field: "guest operation catalog", .. }));
    assert!(runtime.plugin.status("sandbox_guest").is_none());
    runtime.shutdown().await.unwrap();
}

// A Pending Guest Stop Retains Cleanup After Its Deadline
#[tokio::test(start_paused = true)]
async fn shutdown_retains_cleanup() {
    let _report = crate::report::system();
    let runtime = runtime().await;
    serve_host(&runtime, harmos_runtime::operations::Lane::default()).await;
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let bytes = sandbox();
    let loaded = host.load(&bytes).unwrap();
    runtime.plugin.register_guest(guest_catalog(), loaded, pending_settings(), harmos_runtime::operations::Lane::default()).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(ready.attempts, 1);

    // The Guest Never Finishes Stopping, So Its Shutdown Deadline Elapses
    let error = runtime.plugin.stop("sandbox_guest").await.unwrap_err();
    assert!(format!("{error:?}").contains("guest plugin shutdown timeout"), "{error:?}");
    let failed = runtime.plugin.status("sandbox_guest").unwrap();
    assert_eq!(failed.state, harmos_runtime::guest::PluginState::Failed);
    assert_eq!(failed.generation, ready.generation);
    assert_eq!(failed.attempts, 1);
    assert!(failed.next_retry.is_none());
    assert!(failed.cleanup_pending);
    assert!(runtime.operation(ReadOperation { value: "fenced".into() }).await.is_err());

    // Each Later Stop Resumes the Retained Cleanup Until the Guest's Own Shutdown Deadline Settles It, Without Another Attempt
    let mut settled = false;
    while let Err(pending) = runtime.plugin.stop("sandbox_guest").await {
        assert!(!settled, "the stop after the guest deadline completes cleanup: {pending:?}");
        let harmos_runtime::Error::PluginLifecycle { source } = pending else { panic!("cleanup failure must retain its owner") };
        let harmos_runtime::guest::PluginFailure::Cleanup(failures) = *source else { panic!("expected retained cleanup") };
        let [failure] = failures.as_slice() else { panic!("one guest owns cleanup") };
        let harmos_runtime::Error::TaskFailed { source, .. } = &failure.error else { panic!("cleanup retains its original error") };
        let original = source.downcast_ref::<harmos_runtime::Error>().unwrap();
        match original {
            harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut { .. }) => settled = true,
            harmos_runtime::Error::NotReady { domain: "guest plugin shutdown timeout" } => {}
            other => panic!("unexpected cleanup failure: {other:?}"),
        }
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
    let answer = caller.operation::<ReadOperation>(ReadOperation { value: "context-parent".into() }).await.unwrap();
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
        let response = runtime.operation(ReadOperation { value: value.into() }).await.unwrap();
        assert_eq!(response.value, format!("{value}:2"));
    }
    let counts = runtime.operation(CountsOperation { value: "counts".into() }).await.unwrap();
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
    assert!(refused_catalog, "SDK initialization must refuse the actual extra binding: {error}");

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
    let lane = if serial { crate::support::SERIAL } else { harmos_runtime::operations::Lane::default() };
    let mut lifecycle = Lifecycle::sandbox_in(&runtime, lane).await;
    let mut manager = Manager::new(lifecycle.candidate("children").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let result = runtime.operation(ReadOperation { value: "parent".into() }).await;
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
    let before: Vec<_> = runtime.grpc().methods().into_iter().map(|method| (method.owner, method.generation, method.contract)).collect();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(runtime.grpc().methods().len(), before.len() + 3);

    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        operations: harmos_runtime::grpc::server::Settings::default(),
        reflection: true,
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
    let after: Vec<_> = runtime.grpc().methods().into_iter().map(|method| (method.owner, method.generation, method.contract)).collect();
    assert_eq!(after, before);
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

#[tokio::test]
async fn cancel_final_response_guest() {
    use sandbox::FinishOperation;
    let _report = crate::report::system();
    for mode in 0..3 {
        let runtime = runtime().await;
        let mut lifecycle = Lifecycle::sandbox(&runtime).await;
        let mut manager = Manager::new(lifecycle.candidate("cancel-final-response").await).unwrap();
        manager.initialize(&mut lifecycle).await.unwrap();
        let handle = runtime.operation(FinishOperation { mode }).start().await.unwrap();
        loop {
            let counts = runtime.operation(CountsOperation { value: String::new() }).await.unwrap();
            if counts.value == "1" {
                break;
            }
            tokio::task::yield_now().await;
        }
        let result = handle.cancel().await;
        if mode == 0 {
            assert_eq!(result.unwrap().value, "42");
            manager.shutdown(&mut lifecycle).await.unwrap();
        } else {
            assert!(matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
            super::retire_failed_guest(&mut manager, &mut lifecycle).await;
        }
        drop((manager, lifecycle));
        runtime.shutdown().await.unwrap();
    }
}
