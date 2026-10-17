/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

#[path = "../../../../tests/support/fixtures.rs"]
mod prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports
use tokio::io::{AsyncReadExt, DuplexStream};
// Local Imports
use harmos_sidecar::Frame;
use harmos_sidecar::transport::{self, Connection, Io, Packet, Role};

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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("buffered_exit_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, mut peer_data) = tokio::io::duplex(1024);
                        let (control, _peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender: _sender,
                            mut receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let exit = Frame::Push {
                            method: "exiting".to_owned(),
                            params: b"recording complete".to_vec(),
                        };
                        transport::write(&mut peer_data, &Packet::Frame(exit.clone()))
                            .await
                            .unwrap();
                        drop(peer_data);

                        // Observe EOF Before Consuming The Already Buffered Protocol Frame.
                        assert!(driver.run().await.is_err());
                        let received = receiver.receive().await.unwrap().unwrap();
                        assert_eq!(received.frame, exit);
                        received.consumption.unwrap().consumed().unwrap();
                        assert!(receiver.receive().await.is_err());
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("blocked_data_control", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, mut peer_data) = tokio::io::duplex(8);
                        let (control, mut peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let task = tokio::spawn(async move { driver.run().await });
                        sender
                            .send(Frame::Answer {
                                id: 1,
                                result: vec![1; 65536],
                            })
                            .await
                            .unwrap();
                        let mut first = [0];
                        peer_data.read_exact(&mut first).await.unwrap();
                        sender.send(Frame::Cancel { id: 99 }).await.unwrap();
                        assert_eq!(
                            transport::read(&mut peer_control, Role::Control)
                                .await
                                .unwrap(),
                            Some(Packet::Cancellation {
                                id: 99,
                                preceding: 1
                            })
                        );
                        sender.close();
                        task.await.unwrap().unwrap();
                        drop(receiver);
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("credit_requires_consumption", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, mut peer_data) = tokio::io::duplex(1024);
                        let (control, mut peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            mut receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let task = tokio::spawn(async move { driver.run().await });
                        let packet = Packet::Frame(Frame::Answer {
                            id: 7,
                            result: vec![42],
                        });
                        transport::write(&mut peer_data, &packet).await.unwrap();
                        let received = receiver.receive().await.unwrap().unwrap();
                        assert_eq!(
                            received.frame,
                            Frame::Answer {
                                id: 7,
                                result: vec![42]
                            }
                        );
                        // A Pending Control Read Proves Queueing Alone Has Not Returned Credit.
                        let mut pending =
                            Box::pin(transport::read(&mut peer_control, Role::Control));
                        assert!(
                            pending
                                .as_mut()
                                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
                                .is_pending()
                        );
                        drop(received.frame);
                        received.consumption.unwrap().consumed().unwrap();
                        assert_eq!(pending.await.unwrap(), Some(Packet::Credit(4096)));
                        transport::write(&mut peer_data, &packet).await.unwrap();
                        let abandoned = receiver.receive().await.unwrap().unwrap();
                        drop(abandoned);
                        assert!(task.await.unwrap().is_err());
                        assert!(sender.send(Frame::Cancel { id: 7 }).await.is_err());
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_borrow_resumes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, peer_data) = tokio::io::duplex(8);
                        let (control, peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        sender
                            .send(Frame::Answer {
                                id: 5,
                                result: vec![3; 128],
                            })
                            .await
                            .unwrap();
                        {
                            let mut first_run = Box::pin(driver.run());
                            assert!(
                                first_run
                                    .as_mut()
                                    .poll(&mut std::task::Context::from_waker(
                                        std::task::Waker::noop()
                                    ))
                                    .is_pending()
                            );
                        }
                        let mut peer_data = peer_data;
                        let (result, packet) = tokio::join!(driver.run(), async {
                            let packet = transport::read(&mut peer_data, Role::Data)
                                .await
                                .unwrap()
                                .unwrap();
                            sender.close();
                            packet
                        });
                        result.unwrap();
                        assert_eq!(
                            packet,
                            Packet::Frame(Frame::Answer {
                                id: 5,
                                result: vec![3; 128]
                            })
                        );
                        driver.run().await.unwrap();
                        drop((receiver, peer_control));
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("credit_recycles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let (left_data, right_data) = tokio::io::duplex(65536);
                        let (left_control, right_control) = tokio::io::duplex(64);
                        let Connection {
                            sender: left,
                            receiver: left_receiver,
                            driver: mut left_driver,
                        } = Connection::new(io(left_data), io(left_control));
                        let Connection {
                            sender: right,
                            receiver: mut right_receiver,
                            driver: mut right_driver,
                        } = Connection::new(io(right_data), io(right_control));
                        let left_task = tokio::spawn(async move { left_driver.run().await });
                        let right_task = tokio::spawn(async move { right_driver.run().await });
                        for id in 0..40 {
                            left.send(Frame::Answer {
                                id,
                                result: vec![id as u8; 1024 * 1024],
                            })
                            .await
                            .unwrap();
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
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("unearned_credit_closes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, mut peer_data) = tokio::io::duplex(8);
                        let (control, mut peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let task = tokio::spawn(async move { driver.run().await });
                        transport::write(&mut peer_control, &Packet::Credit(1))
                            .await
                            .unwrap();
                        assert!(task.await.unwrap().is_err());
                        let mut byte = [0];
                        assert_eq!(peer_data.read(&mut byte).await.unwrap(), 0);
                        assert_eq!(peer_control.read(&mut byte).await.unwrap(), 0);
                        assert!(sender.send(Frame::Cancel { id: 1 }).await.is_err());
                        drop(receiver);
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_queue_order", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, mut peer_data) = tokio::io::duplex(1024);
                        let (control, mut peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            mut receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let task = tokio::spawn(async move { driver.run().await });
                        transport::write(
                            &mut peer_control,
                            &Packet::Cancellation {
                                id: 9,
                                preceding: 2,
                            },
                        )
                        .await
                        .unwrap();
                        assert_eq!(
                            receiver.receive().await.unwrap().unwrap().frame,
                            Frame::Cancel { id: 9 }
                        );
                        for (id, cancelled) in [(40, false), (9, true)] {
                            transport::write(
                                &mut peer_data,
                                &Packet::Frame(Frame::Call {
                                    id,
                                    method: "test".into(),
                                    params: vec![],
                                }),
                            )
                            .await
                            .unwrap();
                            let received = receiver.receive().await.unwrap().unwrap();
                            assert_eq!(received.cancelled, cancelled);
                            drop(received.frame);
                            received.consumption.unwrap().consumed().unwrap();
                        }
                        // Late Cancellations Cannot Accumulate Tombstones After Their Watermark.
                        for id in 0..300 {
                            transport::write(
                                &mut peer_control,
                                &Packet::Cancellation { id, preceding: 2 },
                            )
                            .await
                            .unwrap();
                            assert_eq!(
                                receiver.receive().await.unwrap().unwrap().frame,
                                Frame::Cancel { id }
                            );
                        }
                        sender.close();
                        task.await.unwrap().unwrap();
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("future_cancellations_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), async {
                        let (data, _peer_data) = tokio::io::duplex(8);
                        let (control, mut peer_control) = tokio::io::duplex(64);
                        let Connection {
                            sender,
                            mut receiver,
                            mut driver,
                        } = Connection::new(io(data), io(control));
                        let task = tokio::spawn(async move { driver.run().await });
                        for id in 0..256 {
                            transport::write(
                                &mut peer_control,
                                &Packet::Cancellation {
                                    id,
                                    preceding: u64::MAX,
                                },
                            )
                            .await
                            .unwrap();
                            assert!(receiver.receive().await.unwrap().is_some());
                        }
                        transport::write(
                            &mut peer_control,
                            &Packet::Cancellation {
                                id: 256,
                                preceding: u64::MAX,
                            },
                        )
                        .await
                        .unwrap();
                        assert!(receiver.receive().await.is_err());
                        assert!(task.await.unwrap().is_err());
                        assert!(sender.send(Frame::Cancel { id: 1 }).await.is_err());
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("binary_sdk_lifecycle", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use super::protocol::{Echo, TransportPlugin};
                    use harmos_sidecar::Message;
                    use harmos_sidecar::operations::{
                        OperationDeclaration, WireAnswer, WireCommand,
                        lifecycle::{Initialize, Shutdown},
                    };
                    use std::num::NonZeroUsize;
                    tokio::time::timeout(Duration::from_secs(3), async {
                        let (host_data, sdk_data) = tokio::io::duplex(65536);
                        let (host_control, sdk_control) = tokio::io::duplex(64);
                        let sdk =
                            tokio::spawn(harmos_sidecar::serve_plugin_channels::<TransportPlugin>(
                                NonZeroUsize::new(8).unwrap(),
                                io(sdk_data),
                                io(sdk_control),
                            ));
                        let Connection {
                            sender,
                            mut receiver,
                            mut driver,
                        } = Connection::new(io(host_data), io(host_control));
                        let driving = tokio::spawn(async move { driver.run().await });
                        let initialization = Initialize {
                            config_json: b"null".to_vec(),
                            generation_context: Vec::new(),
                        };
                        binary_invoke(
                            &sender,
                            &mut receiver,
                            1,
                            Initialize::<TransportPlugin>::ID,
                            initialization.encode_to_vec(),
                        )
                        .await;
                        let request = Echo {
                            nonce: 7,
                            payload: vec![42; 65536],
                        };
                        let result = binary_invoke(
                            &sender,
                            &mut receiver,
                            2,
                            Echo::ID,
                            request.encode_to_vec(),
                        )
                        .await;
                        assert_eq!(Echo::decode(&result).unwrap(), request);
                        binary_invoke(
                            &sender,
                            &mut receiver,
                            3,
                            Shutdown::<TransportPlugin>::ID,
                            Vec::new(),
                        )
                        .await;
                        assert!(matches!(
                            binary_phase(
                                &sender,
                                &mut receiver,
                                WireCommand::Prepare {
                                    invocation: 4,
                                    operation: Echo::ID.into(),
                                    payload: request.encode_to_vec()
                                }
                            )
                            .await,
                            WireAnswer::Failed { .. }
                        ));
                        sender.close();
                        driving.await.unwrap().unwrap();
                        // SDK Completion Proves Cleanup Joined; The Host Driver Owns Protocol Failure Reporting.
                        sdk.await.unwrap().unwrap();
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_call_skipped", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use super::protocol::TransportPlugin;
                    use std::num::NonZeroUsize;
                    tokio::time::timeout(Duration::from_secs(3), async {
                        let (mut peer_data, sdk_data) = tokio::io::duplex(65536);
                        let (mut peer_control, sdk_control) = tokio::io::duplex(64);
                        let mut sdk =
                            Box::pin(harmos_sidecar::serve_plugin_channels::<TransportPlugin>(
                                NonZeroUsize::new(8).unwrap(),
                                io(sdk_data),
                                io(sdk_control),
                            ));
                        transport::write(
                            &mut peer_control,
                            &Packet::Cancellation {
                                id: 77,
                                preceding: 1,
                            },
                        )
                        .await
                        .unwrap();
                        // Drive Control While The Data Pipe Is Empty, Without Sleeps Or Wall-Clock Guesses.
                        for _ in 0..2 {
                            assert!(
                                sdk.as_mut()
                                    .poll(&mut std::task::Context::from_waker(
                                        std::task::Waker::noop()
                                    ))
                                    .is_pending()
                            );
                        }
                        let task = tokio::spawn(sdk);
                        for id in [77, 88] {
                            transport::write(
                                &mut peer_data,
                                &Packet::Frame(Frame::Call {
                                    id,
                                    method: Frame::REGISTER.into(),
                                    params: Vec::new(),
                                }),
                            )
                            .await
                            .unwrap();
                        }
                        let Some(Packet::Frame(Frame::Answer { id: 88, .. })) =
                            transport::read(&mut peer_data, Role::Data).await.unwrap()
                        else {
                            panic!("cancelled registration must not produce an answer");
                        };
                        drop((peer_data, peer_control));
                        task.await.unwrap().unwrap();
                    })
                    .await
                    .unwrap();
                });
        });
    }
    #[cfg(any(unix, windows))]
    {
        scenario_runtime::check("independent_control_channel", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use super::protocol::{Echo, TransportPlugin};
                    use harmos_sidecar::operations::{
                        OperationDeclaration,
                        lifecycle::{Initialize, Shutdown},
                    };
                    use harmos_sidecar::{Attach, Executable, Message};
                    let executable = Executable::at(prepared_fixtures::artifact(
                        "harmos-sidecar-throughput-fixture",
                    ))
                    .unwrap();
                    let (write, read, control, diagnostics, ending) =
                        executable.attach().await.unwrap().into_parts();
                    let mut ending = ending.unwrap();
                    let Connection {
                        sender,
                        mut receiver,
                        mut driver,
                    } = Connection::new(
                        (read, write),
                        control.expect("executable supplies independent control"),
                    );
                    let driving = tokio::spawn(async move { driver.run().await });
                    let sending = sender.clone();
                    let outcome = tokio::spawn(async move {
                        tokio::time::timeout(Duration::from_secs(10), async {
                            let request = Initialize {
                                config_json: b"null".to_vec(),
                                generation_context: Vec::new(),
                            };
                            binary_invoke(
                                &sending,
                                &mut receiver,
                                1,
                                Initialize::<TransportPlugin>::ID,
                                request.encode_to_vec(),
                            )
                            .await;
                            let request = Echo {
                                nonce: 41,
                                payload: vec![123; 1024 * 1024],
                            };
                            let response = binary_invoke(
                                &sending,
                                &mut receiver,
                                2,
                                Echo::ID,
                                request.encode_to_vec(),
                            )
                            .await;
                            assert_eq!(Echo::decode(&response).unwrap(), request);
                            binary_invoke(
                                &sending,
                                &mut receiver,
                                3,
                                Shutdown::<TransportPlugin>::ID,
                                Vec::new(),
                            )
                            .await;
                        })
                        .await
                        .expect("child lifecycle completes within its budget");
                    })
                    .await;
                    sender.close();
                    let _ = driving.await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), ending.settle())
                        .await
                        .unwrap()
                        .unwrap();
                    drop(diagnostics);
                    outcome.unwrap();
                });
        });
    }
}

async fn binary_phase(
    sender: &transport::Sender,
    receiver: &mut transport::Receiver,
    command: harmos_sidecar::operations::WireCommand,
) -> harmos_sidecar::operations::WireAnswer {
    use harmos_sidecar::operations::{WIRE_METHOD, WireAnswer};
    sender
        .send(Frame::Call {
            id: 1,
            method: WIRE_METHOD.into(),
            params: command.encode().unwrap(),
        })
        .await
        .unwrap();
    let received = receiver.receive().await.unwrap().unwrap();
    let Frame::Answer { id: 1, result } = received.frame else {
        panic!("phase was not answered");
    };
    let answer = WireAnswer::decode(&result).unwrap();
    drop(result);
    received.consumption.unwrap().consumed().unwrap();
    answer
}

async fn binary_invoke(
    sender: &transport::Sender,
    receiver: &mut transport::Receiver,
    invocation: u64,
    operation: &str,
    payload: Vec<u8>,
) -> Vec<u8> {
    use harmos_sidecar::operations::{WireAnswer, WireCommand};
    for command in [
        WireCommand::Prepare {
            invocation,
            operation: operation.into(),
            payload,
        },
        WireCommand::Initialize {
            invocation,
            context: [0; 32],
        },
    ] {
        assert!(matches!(
            binary_phase(sender, receiver, command).await,
            WireAnswer::Ready { .. }
        ));
    }
    let WireAnswer::Ready { payload } = binary_phase(
        sender,
        receiver,
        WireCommand::Attempt {
            invocation,
            context: [0; 32],
            values: Vec::new(),
        },
    )
    .await
    else {
        panic!("attempt failed");
    };
    assert!(matches!(
        binary_phase(sender, receiver, WireCommand::Finalize { invocation }).await,
        WireAnswer::Ready { .. }
    ));
    payload
}

/* === Tests === */
