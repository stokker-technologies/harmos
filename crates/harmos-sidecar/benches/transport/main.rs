/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../tests/transport/capacity.rs"]
mod capacity;

#[path = "../../tests/fixtures/throughput_protocol.rs"]
mod protocol;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
// Third Party Imports
use tokio::io::{duplex, split};
// Local Imports
use capacity::{Case, MIB, Observation, Settings, WARMUP_MIB};
use harmos_domain::operations::lifecycle::{Initialize, Shutdown};
use harmos_sidecar::Message;
use harmos_sidecar::operations::{OperationDeclaration, WIRE_METHOD, WireAnswer, WireCommand};
use harmos_sidecar::transport;
use harmos_sidecar::{Attach, Executable, Frame, serve_plugin_channels};
use harmos_sidecar::{Publication, StreamSpec};
use protocol::{Done, Echo, Sample, Start, TransportPlugin};

/* === Constants === */
const LATENCY_SAMPLES: u64 = 200;
const BATCH_ROWS: u32 = 100;
const CASES: [Case; 4] = [
    Case {
        name: "tiny burst",
        samples: 32 * 1024,
        payload_bytes: 64,
        pace_bytes_per_second: 0,
    },
    Case {
        name: "1 KiB burst",
        samples: 16 * 1024,
        payload_bytes: 1024,
        pace_bytes_per_second: 0,
    },
    Case {
        name: "64 KiB burst",
        samples: 512,
        payload_bytes: 64 * 1024,
        pace_bytes_per_second: 0,
    },
    Case {
        name: "sustained 10 MiB/s",
        samples: 320,
        payload_bytes: 64 * 1024,
        pace_bytes_per_second: (10 * MIB) as u64,
    },
];

/* === Macros === */
criterion::criterion_group!(benches, benchmarks);

criterion::criterion_main!(benches);

/* === Type Aliases === */
type Sender = transport::Sender;

/* === Structs === */
struct Receiver(transport::Receiver);
#[derive(Default)]
struct Reception {
    decode: bool,
    sequence: u64,
}

/* === Enums === */
/* === Traits === */

/* === Implementations === */
impl Receiver {
    async fn receive(&mut self) -> std::io::Result<Option<Frame>> {
        let Some(received) = self.0.receive().await? else {
            return Ok(None);
        };
        if let Some(consumption) = received.consumption {
            consumption.consumed()?;
        }
        Ok(Some(received.frame))
    }
}

impl Reception {
    fn accept(&mut self, publication: &Publication, payload_bytes: u32) {
        assert_eq!(publication.stream, Sample::ID);
        assert_eq!(publication.schema, Sample::VERSION);
        assert!(
            publication.sequence > self.sequence,
            "publication sequence repeated or moved backward"
        );
        self.sequence = publication.sequence;
        if self.decode {
            let row = Sample::decode(&publication.payload).expect("sample decodes as Postcard");
            assert_eq!(row.payload.len(), payload_bytes as usize);
            assert!(
                row.payload.iter().all(|byte| *byte == b'x'),
                "sample payload pattern changed"
            );
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn in_memory_transport_profile() {
    fn io(stream: tokio::io::DuplexStream) -> transport::Io {
        let (read, write) = split(stream);
        (Box::new(read), Box::new(write))
    }
    let (client_data, server_data) = duplex(32 * MIB);
    let (client_control, server_control) = duplex(64 * 1024);
    let serving = tokio::spawn(serve_plugin_channels::<TransportPlugin>(
        NonZeroUsize::new(8).unwrap(),
        io(server_data),
        io(server_control),
    ));
    let transport::Connection {
        sender,
        receiver,
        mut driver,
    } = transport::Connection::new(io(client_data), io(client_control));
    let driving = tokio::spawn(async move { driver.run().await });
    measure(
        "Postcard in-memory data and control",
        sender,
        Receiver(receiver),
    )
    .await;
    let _ = driving.await.expect("original memory driver joins");
    serving
        .await
        .expect("benchmark server task joins")
        .expect("benchmark server stops cleanly");
}

async fn child_process_transport_profile() {
    child(None).await;
}

async fn child_process_capacity() {
    let settings =
        Settings::load().unwrap_or_else(|error| panic!("invalid capacity settings: {error}"));
    child(Some(settings)).await;
}

async fn child(settings: Option<Settings>) {
    let executable = Executable::at(env!("CARGO_BIN_EXE_harmos-sidecar-throughput-fixture"))
        .expect("cargo built the absolute benchmark fixture");
    let attached = executable
        .attach()
        .await
        .expect("the benchmark child starts");
    let (write, read, control, _diagnostics, ending) = attached.into_parts();
    let control = control.expect("native benchmark requires its independent control channel");
    let transport::Connection {
        sender,
        receiver,
        mut driver,
    } = transport::Connection::new((read, write), control);
    let driving = tokio::spawn(async move { driver.run().await });

    // A task captures assertion panics as well as timeout failures, so the
    // held child ending is always invoked before the test reports failure.
    let outcome = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(1800), async move {
            let outgoing = sender;
            let incoming = Receiver(receiver);
            if let Some(settings) = settings {
                capacity_profile(settings, outgoing, incoming).await;
            } else {
                measure(
                    "Postcard child with independent control",
                    outgoing,
                    incoming,
                )
                .await;
            }
        })
        .await
        .expect("benchmark exceeded the 30-minute whole-run limit");
    })
    .await;
    let _ = driving.await.expect("original child driver joins");
    if let Some(mut end) = ending {
        tokio::time::timeout(Duration::from_secs(5), end.settle())
            .await
            .expect("the benchmark child cleanup timed out")
            .expect("the benchmark child physically ends");
    }
    outcome.expect("the benchmark failed; its child has been stopped");
}

