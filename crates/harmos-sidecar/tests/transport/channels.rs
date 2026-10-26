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
use tokio::io::{AsyncReadExt, DuplexStream};
// Local Imports
use harmos_domain::Message;
use harmos_sidecar::Frame;
use harmos_sidecar::transport::{self, Connection, Io, Packet, Role};

use super::protocol::EchoOperation;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn io(stream: DuplexStream) -> Io {
    let (read, write) = tokio::io::split(stream);
    (Box::new(read), Box::new(write))
}

#[tokio::test]
async fn split_write_matches() {
    for length in [0, 1, 127, 128, 4096] {
        for frame in [
            Frame::Call { id: 128, method: "call".into(), params: vec![17; length].into() },
            Frame::Push { method: "push".into(), params: vec![17; length].into() },
            Frame::Answer { id: 128, result: vec![17; length].into() },
        ] {
            let packet = Packet::Frame(frame);
            let expected = packet.encode().unwrap();
            let mut actual = vec![0; expected.len()];
            let (mut writer, mut reader) = tokio::io::duplex(7);
            let (written, read) = tokio::join!(transport::write(&mut writer, &packet), reader.read_exact(&mut actual));
            written.unwrap();
            read.unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[tokio::test]
async fn buffered_exit_precedes() {
    let (data, mut peer_data) = tokio::io::duplex(1024);
    let (control, _peer_control) = tokio::io::duplex(64);
    let Connection { sender: _sender, mut receiver, mut driver } = Connection::new(io(data), io(control));
    let exit = Frame::Push { method: "exiting".to_owned(), params: b"recording complete".to_vec().into() };
    transport::write(&mut peer_data, &Packet::Frame(exit.clone())).await.unwrap();
    drop(peer_data);

    // Observe EOF Before Consuming The Already Buffered Protocol Frame.
    assert!(driver.run().await.is_err());
    let received = receiver.receive().await.unwrap().unwrap();
    assert_eq!(received.frame, exit);
    received.consumption.unwrap().consumed().unwrap();
    assert!(receiver.receive().await.is_err());
}

#[tokio::test]
async fn blocked_data_control() {
    let (data, mut peer_data) = tokio::io::duplex(8);
    let (control, mut peer_control) = tokio::io::duplex(64);
    let Connection { sender, receiver, mut driver } = Connection::new(io(data), io(control));
    let task = tokio::spawn(async move { driver.run().await });
    sender.send(Frame::Answer { id: 1, result: vec![1; 65536].into() }).await.unwrap();
    let mut first = [0];
    peer_data.read_exact(&mut first).await.unwrap();
    sender.send(Frame::Cancel { id: 99 }).await.unwrap();
    assert_eq!(transport::read(&mut peer_control, Role::Control).await.unwrap(), Some(Packet::Cancellation { id: 99, preceding: 1 }));
    sender.close();
    task.await.unwrap().unwrap();
    drop(receiver);
}

#[tokio::test]
async fn credit_requires_consumption() {
    let (data, mut peer_data) = tokio::io::duplex(1024);
    let (control, mut peer_control) = tokio::io::duplex(64);
    let Connection { sender, mut receiver, mut driver } = Connection::new(io(data), io(control));
    let task = tokio::spawn(async move { driver.run().await });
    let packet = Packet::Frame(Frame::Answer { id: 7, result: vec![42].into() });
    transport::write(&mut peer_data, &packet).await.unwrap();
    let received = receiver.receive().await.unwrap().unwrap();
    assert_eq!(received.frame, Frame::Answer { id: 7, result: vec![42].into() });
    // A Pending Control Read Proves Queueing Alone Has Not Returned Credit.
    let mut pending = Box::pin(transport::read(&mut peer_control, Role::Control));
    assert!(pending.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop())).is_pending());
    drop(received.frame);
    received.consumption.unwrap().consumed().unwrap();
    assert_eq!(pending.await.unwrap(), Some(Packet::Credit(4096)));
    transport::write(&mut peer_data, &packet).await.unwrap();
    let abandoned = receiver.receive().await.unwrap().unwrap();
    drop(abandoned);
    assert!(task.await.unwrap().is_err());
    assert!(sender.send(Frame::Cancel { id: 7 }).await.is_err());
}

#[tokio::test]
async fn cancelled_borrow_resumes() {
    let (data, peer_data) = tokio::io::duplex(8);
    let (control, peer_control) = tokio::io::duplex(64);
    let Connection { sender, receiver, mut driver } = Connection::new(io(data), io(control));
    sender.send(Frame::Answer { id: 5, result: vec![3; 128].into() }).await.unwrap();
    {
        let mut first_run = Box::pin(driver.run());
        assert!(first_run.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop())).is_pending());
    }
    let mut peer_data = peer_data;
    let (result, packet) = tokio::join!(driver.run(), async {
        let packet = transport::read(&mut peer_data, Role::Data).await.unwrap().unwrap();
        sender.close();
        packet
    });
    result.unwrap();
    assert_eq!(packet, Packet::Frame(Frame::Answer { id: 5, result: vec![3; 128].into() }));
    driver.run().await.unwrap();
    drop((receiver, peer_control));
}

