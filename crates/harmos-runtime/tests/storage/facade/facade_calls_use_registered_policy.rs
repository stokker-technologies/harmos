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
use super::support::{Model, Probe, Tag, payload};
use harmos_runtime::{Runtime, operations::CallContext};
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
async fn facade_calls_use_registered_policy() {
    // Start With a Registered Retrying Operation
    let runtime = Runtime::<Model>::start((), ()).await.unwrap();
    let request = payload(1);
    let mut context = CallContext::default();
    context.insert(Tag(37)).unwrap();

    // Retry a Transient Failure and Preserve the Caller Context
    let result = runtime.for_call(&context).call::<Probe>(request.clone()).await.unwrap();
    assert_eq!(request.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(result.1, Some(37));
    runtime.stop().await.unwrap();
}
