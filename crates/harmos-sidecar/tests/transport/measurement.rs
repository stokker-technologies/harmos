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

mod tests {
    use std::time::Duration;

    use super::super::capacity::*;
    fn settings(values: &[(&str, &str)]) -> Result<Settings, String> {
        Settings::parse(|name| {
            Ok(values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned()))
        })
    }

    #[test]
    fn contract_scenario() {
        {
            super::scenario_runtime::check("sorted_default_rates", || {
                let defaults = settings(&[]).unwrap();
                assert_eq!(defaults.sizes, [16384, 65536]);
                assert_eq!(defaults.repetitions, 3);
                assert_eq!(
                    settings(&[("HARMOS_BENCH_RATES", "700,100")])
                        .unwrap()
                        .rates,
                    [100, 700]
                );
            });
        }
        {
            super::scenario_runtime::check("invalid_settings_named", || {
                for (key, value) in [
                    ("HARMOS_BENCH_SIZES", "0"),
                    ("HARMOS_BENCH_SIZES", "1048577"),
                    ("HARMOS_BENCH_RATES", "1,1"),
                    ("HARMOS_BENCH_RATES", ""),
                    ("HARMOS_BENCH_SECONDS", "0"),
                    ("HARMOS_BENCH_SECONDS", "1.5"),
                    ("HARMOS_BENCH_SECONDS", "18446744073709551616"),
                    ("HARMOS_BENCH_REPETITIONS", "11"),
                    ("HARMOS_BENCH_BURST_MIB", "0"),
                ] {
                    assert!(settings(&[(key, value)]).unwrap_err().contains(key));
                }
                assert!(settings(&[("HARMOS_BENCH_SIZES", "1")]).is_err());
                assert!(
                    settings(&[
                        ("HARMOS_BENCH_RATES", "4096"),
                        ("HARMOS_BENCH_SECONDS", "30")
                    ])
                    .is_err()
                );
                assert!(
                    settings(&[
                        ("HARMOS_BENCH_SECONDS", "30"),
                        ("HARMOS_BENCH_REPETITIONS", "10")
                    ])
                    .is_err()
                );
            });
        }
        {
            super::scenario_runtime::check("bounded_aggregate_work", || {
                let error = settings(&[
                    (
                        "HARMOS_BENCH_SIZES",
                        "16384,32768,65536,131072,262144,524288,786432,1048576",
                    ),
                    ("HARMOS_BENCH_RATES", "1,2,3,4,5,6,7,8"),
                    ("HARMOS_BENCH_REPETITIONS", "10"),
                ])
                .unwrap_err();
                assert!(error.contains("256"));
                let error = settings(&[
                    ("HARMOS_BENCH_RATES", "1000,2000"),
                    ("HARMOS_BENCH_SECONDS", "4"),
                    ("HARMOS_BENCH_REPETITIONS", "8"),
                ])
                .unwrap_err();
                assert!(error.contains("96 GiB"));
                let defaults = settings(&[]).unwrap();
                assert!(
                    defaults.timeout(defaults.case(16384, 700, 256).unwrap())
                        >= Duration::from_secs(18)
                );
            });
        }
        {
            super::scenario_runtime::check("pacing_requires_rate", || {
                let good = Observation {
                    useful_mib_s: 98.0,
                    encoded_mib_s: 99.0,
                    samples_s: 1.0,
                    elapsed_s: 1.0,
                    rejected: 0,
                };
                assert!(good.keeps_pace(100));
                assert!(!good.keeps_pace(0));
                assert!(
                    !Observation {
                        useful_mib_s: 97.9,
                        ..good
                    }
                    .keeps_pace(100)
                );
                assert!(
                    !Observation {
                        rejected: 1,
                        ..good
                    }
                    .keeps_pace(100)
                );
                assert_eq!(
                    highest_met(&[(100, vec![good]), (50, vec![good]), (200, vec![good])]),
                    Some(100)
                );
                assert_eq!(
                    highest_met(&[(
                        100,
                        vec![
                            good,
                            Observation {
                                rejected: 1,
                                ..good
                            }
                        ]
                    )]),
                    None
                );
                assert_eq!(range([3.0, 1.0, 2.0].into_iter()), "2.000 [1.000, 3.000]");
            });
        }
    }
}

mod sdk {
    use super::super::protocol::{Echo, TransportPlugin};
    use harmos_domain::operations::lifecycle::{Initialize, Shutdown};
    use harmos_sidecar::operations::{OperationDeclaration, WIRE_METHOD, WireAnswer, WireCommand};
    use harmos_sidecar::transport::{Connection, Receiver, Sender};
    use harmos_sidecar::{Attach, Frame, Message, PluginCatalog, PluginNative};
    use std::num::NonZeroUsize;

