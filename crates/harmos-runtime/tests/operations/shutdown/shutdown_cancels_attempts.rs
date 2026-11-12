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
async fn shutdown_cancels_attempts() {
    let (runtime, trace) = runtime().await;
    let instance = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(matches!(instance.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(runtime.operations.submit_resident("late").await, Err(Error::Closing { domain: "resident" })));
}
