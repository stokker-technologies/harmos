/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::protocol::{Done, Sample, StartOperation, TransportPlugin};
use harmos_operations::lifecycle::{Initialize, Shutdown};
use harmos_sdk::operations::{CHILD_METHOD, WireChildCall};
use harmos_sidecar::operations::{OperationDeclaration, WIRE_METHOD, WireAnswer, WireCommand};
use harmos_sidecar::transport::{self, Connection, Packet, Receiver, Role, Sender};
use harmos_sidecar::{Attach, Frame, Message, PluginNative, StreamSpec};

const GENERATION: [u8; 32] = [2; 32];
const CONCURRENT_CALLS: usize = 8;
const CONTROL_BUFFER: usize = 1024;

// One `host.publish` Call As The Host Saw It.
#[derive(Debug, PartialEq)]
struct Call {
    first: u64,
    rows: usize,
}

// Release The Whole Cancellation Burst Together, Independently Of Task Scheduling.
async fn batch_cancellations(
    mut input: impl AsyncRead + Unpin, mut output: impl AsyncWrite + Unpin, batching: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let mut packets = Vec::new();
    let mut count = 0;
    while let Some(packet) = transport::read(&mut input, Role::Control).await? {
        if batching.load(Ordering::Acquire) && matches!(packet, Packet::Cancellation { .. }) {
            packets.extend(packet.encode()?);
            count += 1;
            if count < CONCURRENT_CALLS {
                continue;
            }
            output.write_all(&packets).await?;
            packets.clear();
            batching.store(false, Ordering::Release);
            continue;
        }
        transport::write(&mut output, &packet).await?;
    }
    assert!(packets.is_empty());
    Ok(())
}

async fn receive(incoming: &mut Receiver) -> Frame {
    let received = incoming.receive().await.unwrap().unwrap();
    if let Some(consumption) = received.consumption {
        consumption.consumed().unwrap();
    }
    received.frame
}

async fn phase(outgoing: &Sender, incoming: &mut Receiver, id: u64, command: WireCommand) {
    outgoing
        .send(Frame::Call {
            id,
            method: WIRE_METHOD.to_owned(),
            params: command.encode().unwrap().into(),
        })
        .await
        .unwrap();
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
        WireCommand::Attempt {
            invocation: id,
            context: [1; 32],
            values: Vec::new(),
        },
        WireCommand::Finalize { invocation: id },
    ] {
        phase(outgoing, incoming, id, command).await;
    }
}

// Run one producer case and answer its publications until it reports done.
async fn publish(outgoing: &Sender, incoming: &mut Receiver, id: u64, samples: u32, payload_bytes: u32, batch: u32) -> Vec<Call> {
    let start = StartOperation {
        samples,
        payload_bytes,
        pace_bytes_per_second: 0,
        batch,
    };
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
    let attempt = WireCommand::Attempt {
        invocation: id,
        context: [1; 32],
        values: Vec::new(),
    };
    outgoing
        .send(Frame::Call {
            id,
            method: WIRE_METHOD.to_owned(),
            params: attempt.encode().unwrap().into(),
        })
        .await
        .unwrap();
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
        assert_eq!(child.operation, "host.host.publish");
        assert_eq!(child.generation, 1);
        assert_eq!(
            child.fingerprint,
            <harmos_operations::streams::Publish as harmos_domain::operations::OperationDeclaration>::definition().fingerprint.0
        );
        let publication = <harmos_operations::streams::Publish as Message>::decode(&child.payload).unwrap();
        outgoing
            .send(Frame::Answer {
                id: call,
                result: <() as Message>::encode(&()).into(),
            })
            .await
            .unwrap();
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
        calls.push(Call {
            first: publication.sequence.unwrap(),
            rows: 1 + publication.following.len(),
        });
    }
    phase(outgoing, incoming, id, WireCommand::Finalize { invocation: id }).await;
    calls
}

