/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Position, Receipt and Policy Journey

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;

// Local Imports
use harmos_protocol::model;
use harmos_protocol::policy;

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

#[test]
fn a_consumer_retains_composite_reads_receipts_failures_and_retry_policy() {
    // Retain Two Independent Histories And A Writable Revision
    let accepted = model::Position {
        history: Some(model::History {
            store: "store".into(),
            epoch: "epoch".into(),
        }),
        sequence: 7,
    };
    let other = model::Position {
        history: Some(model::History {
            store: "other".into(),
            epoch: "epoch".into(),
        }),
        sequence: 3,
    };
    let cut = model::ReadCut {
        positions: vec![accepted.clone(), other],
        revisions: vec![model::ResourceRevision {
            name: "definition".into(),
            revision: vec![1, 2],
        }],
    };
    assert_eq!(model::ReadCut::decode(cut.encode_to_vec().as_slice()).unwrap(), cut);

    // Acceptance Alone Does Not Claim Durability
    let mut receipt = model::Receipt {
        request_id: "mutation".into(),
        accepted_at: Some(accepted.clone()),
        ..Default::default()
    };
    let received = model::Receipt::decode(receipt.encode_to_vec().as_slice()).unwrap();
    assert_eq!(received.accepted_at, Some(accepted.clone()));
    assert!(received.durable_at.is_none());
    assert!(!received.completed);
    receipt.durable_at = Some(accepted);
    receipt.completed = true;
    assert_eq!(model::Receipt::decode(receipt.encode_to_vec().as_slice()).unwrap(), receipt);

    // Preserve A Structured Child Cause Independently Of Diagnostics
    let failure = model::Failure {
        classification: model::Classification::Transient as i32,
        code: "unavailable".into(),
        action: "Call again".into(),
        cause: Some(Box::new(model::Failure {
            code: "child_unavailable".into(),
            ..Default::default()
        })),
        diagnostics: "source".into(),
        ..Default::default()
    };
    assert_eq!(model::Failure::decode(failure.encode_to_vec().as_slice()).unwrap(), failure);

    // Apply A Call-Local Total-Attempt Budget With Exponential Backoff
    let wire = model::OperationPolicy {
        retries: Some(model::RetryPolicy {
            total_attempts: 3,
            initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            maximum: Some(prost_types::Duration { seconds: 4, nanos: 0 }),
            deterministic_jitter: false,
        }),
        ..Default::default()
    };
    let applied = policy::OperationPolicy::try_from(Some(&wire)).unwrap();
    assert_eq!(
        applied.retries.retry_after(0, policy::FailureKind::Transient, policy::ReplaySafety::Idempotent, 0),
        Some(std::time::Duration::from_secs(1))
    );
    assert_eq!(
        applied.retries.retry_after(1, policy::FailureKind::Transient, policy::ReplaySafety::Idempotent, 0),
        Some(std::time::Duration::from_secs(2))
    );
    assert_eq!(applied.retries.retry_after(2, policy::FailureKind::Transient, policy::ReplaySafety::Idempotent, 0), None);
    assert_eq!(applied.retries.retry_after(0, policy::FailureKind::Transient, policy::ReplaySafety::Unknown, 0), None);
    assert_eq!(policy::OperationPolicy::try_from(None).unwrap().retries, policy::RetryPolicy::Never);
}
