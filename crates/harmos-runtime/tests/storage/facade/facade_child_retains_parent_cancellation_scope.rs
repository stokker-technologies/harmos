/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Facade Integration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{Capture, ChildRequest, Gate, Model, Parent, Probe, payload};
use harmos_runtime::{Error, Runtime, operations::CallContext};
use std::sync::Arc;
use std::sync::atomic::Ordering;

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

#[tokio::test(start_paused = true)]
async fn facade_child_retains_parent_cancellation_scope() {
    // Keep the Child Handle After Cancelling Its Parent
    let runtime = Runtime::<Model>::start((), ()).await.unwrap();
    let second = Runtime::<Model>::start((), ()).await.unwrap();
    let context = runtime.for_call(&CallContext::default()).call::<Capture>(()).await.unwrap();
    let request = payload(0);
    let result = second.for_call(&context).call::<Probe>(request.clone()).await;
    assert!(matches!(result, Err(Error::InvalidInput { .. })));
    assert_eq!(request.attempts.load(Ordering::SeqCst), 0);

    // Retain the Child's Handle While Cancelling Its Parent
    let gate = Arc::new(Gate::default());
    let parent = runtime.for_call(&CallContext::default()).submit::<Parent>(ChildRequest(gate.clone()), ()).unwrap();
    gate.loaded.notified().await;
    gate.started.notified().await;
    let child = gate.child.lock().unwrap().take().unwrap();

    // Parent Cancellation Settles the Child Even While Its Handle Is Retained
    parent.request_cancel();
    let _outcome = parent.await;
    assert!(child.await.is_err());
    assert_eq!(gate.closed.load(Ordering::SeqCst), 1);
    second.stop().await.unwrap();
    runtime.stop().await.unwrap();
}