async fn capacity_profile(settings: Settings, outgoing: Sender, mut incoming: Receiver) {
    eprintln!(
        "CAPACITY real child stdio; current-thread Tokio runtimes; Postcard decode + full payload pattern validation; settings={settings:?}"
    );
    call(&outgoing, &mut incoming, 1, Frame::REGISTER, Vec::new()).await;
    sdk(&outgoing, &mut incoming, 2, true).await;
    let mut id = 2;
    let mut reception = Reception {
        decode: true,
        sequence: 0,
    };
    for &size in &settings.sizes {
        let warmup = settings
            .case(size, 0, WARMUP_MIB)
            .expect("validated warmup");
        id += 1;
        eprintln!(
            "CAPACITY warmup: {WARMUP_MIB} MiB at {size} bytes/sample; excluded from summary"
        );
        timed_case(
            &settings,
            warmup,
            id,
            &outgoing,
            &mut incoming,
            &mut reception,
        )
        .await;
        let mut cases = Vec::new();
        for rate in settings.targets() {
            let case = settings
                .case(size, rate, settings.burst_mib)
                .expect("validated case");
            let mut trials = Vec::new();
            for repetition in 1..=settings.repetitions {
                id += 1;
                eprintln!(
                    "CAPACITY repetition {repetition}/{}; size={size} target={rate} MiB/s (0=uncapped)",
                    settings.repetitions
                );
                let observation = timed_case(
                    &settings,
                    case,
                    id,
                    &outgoing,
                    &mut incoming,
                    &mut reception,
                )
                .await;
                eprintln!(
                    "HARMOS_BENCHMARK {}",
                    serde_json::json!({
                        "name": format!("sidecar_transport/capacity/{size}/{rate}/{repetition}"),
                        "mean": observation.elapsed_s,
                        "unit": "s",
                        "payload_bytes": size,
                        "samples": case.samples,
                        "target_mib_s": rate,
                        "useful_mib_s": observation.useful_mib_s,
                        "encoded_mib_s": observation.encoded_mib_s,
                        "rejected_attempts": observation.rejected,
                    })
                );
                trials.push(observation);
            }
            cases.push((rate, trials));
        }
        eprintln!(
            "CAPACITY SUMMARY size={size}; median [min, max], rates include receiver decode and pattern validation"
        );
        for (rate, trials) in &cases {
            let keeping_pace = if *rate == 0 {
                "n/a".to_owned()
            } else {
                trials
                    .iter()
                    .all(|trial| trial.keeps_pace(*rate))
                    .to_string()
            };
            let ratio = if *rate == 0 {
                "uncapped".to_owned()
            } else {
                capacity::range(trials.iter().map(|trial| trial.useful_mib_s / *rate as f64))
            };
            eprintln!(
                "target={rate} MiB/s: useful={} MiB/s; encoded={} MiB/s; samples/s={}; elapsed={} s; achieved/target={ratio}; rejected_attempts={}; keeps_pace_all={}",
                capacity::range(trials.iter().map(|trial| trial.useful_mib_s)),
                capacity::range(trials.iter().map(|trial| trial.encoded_mib_s)),
                capacity::range(trials.iter().map(|trial| trial.samples_s)),
                capacity::range(trials.iter().map(|trial| trial.elapsed_s)),
                capacity::range(trials.iter().map(|trial| trial.rejected as f64)),
                keeping_pace
            );
        }
        let peak = cases
            .iter()
            .flat_map(|(rate, trials)| {
                trials
                    .iter()
                    .filter(move |trial| *rate == 0 || !trial.keeps_pace(*rate))
            })
            .map(|trial| trial.useful_mib_s)
            .fold(0.0_f64, f64::max);
        eprintln!(
            "CAPACITY RESULT size={size}: highest tested target met in every repetition={} MiB/s; peak observed uncapped or missed-target rate={peak:.3} useful MiB/s; these are tested operating points, not a precise ceiling",
            capacity::highest_met(&cases)
                .map(|rate| rate.to_string())
                .unwrap_or_else(|| "none".to_owned())
        );
    }
    sdk(&outgoing, &mut incoming, id + 1, false).await;
}

