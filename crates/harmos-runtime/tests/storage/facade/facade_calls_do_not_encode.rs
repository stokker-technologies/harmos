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
use super::support::{Model, Probe, payload};
use harmos_runtime::{Runtime, operations::CallContext};
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
async fn facade_calls_do_not_encode() {
    // Keep the Payload's Original Allocation Through Native Admission
    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
    let request = payload(0);
    let result = runtime.for_call(&CallContext::default()).call::<Probe>(request.clone()).await.unwrap();
    assert!(Arc::ptr_eq(&request.bytes, &result.0));
    assert!(request.memory_reads.load(Ordering::SeqCst) > 0);
    assert_eq!(request.encodes.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}
