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
async fn exchange_failure_settles_attempt() {
    use futures_util::StreamExt;
    let trace = Arc::new(Trace::default());
    let runtime = runtime_with_resource(trace.clone()).await;
    runtime
        .operations
        .register_operation(forward_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let (input, receiving) = channel();
    let (sending, output) = channel();
    let instance =
        runtime.operations.submit_endpoints::<ForwardDeclaration, _>(ForwardDeclaration { consume: true }, (receiving, sending)).unwrap();
    // Only a constructed instance has cleanup to retain.
    let initialized = trace.clone();
    let incoming = futures_util::stream::once(async move {
        initialized.started.notified().await;
        Err(tonic::Status::data_loss("broken input"))
    });
    let mut exchange = harmos_runtime::grpc::exchange(instance, input, incoming, output);
    let error = match bounded(exchange.next()).await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("expected forwarding failure"),
    };
    assert!(matches!(error, harmos_runtime::grpc::InputError::Forward { settlement: Some(ref settlement), .. }
        if matches!(settlement.as_ref(), Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
    assert!(bounded(exchange.next()).await.is_none());
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    bounded(crate::support::app::shutdown(&runtime)).await.unwrap();
    assert_eq!(trace.new.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
}
