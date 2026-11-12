/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Local Imports
use super::*;

/* === Tests === */

#[cfg(feature = "grpc")]
#[tokio::test]
async fn input_failure_settles_attempt() {
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime
        .operations
        .register_operation(forward_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let (input, receiving) = channel();
    let (sending, _output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to preserve.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        initialized.started.notified().await;
        Err(tonic::Status::data_loss("broken input"))
    });
    let error = bounded(harmos_runtime::grpc::input(instance, input, incoming)).await.unwrap_err();
    let harmos_runtime::grpc::InputError::Forward { source, settlement } = error else { panic!("input failure missing") };
    assert!(matches!(*source, Error::TaskFailed { domain: "gRPC input", .. }));
    assert!(matches!(settlement.as_deref(), Some(Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(crate::support::app::shutdown(&runtime)).await.unwrap();
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}
