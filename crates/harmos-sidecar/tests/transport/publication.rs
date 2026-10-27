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
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use super::protocol::{Done, Sample, Start, TransportPlugin};
use harmos_domain::operations::lifecycle::{Initialize, InitializeRequest};
use harmos_sdk::operations::{CHILD_METHOD, WireChildCall};
use harmos_sidecar::operations::{OperationDeclaration, WIRE_METHOD, WireAnswer, WireCommand};
use harmos_sidecar::transport::{Connection, Receiver, Sender};
use harmos_sidecar::{Attach, Frame, Message, PluginNative, StreamSpec};

use super::protocol::StartOperation;

/* === Constants === */

const GENERATION: [u8; 32] = [2; 32];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// One `host.publish` Call As The Host Saw It.
#[derive(Debug, PartialEq)]
struct Call {
    first: u64,
    rows: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn receive(incoming: &mut Receiver) -> Frame {
    let received = incoming.receive().await.unwrap().unwrap();
    if let Some(consumption) = received.consumption {
        consumption.consumed().unwrap();
    }
    received.frame
}

async fn phase(outgoing: &Sender, incoming: &mut Receiver, id: u64, command: WireCommand) {
    outgoing.send(Frame::Call { id, method: WIRE_METHOD.to_owned(), params: command.encode().unwrap().into() }).await.unwrap();
    let Frame::Answer { id: answered, result } = receive(incoming).await else {
        panic!("the carrier returns one operation phase answer");
    };
    assert_eq!(answered, id);
    assert!(matches!(WireAnswer::decode(&result).unwrap(), WireAnswer::Ready { .. }));
}

async fn invoke<D: harmos_domain::operations::OperationDeclaration>(outgoing: &Sender, incoming: &mut Receiver, id: u64, payload: Vec<u8>) {
    let operation = format!("throughput_fixture.{}", D::ID);
    for command in [
        WireCommand::Prepare {
            invocation: id,
            operation,
            fingerprint: (harmos_domain::operations::Entry::of::<D>().fingerprint)().unwrap().0,
            generation: 1,
            payload,
        },
        WireCommand::Initialize { invocation: id, context: [1; 32] },
        WireCommand::Attempt { invocation: id, context: [1; 32], values: Vec::new() },
        WireCommand::Finalize { invocation: id },
    ] {
        phase(outgoing, incoming, id, command).await;
    }
}

// Run one producer case and answer its publications until it reports done.
async fn publish(outgoing: &Sender, incoming: &mut Receiver, id: u64, samples: u32, payload_bytes: u32, batch: u32) -> Vec<Call> {
    let start = Start { samples, payload_bytes, pace_bytes_per_second: 0, batch };
    for command in [
        WireCommand::Prepare {
            invocation: id,
            operation: format!("throughput_fixture.{}", StartOperation::ID),
            fingerprint: (harmos_domain::operations::Entry::of::<StartOperation>().fingerprint)().unwrap().0,
            generation: 1,
            payload: start.encode(),
        },
        WireCommand::Initialize { invocation: id, context: [1; 32] },
    ] {
        phase(outgoing, incoming, id, command).await;
    }
    let attempt = WireCommand::Attempt { invocation: id, context: [1; 32], values: Vec::new() };
    outgoing.send(Frame::Call { id, method: WIRE_METHOD.to_owned(), params: attempt.encode().unwrap().into() }).await.unwrap();
    // The Producer Publishes Concurrently With The Attempt Answer.
    let mut calls = Vec::new();
    let mut answered = false;
    let mut done = false;
    while !(answered && done) {
        let (call, method, params) = match receive(incoming).await {
            Frame::Answer { id: answered_id, .. } if answered_id == id => {
                answered = true;
                continue;
            }
            Frame::Call { id, method, params } => (id, method, params),
            other => panic!("unexpected frame {other:?}"),
        };
        assert_eq!(method, CHILD_METHOD);
        let child = WireChildCall::decode(&params).unwrap();
        assert_eq!(child.generation_context, Some(GENERATION));
        assert_eq!(child.operation, "host.host_publish");
        assert_eq!(child.generation, 1);
        assert_eq!(
            child.fingerprint,
            <harmos_operations::streams::Publish as harmos_domain::operations::OperationDeclaration>::definition().fingerprint.0
        );
        let publication = <harmos_operations::streams::PublishRequest as Message>::decode(&child.payload).unwrap();
        outgoing.send(Frame::Answer { id: call, result: <() as Message>::encode(&()).into() }).await.unwrap();
        if publication.stream == Done::ID {
            let finished = Done::decode(&publication.payload).unwrap();
            assert_eq!(finished.accepted, u64::from(samples));
            done = true;
            continue;
        }
        assert_eq!(publication.stream, Sample::ID);
        let rows = std::iter::once(&publication.payload).chain(publication.following.iter().map(|row| &row.payload));
        for payload in rows {
            assert_eq!(Sample::decode(payload).unwrap().payload.len(), payload_bytes as usize);
        }
        // Every Row Carries Its Own Production Stamp As A Forward Delta
        assert!(publication.published_unix_micros.is_some());
        assert!(publication.following.iter().all(|row| row.published_delta_micros.is_some_and(|delta| delta >= 0)));
        calls.push(Call { first: publication.sequence.unwrap(), rows: 1 + publication.following.len() });
    }
    phase(outgoing, incoming, id, WireCommand::Finalize { invocation: id }).await;
    calls
}

// Runs Inside The Transport SDK Scenario, Which Owns The Test Entry Point.
#[tokio::test]
async fn batches_split_contiguously() {
    let native = PluginNative::<TransportPlugin>::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let (write, read, control, _, _ending) = native.attach().await.unwrap().into_parts();
    let Connection { sender: outgoing, receiver: mut incoming, mut driver } = Connection::new((read, write), control.unwrap());
    let driving = tokio::spawn(async move { driver.run().await });
    outgoing
        .send(Frame::Call { id: 1, method: Frame::REGISTER.to_owned(), params: "throughput_fixture".to_owned().encode().into() })
        .await
        .unwrap();
    assert!(matches!(receive(&mut incoming).await, Frame::Answer { id: 1, .. }));
    let initialization = InitializeRequest {
        config_json: b"null".to_vec(),
        generation_context: GENERATION.to_vec(),
        assignments: harmos_sdk::operations::AssignedOperation::encode(&[harmos_sdk::operations::AssignedOperation {
            operation: "host.host_publish".into(),
            fingerprint: <harmos_operations::streams::Publish as harmos_domain::operations::OperationDeclaration>::definition()
                .fingerprint
                .0,
            generation: 1,
        }])
        .unwrap(),
    };
    invoke::<Initialize>(&outgoing, &mut incoming, 2, initialization.encode()).await;

    // Row By Row, Then Ordered Batches With A Short Tail
    let single = publish(&outgoing, &mut incoming, 3, 3, 64, 1).await;
    assert_eq!(single, (1..=3).map(|first| Call { first, rows: 1 }).collect::<Vec<_>>());
    let batched = publish(&outgoing, &mut incoming, 4, 250, 64, 100).await;
    assert_eq!(batched, vec![Call { first: 4, rows: 100 }, Call { first: 104, rows: 100 }, Call { first: 204, rows: 50 },]);

    // Rows That Fit One Frame But Not One Batch Budget Split
    let large = publish(&outgoing, &mut incoming, 5, 4, 600 * 1024, 4).await;
    assert_eq!(large, (254..=257).map(|first| Call { first, rows: 1 }).collect::<Vec<_>>());
    let medium = publish(&outgoing, &mut incoming, 6, 5, 300 * 1024, 5).await;
    assert_eq!(medium, vec![Call { first: 258, rows: 3 }, Call { first: 261, rows: 2 },]);
    drop(outgoing);
    drop(incoming);
    let _ = driving.await;
}

/* === Tests === */