async fn timed_case(
    settings: &Settings,
    case: Case,
    id: u64,
    outgoing: &Sender,
    incoming: &mut Receiver,
    reception: &mut Reception,
) -> Observation {
    tokio::time::timeout(
        settings.timeout(case),
        throughput_case(
            "child process capacity",
            case,
            1,
            id,
            outgoing,
            incoming,
            reception,
        ),
    )
    .await
    .unwrap_or_else(|_| {
        panic!(
            "capacity case timed out: {} bytes/sample, {} requested bytes/s",
            case.payload_bytes, case.pace_bytes_per_second
        )
    })
}

async fn measure(label: &str, outgoing: Sender, mut incoming: Receiver) {
    let mut call_id = 1_u64;
    call(
        &outgoing,
        &mut incoming,
        call_id,
        Frame::REGISTER,
        Vec::new(),
    )
    .await;
    call_id += 1;
    sdk(&outgoing, &mut incoming, call_id, true).await;

    let mut reception = Reception::default();
    for case in CASES {
        call_id += 1;
        throughput_case(
            label,
            case,
            1,
            call_id,
            &outgoing,
            &mut incoming,
            &mut reception,
        )
        .await;
    }
    // The Same Rows As Ordered `host.publish` Batches Of 100 Rows Per Call
    call_id += 1;
    throughput_case(
        label,
        CASES[0],
        BATCH_ROWS,
        call_id,
        &outgoing,
        &mut incoming,
        &mut reception,
    )
    .await;

    call_id += 1;
    latency(label, call_id, &outgoing, &mut incoming).await;
    call_id += LATENCY_SAMPLES;
    sdk(&outgoing, &mut incoming, call_id, false).await;
}

