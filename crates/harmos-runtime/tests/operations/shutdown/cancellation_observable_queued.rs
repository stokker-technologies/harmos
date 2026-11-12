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

#[tokio::test]
async fn cancellation_observable_queued() {
    let (runtime, trace) = runtime().await;
    let first = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    let queued = runtime.operation(WaitContractDeclaration { mode: Mode::Count }).start().await.unwrap();
    queued.request_cancel();
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(trace.counted.load(Ordering::SeqCst), 0);
    trace.release.notify_one();
    first.await.unwrap();

    let running = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;

    running.request_cancel();
    assert!(matches!(running.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));

    // The cancelled queued instance was never constructed.
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
