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
use super::support::{Forward, Model, POLICY, Probe, Tag, payload};
use harmos_runtime::{CallOptions, OperationPolicy, RetryPolicy, Runtime, operations::CallContext};
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
async fn facade_override_applies_to_one_call() {
    // Override One Call's Retry Policy
    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
    let request = payload(1);
    let mut context = CallContext::default();
    context
        .insert(CallOptions {
            policy: Some(OperationPolicy { retries: RetryPolicy::Never, ..POLICY }),
        })
        .unwrap();
    assert!(runtime.for_call(&context).call::<Probe>(request.clone()).await.is_err());
    assert_eq!(request.attempts.load(Ordering::SeqCst), 1);

    // Children Keep Their Own Policy and Caller Metadata
    let request = payload(1);
    context.insert(Tag(19)).unwrap();
    let result = runtime.for_call(&context).call::<Forward>(request.clone()).await.unwrap();
    assert_eq!(request.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(result.1, Some(19));
    assert!(context.get::<CallOptions>().is_some());

    // The Next Caller Still Uses the Registered Policy
    let request = payload(1);
    runtime.for_call(&CallContext::default()).call::<Probe>(request.clone()).await.unwrap();
    assert_eq!(request.attempts.load(Ordering::SeqCst), 2);
    runtime.shutdown().await.unwrap();
}