async fn throughput_case(
    label: &str,
    case: Case,
    batch: u32,
    call_id: u64,
    outgoing: &Sender,
    incoming: &mut Receiver,
    reception: &mut Reception,
) -> Observation {
    let previous_sequence = reception.sequence;
    let started = Instant::now();
    let request = Start {
        samples: case.samples,
        payload_bytes: case.payload_bytes,
        pace_bytes_per_second: case.pace_bytes_per_second,
        batch,
    };
    phase(
        outgoing,
        incoming,
        call_id,
        WireCommand::Prepare {
            invocation: call_id,
            operation: Start::ID.to_owned(),
            payload: request.encode_to_vec(),
        },
    )
    .await;
    phase(
        outgoing,
        incoming,
        call_id,
        WireCommand::Initialize {
            invocation: call_id,
            context: [0; 32],
        },
    )
    .await;
    let command = WireCommand::Attempt {
        invocation: call_id,
        context: [0; 32],
        values: Vec::new(),
    };
    outgoing
        .send(Frame::Call {
            id: call_id,
            method: WIRE_METHOD.to_owned(),
            params: command.encode().unwrap(),
        })
        .await
        .expect("the publication case is requested");

    let mut samples = 0_usize;
    let mut calls = 0_usize;
    let mut bytes = 0_usize;
    let mut answered = false;
    let mut done = None;
    while done.is_none() || !answered || samples < case.samples as usize {
        match receive(incoming).await {
            Frame::Call { id, method, params }
                if method == harmos_domain::operations::CHILD_METHOD =>
            {
                let call: harmos_domain::operations::WireChildCall =
                    harmos_domain::operations::WireChildCall::decode(&params)
                        .expect("typed child envelope");
                assert_eq!(call.operation, "host.publish");
                assert_eq!(call.generation_context, Some([2; 32]));
                let row = harmos_domain::operations::streams::Publish::decode(&call.payload)
                    .expect("typed publication");
                calls += 1;
                if row.stream == Done::ID {
                    done = Some(Done::decode(&row.payload).expect("typed completion stream"));
                } else {
                    assert_eq!(row.stream, Sample::ID);
                    let first = row.sequence.expect("native source sequence is explicit");
                    let mut stamp = row.published_unix_micros;
                    let following =
                        row.following.into_iter().map(|following| {
                            stamp = stamp.zip(following.published_delta_micros).map(
                                |(previous, delta)| previous.checked_add(delta).expect("row stamp"),
                            );
                            (following.payload, stamp)
                        });
                    let rows =
                        std::iter::once((row.payload, row.published_unix_micros)).chain(following);
                    for (sequence, (payload, published_unix_micros)) in (first..).zip(rows) {
                        let publication = harmos_sidecar::Publication {
                            stream: row.stream.clone(),
                            schema: row.schema,
                            sequence,
                            published_unix_micros,
                            context: row.context.clone(),
                            payload,
                        };
                        reception.accept(&publication, case.payload_bytes);
                        samples += 1;
                        bytes += publication.payload.len();
                    }
                }
                // This Bench Exercises The Direct Carrier, Not Root Authorization
                outgoing
                    .send(Frame::Answer {
                        id,
                        result: <() as Message>::encode_to_vec(&()),
                    })
                    .await
                    .expect("publication admission acknowledged");
            }
            Frame::Answer { id, result } if id == call_id => {
                match WireAnswer::decode(&result).unwrap() {
                    WireAnswer::Ready { .. } => {}
                    WireAnswer::Stream { .. } => {
                        panic!("unexpected stream response to publication operation")
                    }
                    WireAnswer::Failed { failure } => {
                        panic!("publication operation refused: {failure}")
                    }
                }
                answered = true;
            }
            other => panic!("the benchmark received an unexpected frame: {other:?}"),
        }
    }
    let done = done.expect("the publication case announced completion");

    let elapsed = started.elapsed();
    phase(
        outgoing,
        incoming,
        call_id,
        WireCommand::Finalize {
            invocation: call_id,
        },
    )
    .await;
    let mib = bytes as f64 / MIB as f64;
    let mib_per_second = mib / elapsed.as_secs_f64();
    let samples_per_second = samples as f64 / elapsed.as_secs_f64();
    let rejected = done
        .attempted
        .checked_sub(done.accepted)
        .expect("accepted count cannot exceed attempted count");
    assert_eq!(
        reception.sequence - previous_sequence,
        done.attempted,
        "sequence gaps must exactly account for rejected enqueue attempts"
    );
    let useful_mib_per_second =
        samples as f64 * f64::from(case.payload_bytes) / MIB as f64 / elapsed.as_secs_f64();
    assert_eq!(samples, case.samples as usize);
    assert_eq!(done.accepted, u64::from(case.samples));
    eprintln!(
        "SIDECAR ({label}, {}, {batch} row(s)/call): {samples} samples, {mib:.2} MiB Postcard payload, \
         {mib_per_second:.2} encoded MiB/s, {useful_mib_per_second:.2} useful MiB/s, {samples_per_second:.0} samples/s, {rejected} queue rejected attempts in {elapsed:?}; {calls} host.publish calls, {:?}/call",
        case.name,
        elapsed / u32::try_from(calls.max(1)).unwrap_or(u32::MAX),
    );
    Observation {
        useful_mib_s: useful_mib_per_second,
        encoded_mib_s: mib_per_second,
        samples_s: samples_per_second,
        elapsed_s: elapsed.as_secs_f64(),
        rejected,
    }
}

