/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn retries_preserve_identity_backoff_and_child_settlement() {
    // Launch And Register The Parent And Child
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime
        .operations
        .register_operation(compute_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.operations.register_operation(parent_operation().unwrap(), crate::support::SERIAL).await.unwrap();

    // Retry The Original Invocation Three Times
    let mut invocation = runtime.operation(Request { failures: 2, ..request() }).start().await.unwrap();
    let id = invocation.id();
    let generation = invocation.generation();
    let response = (&mut invocation).await.unwrap();
    assert_eq!(invocation.id(), id);
    assert_eq!(invocation.generation(), generation);
    assert_eq!(invocation.status().attempt(), 2);
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Settled);
    assert_eq!(response.attempts, 3);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(*owner.seen.lock().unwrap(), vec![id; 3]);
    assert_eq!(owner.finished.load(Ordering::SeqCst), 3);
    let times = owner.times.lock().unwrap().clone();
    assert_eq!(times[1] - times[0], Duration::from_millis(1));
    assert_eq!(times[2] - times[1], Duration::from_millis(1));

    // Retry A Child Without Repeating The Parent
    let child = runtime.operation(Parent(Request { failures: 2, ..request() })).await.unwrap();
    assert_eq!(child.attempts, 3);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 6);
    assert_eq!(owner.finished.load(Ordering::SeqCst), 6);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
    let seen = owner.seen.lock().unwrap().clone();
    assert!(seen[3..].iter().all(|child| *child == seen[3]));
    assert_ne!(seen[3], id);

    // Stop
    crate::support::app::shutdown(&runtime).await.unwrap();
}