#[tokio::test]
async fn consumed_credit_recycles() {
    let (left_data, right_data) = tokio::io::duplex(65536);
    let (left_control, right_control) = tokio::io::duplex(64);
    let Connection { sender: left, receiver: left_receiver, driver: mut left_driver } = Connection::new(io(left_data), io(left_control));
    let Connection { sender: right, receiver: mut right_receiver, driver: mut right_driver } =
        Connection::new(io(right_data), io(right_control));
    let left_task = tokio::spawn(async move { left_driver.run().await });
    let right_task = tokio::spawn(async move { right_driver.run().await });
    for id in 0..40 {
        left.send(Frame::Answer { id, result: vec![id as u8; 1024 * 1024].into() }).await.unwrap();
        let received = right_receiver.receive().await.unwrap().unwrap();
        let Frame::Answer { id: actual, result } = received.frame else {
            panic!("unexpected frame");
        };
        assert_eq!(actual, id);
        assert_eq!(result.len(), 1024 * 1024);
        assert!(result.iter().all(|byte| *byte == id as u8));
        drop(result);
        received.consumption.unwrap().consumed().unwrap();
    }
    left.close();
    right.close();
    left_task.await.unwrap().unwrap();
    right_task.await.unwrap().unwrap();
    drop((left_receiver, right_receiver));
}

#[tokio::test]
async fn unearned_credit_closes() {
    let (data, mut peer_data) = tokio::io::duplex(8);
    let (control, mut peer_control) = tokio::io::duplex(64);
    let Connection { sender, receiver, mut driver } = Connection::new(io(data), io(control));
    let task = tokio::spawn(async move { driver.run().await });
    transport::write(&mut peer_control, &Packet::Credit(1)).await.unwrap();
    assert!(task.await.unwrap().is_err());
    let mut byte = [0];
    assert_eq!(peer_data.read(&mut byte).await.unwrap(), 0);
    assert_eq!(peer_control.read(&mut byte).await.unwrap(), 0);
    assert!(sender.send(Frame::Cancel { id: 1 }).await.is_err());
    drop(receiver);
}