// Runs Inside The Transport SDK Scenario, Which Owns The Test Entry Point.
#[tokio::test]
async fn batches_split_contiguously() {
    let native = PluginNative::<TransportPlugin>::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let (write, read, control, _, _ending) = native.attach().await.unwrap().into_parts();
    let Connection {
        sender: outgoing,
        receiver: mut incoming,
        mut driver,
    } = Connection::new((read, write), control.unwrap());
    let driving = tokio::spawn(async move { driver.run().await });
    outgoing
        .send(Frame::Call {
            id: 1,
            method: Frame::REGISTER.to_owned(),
            params: "throughput_fixture".to_owned().encode().into(),
        })
        .await
        .unwrap();
    assert!(matches!(receive(&mut incoming).await, Frame::Answer { id: 1, .. }));
    let initialization = Initialize {
        config_json: b"null".to_vec(),
        generation_context: GENERATION.to_vec(),
        assignments: harmos_sdk::operations::AssignedOperation::encode(&[harmos_sdk::operations::AssignedOperation {
            operation: "host.host.publish".into(),
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

#[tokio::test]
async fn cancellation_burst_settles() {
    let native = PluginNative::<TransportPlugin>::new(NonZeroUsize::new(CONCURRENT_CALLS).unwrap()).unwrap();
    let (write, read, control, _, mut ending) = native.attach().await.unwrap().into_parts();
    let (control_read, control_write) = control.unwrap();
    let (staging, staged) = tokio::io::duplex(CONTROL_BUFFER);
    let batching = Arc::new(AtomicBool::new(false));
    let forwarding = tokio::spawn(batch_cancellations(control_read, staging, batching.clone()));
    let Connection {
        sender: outgoing,
        receiver: mut incoming,
        mut driver,
    } = Connection::new((read, write), (Box::new(staged), control_write));
    let driving = tokio::spawn(async move { driver.run().await });
    outgoing
        .send(Frame::Call {
            id: 1,
            method: Frame::REGISTER.to_owned(),
            params: "throughput_fixture".to_owned().encode().into(),
        })
        .await
        .unwrap();
    assert!(matches!(receive(&mut incoming).await, Frame::Answer { id: 1, .. }));
    let initialization = Initialize {
        config_json: b"null".to_vec(),
        generation_context: GENERATION.to_vec(),
        assignments: harmos_sdk::operations::AssignedOperation::encode(&[harmos_sdk::operations::AssignedOperation {
            operation: "host.host.publish".into(),
            fingerprint: <harmos_operations::streams::Publish as OperationDeclaration>::definition().fingerprint.0,
            generation: 1,
        }])
        .unwrap(),
    };
    invoke::<Initialize>(&outgoing, &mut incoming, 2, initialization.encode()).await;

    // Prepare Every Producer Before Any Can Issue Its Publication Call.
    let invocations = 3..3 + CONCURRENT_CALLS as u64;
    let start = StartOperation {
        samples: 1,
        payload_bytes: 1,
        pace_bytes_per_second: 0,
        batch: 1,
    };
    for id in invocations.clone() {
        for command in [
            WireCommand::Prepare {
                invocation: id,
                operation: format!("throughput_fixture.{}", StartOperation::ID),
                fingerprint: <StartOperation as OperationDeclaration>::definition().fingerprint.0,
                generation: 1,
                payload: start.encode(),
            },
            WireCommand::Initialize { invocation: id, context: [1; 32] },
        ] {
            phase(&outgoing, &mut incoming, id, command).await;
        }
    }
    for id in invocations.clone() {
        let command = WireCommand::Attempt {
            invocation: id,
            context: [1; 32],
            values: Vec::new(),
        };
        outgoing
            .send(Frame::Call {
                id,
                method: WIRE_METHOD.into(),
                params: command.encode().unwrap().into(),
            })
            .await
            .unwrap();
    }
    let mut pending = BTreeSet::new();
    let mut answered = BTreeSet::new();
    while pending.len() < CONCURRENT_CALLS || answered.len() < CONCURRENT_CALLS {
        match receive(&mut incoming).await {
            Frame::Call { id, method, .. } => {
                assert_eq!(method, CHILD_METHOD);
                assert!(pending.insert(id));
            }
            Frame::Answer { id, result } => {
                assert!(invocations.contains(&id));
                assert!(matches!(WireAnswer::decode(&result).unwrap(), WireAnswer::Ready { .. }));
                assert!(answered.insert(id));
            }
            other => panic!("unexpected producer frame {other:?}"),
        }
    }
    for id in invocations.clone() {
        phase(&outgoing, &mut incoming, id, WireCommand::Finalize { invocation: id }).await;
    }

    // Shutdown Joins The Producers And Cancels All Their Unanswered Calls.
    let shutdown = invocations.end;
    for command in [
        WireCommand::Prepare {
            invocation: shutdown,
            operation: format!("throughput_fixture.{}", Shutdown::ID),
            fingerprint: <Shutdown as OperationDeclaration>::definition().fingerprint.0,
            generation: 1,
            payload: Shutdown {}.encode(),
        },
        WireCommand::Initialize { invocation: shutdown, context: [1; 32] },
    ] {
        phase(&outgoing, &mut incoming, shutdown, command).await;
    }
    batching.store(true, Ordering::Release);
    let command = WireCommand::Attempt {
        invocation: shutdown,
        context: [1; 32],
        values: Vec::new(),
    };
    outgoing
        .send(Frame::Call {
            id: shutdown,
            method: WIRE_METHOD.into(),
            params: command.encode().unwrap().into(),
        })
        .await
        .unwrap();
    let mut settled = false;
    while !pending.is_empty() || !settled {
        match receive(&mut incoming).await {
            Frame::Cancel { id } => assert!(pending.remove(&id), "each outstanding call is cancelled exactly once"),
            Frame::Answer { id, result } => {
                assert_eq!(id, shutdown);
                assert!(matches!(WireAnswer::decode(&result).unwrap(), WireAnswer::Ready { .. }));
                assert!(!settled);
                settled = true;
            }
            other => panic!("unexpected shutdown frame {other:?}"),
        }
    }
    // A Subsequent Phase Answer Proves The Same Connection Remains Usable.
    phase(&outgoing, &mut incoming, shutdown, WireCommand::Finalize { invocation: shutdown }).await;
    outgoing.close();
    driving.await.unwrap().unwrap();
    ending.as_mut().unwrap().settle().await.unwrap();
    forwarding.await.unwrap().unwrap();
}
