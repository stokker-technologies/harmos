/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Authored Operation Admission, Child Ownership And Cleanup.

/* === Modules === */

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[tokio::test]
async fn catalog_mismatch_refused() {
    for change in 0..8 {
        let counts = Arc::new(Counts::default());
        let mut operations = Declarations::of::<Operations>().unwrap();
        match change {
            0 => {
                operations.pop();
            }
            1 => operations[0].policy.timeout += Duration::from_secs(1),
            2 => operations[0].visibility = Visibility::Shared,
            3 => operations[0].replay = harmos_runtime::operations::ReplaySafety::Idempotent,
            4 => operations[0].description.push('!'),
            5 => operations.push(Definition { id: harmos_runtime::Identifier::new("fixture.extra").unwrap(), ..operations[0].clone() }),
            6 => operations.push(operations[0].clone()),
            _ => operations[0].policy.timeout = Duration::ZERO,
        }
        let mut source = source(Arc::clone(&counts));
        source.operations = operations;
        let mut fixture = Managed::new(source, Lanes::NativeOpen).await;
        assert!(fixture.initialize().await.is_err());
        let status = service_status(fixture.runtime.operations.watch_residents(), "operation-handshake", |status| {
            matches!(status, ServiceStatus::Inactive { .. })
        })
        .await;
        let ServiceStatus::Inactive { cause: ServiceExit::Application(reason), .. } = status else {
            panic!("operation admission must fail before initialization: {status:?}");
        };
        let expected_reason = match change {
            6 => "appears more than once",
            7 => "operation deadlines must be positive",
            _ => "native canonical contracts differ from the admitted operations",
        };
        assert!(reason.contains(expected_reason), "{reason}");
        assert_eq!(counts.attached.load(Ordering::SeqCst), 0);
        assert_eq!(counts.initialized.load(Ordering::SeqCst), 0);
        assert_eq!(counts.cleaned.load(Ordering::SeqCst), 0);
        assert!(!*fixture.host.ready.as_ref().unwrap().borrow());
        assert!(matches!(fixture.runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
    }
}

#[tokio::test]
async fn catalog_order_admission() {
    let counts = Arc::new(Counts::default());
    let mut source = source(Arc::clone(&counts));
    source.operations.reverse();
    let mut fixture = Managed::new(source, Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    (fixture.host.ready.as_mut().unwrap().wait_for(|ready| *ready)).await.unwrap();
    assert_eq!(counts.attached.load(Ordering::SeqCst), 1);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
    assert!(!*fixture.host.ready.as_ref().unwrap().borrow());
}

#[tokio::test]
async fn runner_retry_finalization() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    fixture.call(Read {}).await.unwrap();
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn portable_call_context() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    fixture.call(Read {}).await.unwrap();
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal).unwrap();
    context.insert_portable(Tenant { name: "lab".into() }).unwrap();
    fixture.call_in::<Read>(context, Read {}).await.unwrap();
    assert_eq!(
        *counts.tenants.lock().unwrap(),
        [None, None, Some("lab".to_owned()), Some("lab".to_owned())],
        "every retried attempt carries the admitted values; local ones stay"
    );
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn submitted_child_controls() {
    let counts = Arc::new(Counts::default());
    counts.submit_child.store(true, Ordering::SeqCst);
    counts.refresh_hold.store(true, Ordering::SeqCst);
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    harmos_runtime::builtin::Catalog::register(&fixture.runtime.operations, "host", Lanes::NativeOpen).unwrap();
    fixture.initialize().await.unwrap();
    fixture.call(Read {}).await.unwrap();
    assert_eq!(counts.children.load(Ordering::SeqCst), 4);
    assert_eq!(counts.refresh_completed.load(Ordering::SeqCst), 2);
    assert_eq!(counts.refresh_cleaned.load(Ordering::SeqCst), 4);
    let refused = counts.detached_status.lock().unwrap().take().unwrap();
    assert!(refused.contains("may not invoke operation"), "{refused}");
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cleanup_retry_once() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeSerial).await;
    fixture.initialize().await.unwrap();
    let runtime = &fixture.runtime;
    counts.fail_cleanup.store(true, Ordering::SeqCst);
    assert!(matches!(fixture.call(Read {}).await, Err(harmos_runtime::Error::OperationFinalization { .. })));
    assert!(runtime.operations.unregister(Read::ID).await.is_err());
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
    counts.fail_cleanup.store(false, Ordering::SeqCst);
    runtime.operations.unregister(Read::ID).await.unwrap();
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn preparation_refusal_state() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    counts.fail_prepare.store(true, Ordering::SeqCst);
    let Err(harmos_runtime::Error::OperationFailed { source, .. }) = fixture.call(Read {}).await else {
        panic!("preparation is refused before any attempt")
    };
    assert!(matches!(
        source.downcast_ref::<harmos_runtime::operations::NativeError>(),
        Some(harmos_runtime::operations::NativeError::Preparation { .. })
    ));
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn children_attempt_lifetime() {
    for serial in [false, true] {
        let counts = Arc::new(Counts::default());
        counts.call_child.store(true, Ordering::SeqCst);
        let lane = if serial { Lanes::NativeSerial } else { Lanes::NativeOpen };
        let mut fixture = Managed::new(source(counts.clone()), lane).await;
        fixture.initialize().await.unwrap();
        let result = fixture.call(Read {}).await;
        if serial {
            assert!(matches!(result, Err(harmos_runtime::Error::OperationFailed { .. })));
            assert_eq!(counts.children.load(Ordering::SeqCst), 0);
        } else {
            result.unwrap();
            assert_eq!(counts.children.load(Ordering::SeqCst), 2);
            assert_eq!(counts.stale_refused.load(Ordering::SeqCst), 1);
            assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
            assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
        }
        let saved = counts.saved.lock().unwrap().take().unwrap();
        assert!(saved.call(Refresh {}).await.is_err());
        fixture.shutdown().await.unwrap();
        fixture.runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn initialization_failure() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    counts.fail_initialize.store(true, Ordering::SeqCst);
    let Err(harmos_runtime::Error::OperationFailed { source, .. }) = fixture.call(Read {}).await else {
        panic!("initialization returns the original failure")
    };
    assert!(matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
        Some(harmos_runtime::operations::NativeError::Preparation(failure)) if failure.code() == harmos_runtime::operations::FailureKind::Transient && failure.cause().contains("partial initialization failed")));
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
    // A failed or cancelled initialization owns no instance to clean up.
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn initialization_child_expires() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    counts.initialization_child.store(true, Ordering::SeqCst);
    fixture.call(Read {}).await.unwrap();
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.children.load(Ordering::SeqCst), 1);
    assert_eq!(counts.stale_refused.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_init_resources() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    fixture.initialize().await.unwrap();
    counts.block_initialize.store(true, Ordering::SeqCst);
    let mut invocation = fixture.runtime.operations.submit_encoded(Read::ID, Vec::new()).unwrap();
    (counts.initialization_entered.notified()).await;
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Running);
    invocation.cancel();
    let result = (&mut invocation).await;
    assert!(matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Settled);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
    // A failed or cancelled initialization owns no instance to clean up.
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 0);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 0);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_original_owner() {
    use std::num::NonZeroUsize;
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::NativeOpen).await;
    assert!(fixture.runtime.grpc().methods().is_empty());
    fixture.initialize().await.unwrap();
    assert_eq!(fixture.runtime.grpc().methods().len(), 4);
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
    let attached = fixture.runtime.grpc().attach(settings).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let settings = harmos_runtime::grpc::server::Settings::default();
    // Unknown Protobuf Fields Are Legal For Unit; Postcard Unit Rejects These Bytes.
    let codec = harmos_runtime::grpc::Codec::<harmos_runtime::grpc::Transcoded, (), harmos_runtime::operations::Response<u32>>::new(
        settings.decode,
        settings.encode,
    )
    .unwrap();
    client
        .unary(
            tonic::Request::new(harmos_runtime::operations::Response::new(42u32)),
            tonic::codegen::http::uri::PathAndQuery::from_static("/fixture.v1.Native/Read"),
            codec,
        )
        .await
        .unwrap();
    assert_eq!(counts.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(counts.operation_initialized.load(Ordering::SeqCst), 1);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    assert!(fixture.runtime.grpc().methods().is_empty());
    fixture.runtime.stop().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
}

/* === Tests === */
