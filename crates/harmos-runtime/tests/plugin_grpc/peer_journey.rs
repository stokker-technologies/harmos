/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Host-Mediated Peer Journey Covers Scope, Failure and Cancellation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::model::Failure as WireFailure;

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{invoke_support::*, support};
use harmos_runtime::{
    operations::middleware::{Enter, Invocation, Middleware, Observation},
    plugin::grpc::Placement,
};
use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct PeerSettled(tokio::sync::mpsc::UnboundedSender<(bool, harmos_runtime::operations::OperationOutcome)>);
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Middleware for PeerSettled {
    fn enter<'a>(&'a self, _: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async { Ok(()) })
    }
    fn observe(&self, call: &Invocation, observation: Observation) {
        if call.operation().id.as_str() == "fixture.peer" {
            let _ = self.0.send((observation.settled, observation.outcome));
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn public_peer_inherits_scope_cause_and_caller_cancellation_through_the_host() {
    let host = HostFixture::new().await;
    let (send, mut observed) = tokio::sync::mpsc::unbounded_channel();
    host.runtime.operations.register_middleware(Arc::new(PeerSettled(send))).unwrap();
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();
    let peer = host.sdk("provider").await;
    let _peer = host.runtime.attach_grpc(Placement::Remote(peer.address.clone()), support::settings()).await.unwrap();
    let scoped = host.runtime.operations.with_scope("scope/opaque").unwrap();

    assert_eq!(scoped.call_encoded("fixture.peer_forward", payload(b"scope")).await.unwrap(), payload(b"scope/opaque"));
    assert!(observed.recv().await.unwrap().0);
    let error = scoped.call_encoded("fixture.peer_forward", payload(b"failure")).await.unwrap_err();
    let harmos_runtime::plugin::grpc::CallError::Remote(failure) = call_error(&error) else {
        panic!("Expected Peer Failure");
    };
    let failure = WireFailure::from(failure.as_ref());
    assert_eq!(failure.operation, "fixture.peer");
    assert_eq!(failure.cause.unwrap().code, "fixture_cause");
    assert!(observed.recv().await.unwrap().0);

    let invocation = scoped.submit_encoded("fixture.peer_forward", payload(b"wait")).unwrap();
    host.signals.entered.notified().await;
    invocation.request_cancel();
    assert_eq!(invocation.await.unwrap(), payload(b"wait"));
    let (settled, outcome) = observed.recv().await.unwrap();
    assert!(settled);
    assert_eq!(outcome, harmos_runtime::operations::OperationOutcome::Succeeded);
    host.runtime.shutdown().await.unwrap();
}