#[tokio::test]
async fn cancellation_queue_order() {
    let (data, mut peer_data) = tokio::io::duplex(1024);
    let (control, mut peer_control) = tokio::io::duplex(64);
    let Connection { sender, mut receiver, mut driver } = Connection::new(io(data), io(control));
    let task = tokio::spawn(async move { driver.run().await });
    transport::write(&mut peer_control, &Packet::Cancellation { id: 9, preceding: 2 }).await.unwrap();
    assert_eq!(receiver.receive().await.unwrap().unwrap().frame, Frame::Cancel { id: 9 });
    for (id, cancelled) in [(40, false), (9, true)] {
        transport::write(&mut peer_data, &Packet::Frame(Frame::Call { id, method: "test".into(), params: vec![].into() })).await.unwrap();
        let received = receiver.receive().await.unwrap().unwrap();
        assert_eq!(received.cancelled, cancelled);
        drop(received.frame);
        received.consumption.unwrap().consumed().unwrap();
    }
    // Late Cancellations Cannot Accumulate Tombstones After Their Watermark.
    for id in 0..300 {
        transport::write(&mut peer_control, &Packet::Cancellation { id, preceding: 2 }).await.unwrap();
        assert_eq!(receiver.receive().await.unwrap().unwrap().frame, Frame::Cancel { id });
    }
    sender.close();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn future_cancellations_refused() {
    let (data, _peer_data) = tokio::io::duplex(8);
    let (control, mut peer_control) = tokio::io::duplex(64);
    let Connection { sender, mut receiver, mut driver } = Connection::new(io(data), io(control));
    let task = tokio::spawn(async move { driver.run().await });
    for id in 0..256 {
        transport::write(&mut peer_control, &Packet::Cancellation { id, preceding: u64::MAX }).await.unwrap();
        assert!(receiver.receive().await.unwrap().is_some());
    }
    transport::write(&mut peer_control, &Packet::Cancellation { id: 256, preceding: u64::MAX }).await.unwrap();
    assert!(receiver.receive().await.is_err());
    assert!(task.await.unwrap().is_err());
    assert!(sender.send(Frame::Cancel { id: 1 }).await.is_err());
}

#[tokio::test]
async fn binary_sdk_lifecycle() {
    use super::protocol::{Echo, TransportPlugin};
    use harmos_sidecar::Message;
    use harmos_sidecar::operations::{
        OperationDeclaration, WireAnswer, WireCommand,
        lifecycle::{Initialize, InitializeRequest, Shutdown},
    };
    use std::num::NonZeroUsize;
    let (host_data, sdk_data) = tokio::io::duplex(65536);
    let (host_control, sdk_control) = tokio::io::duplex(64);
    let sdk = tokio::spawn(harmos_sidecar::serve_plugin_channels::<TransportPlugin>(
        NonZeroUsize::new(8).unwrap(),
        io(sdk_data),
        io(sdk_control),
    ));
    let Connection { sender, mut receiver, mut driver } = Connection::new(io(host_data), io(host_control));
    let driving = tokio::spawn(async move { driver.run().await });
    sender
        .send(Frame::Call { id: 0, method: Frame::REGISTER.to_owned(), params: "throughput_fixture".to_owned().encode().into() })
        .await
        .unwrap();
    let registration = receiver.receive().await.unwrap().unwrap();
    assert!(matches!(registration.frame, Frame::Answer { id: 0, .. }));
    registration.consumption.unwrap().consumed().unwrap();
    let initialization = InitializeRequest { config_json: b"null".to_vec(), generation_context: Vec::new() };
    binary_invoke::<Initialize>(&sender, &mut receiver, 1, initialization.encode()).await;
    let request = Echo { nonce: 7, payload: vec![42; 65536] };
    let result = binary_invoke::<EchoOperation>(&sender, &mut receiver, 2, request.encode()).await;
    assert_eq!(Echo::decode(&result).unwrap(), request);
    binary_invoke::<Shutdown>(&sender, &mut receiver, 3, Vec::new()).await;
    assert!(matches!(
        binary_phase(
            &sender,
            &mut receiver,
            WireCommand::Prepare {
                invocation: 4,
                operation: format!("throughput_fixture.{}", EchoOperation::ID),
                fingerprint: (harmos_domain::operations::Entry::of::<EchoOperation>().fingerprint)().unwrap().0,
                generation: 1,
                payload: request.encode()
            }
        )
        .await,
        WireAnswer::Failed { .. }
    ));
    sender.close();
    driving.await.unwrap().unwrap();
    // SDK Completion Proves Cleanup Joined; The Host Driver Owns Protocol Failure Reporting.
    sdk.await.unwrap().unwrap();
}

#[tokio::test]
async fn cancelled_call_skipped() {
    use super::protocol::TransportPlugin;
    use std::num::NonZeroUsize;
    let (mut peer_data, sdk_data) = tokio::io::duplex(65536);
    let (mut peer_control, sdk_control) = tokio::io::duplex(64);
    let mut sdk =
        Box::pin(harmos_sidecar::serve_plugin_channels::<TransportPlugin>(NonZeroUsize::new(8).unwrap(), io(sdk_data), io(sdk_control)));
    transport::write(&mut peer_control, &Packet::Cancellation { id: 77, preceding: 1 }).await.unwrap();
    // Drive Control While The Data Pipe Is Empty, Without Sleeps Or Wall-Clock Guesses.
    for _ in 0..2 {
        assert!(sdk.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop())).is_pending());
    }
    let task = tokio::spawn(sdk);
    for id in [77, 88] {
        transport::write(
            &mut peer_data,
            &Packet::Frame(Frame::Call { id, method: Frame::REGISTER.into(), params: "throughput_fixture".to_owned().encode().into() }),
        )
        .await
        .unwrap();
    }
    let Some(Packet::Frame(Frame::Answer { id: 88, .. })) = transport::read(&mut peer_data, Role::Data).await.unwrap() else {
        panic!("cancelled registration must not produce an answer");
    };
    drop((peer_data, peer_control));
    task.await.unwrap().unwrap();
}

async fn binary_phase(
    sender: &transport::Sender, receiver: &mut transport::Receiver, command: harmos_sidecar::operations::WireCommand,
) -> harmos_sidecar::operations::WireAnswer {
    use harmos_sidecar::operations::{WIRE_METHOD, WireAnswer};
    sender.send(Frame::Call { id: 1, method: WIRE_METHOD.into(), params: command.encode().unwrap().into() }).await.unwrap();
    let received = receiver.receive().await.unwrap().unwrap();
    let Frame::Answer { id: 1, result } = received.frame else {
        panic!("phase was not answered");
    };
    let answer = WireAnswer::decode(&result).unwrap();
    drop(result);
    received.consumption.unwrap().consumed().unwrap();
    answer
}

async fn binary_invoke<D: harmos_domain::operations::OperationDeclaration>(
    sender: &transport::Sender, receiver: &mut transport::Receiver, invocation: u64, payload: Vec<u8>,
) -> Vec<u8> {
    use harmos_sidecar::operations::{WireAnswer, WireCommand};
    for command in [
        WireCommand::Prepare {
            invocation,
            operation: format!("throughput_fixture.{}", D::ID),
            fingerprint: (harmos_domain::operations::Entry::of::<D>().fingerprint)().unwrap().0,
            generation: 1,
            payload,
        },
        WireCommand::Initialize { invocation, context: [0; 32] },
    ] {
        assert!(matches!(binary_phase(sender, receiver, command).await, WireAnswer::Ready { .. }));
    }
    let WireAnswer::Ready { payload } =
        binary_phase(sender, receiver, WireCommand::Attempt { invocation, context: [0; 32], values: Vec::new() }).await
    else {
        panic!("attempt failed");
    };
    assert!(matches!(binary_phase(sender, receiver, WireCommand::Finalize { invocation }).await, WireAnswer::Ready { .. }));
    payload
}

/* === Tests === */