    async fn phase(
        outgoing: &Sender,
        incoming: &mut Receiver,
        id: u64,
        command: WireCommand,
    ) -> WireAnswer {
        outgoing
            .send(Frame::Call {
                id,
                method: WIRE_METHOD.to_owned(),
                params: command.encode().unwrap(),
            })
            .await
            .unwrap();
        let received = incoming.receive().await.unwrap().unwrap();
        let Frame::Answer {
            id: answered,
            result,
        } = received.frame
        else {
            panic!("the carrier returns one operation phase answer");
        };
        assert_eq!(answered, id);
        let answer = WireAnswer::decode(&result).unwrap();
        drop(result);
        received.consumption.unwrap().consumed().unwrap();
        answer
    }

    async fn invoke(
        outgoing: &Sender,
        incoming: &mut Receiver,
        id: u64,
        operation: &str,
        payload: Vec<u8>,
    ) -> Vec<u8> {
        assert!(matches!(
            phase(
                outgoing,
                incoming,
                id,
                WireCommand::Prepare {
                    invocation: id,
                    operation: operation.to_owned(),
                    payload,
                }
            )
            .await,
            WireAnswer::Ready { .. }
        ));
        assert!(matches!(
            phase(
                outgoing,
                incoming,
                id,
                WireCommand::Initialize {
                    invocation: id,
                    context: [1; 32]
                }
            )
            .await,
            WireAnswer::Ready { .. }
        ));
        let WireAnswer::Ready { payload } = phase(
            outgoing,
            incoming,
            id,
            WireCommand::Attempt {
                invocation: id,
                context: [1; 32],
                values: Vec::new(),
            },
        )
        .await
        else {
            panic!("the retained original operation succeeds");
        };
        assert!(matches!(
            phase(
                outgoing,
                incoming,
                id,
                WireCommand::Finalize { invocation: id }
            )
            .await,
            WireAnswer::Ready { .. }
        ));
        payload
    }

    #[test]
    fn contract_scenario() {
        {
            super::scenario_runtime::check("sdk_binding_lifecycle", || {
                let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
                scenario_executor
                    .enable_all()
                    .build()
                    .expect("scenario runtime is valid")
                    .block_on(async {
                        let native =
                            PluginNative::<TransportPlugin>::new(NonZeroUsize::new(8).unwrap())
                                .unwrap();
                        assert_eq!(native.sdk_lifecycle().unwrap().unwrap().len(), 2);
                        let (write, read, control, _, ending) =
                            native.attach().await.unwrap().into_parts();
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
                                params: Vec::new(),
                            })
                            .await
                            .unwrap();
                        let received = incoming.receive().await.unwrap().unwrap();
                        let Frame::Answer { id: 1, result } = received.frame else {
                            panic!("registration returns inert catalog");
                        };
                        let catalog: PluginCatalog = serde_json::from_slice(&result).unwrap();
                        drop(result);
                        received.consumption.unwrap().consumed().unwrap();
                        assert_eq!(catalog.operations, native.operations().unwrap());
                        assert_eq!(catalog.operations.len(), 2);
                        let request = Echo {
                            nonce: 7,
                            payload: b"original-handler".to_vec(),
                        };
                        assert!(matches!(
                            phase(
                                &outgoing,
                                &mut incoming,
                                2,
                                WireCommand::Prepare {
                                    invocation: 2,
                                    operation: Echo::ID.to_owned(),
                                    payload: request.encode_to_vec(),
                                }
                            )
                            .await,
                            WireAnswer::Failed { .. }
                        ));
                        let initialization = Initialize {
                            config_json: b"null".to_vec(),
                            generation_context: Vec::new(),
                        };
                        invoke(
                            &outgoing,
                            &mut incoming,
                            3,
                            Initialize::ID,
                            initialization.encode_to_vec(),
                        )
                        .await;
                        let result = invoke(
                            &outgoing,
                            &mut incoming,
                            4,
                            Echo::ID,
                            request.encode_to_vec(),
                        )
                        .await;
                        assert_eq!(Echo::decode(result.as_slice()).unwrap(), request);
                        invoke(&outgoing, &mut incoming, 5, Shutdown::ID, Vec::new()).await;
                        assert!(matches!(
                            phase(
                                &outgoing,
                                &mut incoming,
                                6,
                                WireCommand::Prepare {
                                    invocation: 6,
                                    operation: Echo::ID.to_owned(),
                                    payload: request.encode_to_vec(),
                                }
                            )
                            .await,
                            WireAnswer::Failed { .. }
                        ));
                        let mut ending = ending.unwrap();
                        ending.settle().await.unwrap();
                        assert!(driving.await.unwrap().is_err());
                    });
            });
        }
        super::super::publication::contract_scenario();
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

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
