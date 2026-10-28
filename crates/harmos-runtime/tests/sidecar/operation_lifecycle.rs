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
            2 => operations[0].visibility = harmos_runtime::operations::Visibility::Shared,
            3 => operations[0].replay = harmos_runtime::operations::ReplaySafety::Idempotent,
            4 => operations[0].description.push('!'),
            5 => operations.push(Definition {
                id: harmos_runtime::Identifier::new("fixture_extra").unwrap(),
                ..operations[0].clone()
            }),
            6 => operations.push(operations[0].clone()),
            _ => operations[0].policy.timeout = Duration::ZERO,
        }
        let mut source = source(Arc::clone(&counts));
        source.operations = operations;
        let mut fixture = Managed::new(source, Lanes::Open).await;
        let failure = fixture.initialize().await.unwrap_err();
        let reason = harmos_runtime::failure::Report(&failure).to_string();
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
        assert!(fixture.runtime.operations.residents().is_empty());
        fixture.runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn catalog_order_admission() {
    let counts = Arc::new(Counts::default());
    let mut source = source(Arc::clone(&counts));
    source.operations.reverse();
    let mut fixture = Managed::new(source, Lanes::Open).await;
    fixture.initialize().await.unwrap();
    (fixture.host.ready.as_mut().unwrap().wait_for(|ready| *ready)).await.unwrap();
    assert_eq!(counts.attached.load(Ordering::SeqCst), 1);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
    assert!(!*fixture.host.ready.as_ref().unwrap().borrow());
}

#[tokio::test]
async fn runner_retry_finalization() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.initialize().await.unwrap();
    fixture.call::<ReadDeclaration>(ReadDeclaration {}).await.unwrap();
    assert_eq!(counts.started.load(Ordering::SeqCst), 2);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.finished.load(Ordering::SeqCst), 2);
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn portable_call_context() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.initialize().await.unwrap();
    fixture.call::<ReadDeclaration>(ReadDeclaration {}).await.unwrap();
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal).unwrap();
    context.insert_portable(Tenant { name: "lab".into() }).unwrap();
    fixture.call_in::<ReadDeclaration>(context, ReadDeclaration {}).await.unwrap();
    assert_eq!(
        *counts.tenants.lock().unwrap(),
        [None, None, Some("lab".to_owned()), Some("lab".to_owned())],
        "every retried attempt carries the admitted values; local ones stay"
    );
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn submitted_child_controls() {
    let counts = Arc::new(Counts::default());
    counts.submit_child.store(true, Ordering::SeqCst);
    counts.refresh_hold.store(true, Ordering::SeqCst);
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.runtime.operations.register::<harmos_operations::Lifecycle>(Lanes::Open).await.unwrap();
    fixture.initialize().await.unwrap();
    fixture.call::<ReadDeclaration>(ReadDeclaration {}).await.unwrap();
    assert_eq!(counts.children.load(Ordering::SeqCst), 4);
    assert_eq!(counts.refresh_completed.load(Ordering::SeqCst), 2);
    assert_eq!(counts.refresh_cleaned.load(Ordering::SeqCst), 4);
    let refused = counts.detached_status.lock().unwrap().take().unwrap();
    assert!(refused.contains("may not invoke operation"), "{refused}");
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn children_attempt_lifetime() {
    for serial in [false, true] {
        let counts = Arc::new(Counts::default());
        counts.call_child.store(true, Ordering::SeqCst);
        let lane = if serial { Lanes::Serial } else { Lanes::Open };
        let mut fixture = Managed::new(source(counts.clone()), lane).await;
        fixture.initialize().await.unwrap();
        let result = fixture.call::<ReadDeclaration>(ReadDeclaration {}).await;
        if serial {
            assert!(matches!(result, Err(harmos_runtime::Error::OperationFailed { .. })));
            assert_eq!(counts.children.load(Ordering::SeqCst), 0);
        } else {
            result.unwrap();
            assert_eq!(counts.children.load(Ordering::SeqCst), 2);
            assert_eq!(counts.stale_refused.load(Ordering::SeqCst), 1);
            assert_eq!(counts.started.load(Ordering::SeqCst), 2);
            assert_eq!(counts.finished.load(Ordering::SeqCst), 2);
        }
        let saved = counts.saved.lock().unwrap().take().unwrap();
        assert!(saved.call::<RefreshDeclaration>(RefreshDeclaration {}).await.is_err());
        fixture.shutdown().await.unwrap();
        fixture.runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_attempt_resources() {
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
    fixture.initialize().await.unwrap();
    counts.block_attempt.store(true, Ordering::SeqCst);
    let mut invocation =
        fixture.runtime.operations.submit_encoded(&format!("operation_handshake.{}", ReadDeclaration::ID), Vec::new()).unwrap();
    (counts.attempt_entered.notified()).await;
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Running);
    invocation.cancel();
    let result = (&mut invocation).await;
    assert!(matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Settled);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 0);
    // Cancellation Drops The Attempt Before Returning Its Outcome.
    assert_eq!(counts.finished.load(Ordering::SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_original_owner() {
    use std::num::NonZeroUsize;
    let counts = Arc::new(Counts::default());
    let mut fixture = Managed::new(source(counts.clone()), Lanes::Open).await;
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
    assert_eq!(counts.started.load(Ordering::SeqCst), 2);
    assert_eq!(counts.runs.load(Ordering::SeqCst), 2);
    assert_eq!(counts.finished.load(Ordering::SeqCst), 2);
    fixture.shutdown().await.unwrap();
    assert!(fixture.runtime.grpc().methods().is_empty());
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);
}

/* === Tests === */