async fn latency(label: &str, first_id: u64, outgoing: &Sender, incoming: &mut Receiver) {
    let mut elapsed = Vec::with_capacity(LATENCY_SAMPLES as usize);
    for nonce in 0..LATENCY_SAMPLES {
        let id = first_id + nonce;
        let request = Echo {
            nonce,
            payload: vec![b'x'; 64],
        };
        let started = Instant::now();
        phase(
            outgoing,
            incoming,
            id,
            WireCommand::Prepare {
                invocation: id,
                operation: Echo::ID.to_owned(),
                payload: request.encode_to_vec(),
            },
        )
        .await;
        phase(
            outgoing,
            incoming,
            id,
            WireCommand::Initialize {
                invocation: id,
                context: [0; 32],
            },
        )
        .await;
        let answer = phase(
            outgoing,
            incoming,
            id,
            WireCommand::Attempt {
                invocation: id,
                context: [0; 32],
                values: Vec::new(),
            },
        )
        .await;
        phase(
            outgoing,
            incoming,
            id,
            WireCommand::Finalize { invocation: id },
        )
        .await;
        elapsed.push(started.elapsed());
        assert_eq!(
            Echo::decode(answer.as_slice()).expect("the echo reply is Postcard"),
            request
        );
    }
    elapsed.sort_unstable();
    eprintln!(
        "SIDECAR ({label}, 64-byte route round trip): p50 {:?}, p95 {:?}, p99 {:?}, worst {:?} over {LATENCY_SAMPLES} calls",
        percentile(&elapsed, 50),
        percentile(&elapsed, 95),
        percentile(&elapsed, 99),
        elapsed.last().expect("the benchmark made calls"),
    );
}

async fn sdk(outgoing: &Sender, incoming: &mut Receiver, id: u64, initialize: bool) {
    let (operation, payload) = if initialize {
        let request = Initialize {
            config_json: b"null".to_vec(),
            generation_context: vec![2; 32],
        };
        (Initialize::<TransportPlugin>::ID, request.encode_to_vec())
    } else {
        (Shutdown::<TransportPlugin>::ID, Vec::new())
    };
    phase(
        outgoing,
        incoming,
        id,
        WireCommand::Prepare {
            invocation: id,
            operation: operation.to_owned(),
            payload,
        },
    )
    .await;
    phase(
        outgoing,
        incoming,
        id,
        WireCommand::Initialize {
            invocation: id,
            context: [1; 32],
        },
    )
    .await;
    phase(
        outgoing,
        incoming,
        id,
        WireCommand::Attempt {
            invocation: id,
            context: [1; 32],
            values: Vec::new(),
        },
    )
    .await;
    phase(
        outgoing,
        incoming,
        id,
        WireCommand::Finalize { invocation: id },
    )
    .await;
}

async fn phase(
    outgoing: &Sender,
    incoming: &mut Receiver,
    id: u64,
    command: WireCommand,
) -> Vec<u8> {
    let params = command.encode().unwrap();
    let result = call(outgoing, incoming, id, WIRE_METHOD, params).await;
    match WireAnswer::decode(&result).unwrap() {
        WireAnswer::Ready { payload } => payload,
        WireAnswer::Failed { failure } => panic!("operation phase refused: {failure}"),
        WireAnswer::Stream { .. } => panic!("unexpected stream response to unary phase"),
    }
}

async fn call(
    outgoing: &Sender,
    incoming: &mut Receiver,
    id: u64,
    method: &str,
    params: Vec<u8>,
) -> Vec<u8> {
    outgoing
        .send(Frame::Call {
            id,
            method: method.to_owned(),
            params,
        })
        .await
        .expect("the benchmark call is sent");
    match receive(incoming).await {
        Frame::Answer {
            id: answered,
            result,
        } if answered == id => result,
        other => panic!("the benchmark received an unexpected frame: {other:?}"),
    }
}

async fn receive(incoming: &mut Receiver) -> Frame {
    tokio::time::timeout(Duration::from_secs(5), incoming.receive())
        .await
        .expect("the benchmark received no frame for 5 seconds")
        .expect("the benchmark stream remains valid")
        .expect("the benchmark stream remains open")
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    samples[(samples.len() * percentile).div_ceil(100).saturating_sub(1)]
}

fn benchmarks(c: &mut criterion::Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut group = c.benchmark_group("sidecar_transport");
    group.sample_size(10);
    group.bench_function("memory", |b| {
        b.iter(|| runtime.block_on(in_memory_transport_profile()))
    });
    group.bench_function("process", |b| {
        b.iter(|| runtime.block_on(child_process_transport_profile()))
    });
    group.finish();
    // The Capacity Sweep Already Owns Warmup And Repeated Trials Per Workload.
    runtime.block_on(child_process_capacity());
}

/* === Tests === */
