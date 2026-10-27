/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Real Socket Commands Against An Independently Running Server Process.

/* === Modules === */

#[allow(dead_code, reason = "The Shared Protocol Also Defines The Separate Server Implementation")]
#[path = "../../tests/fixtures/benchmark_protocol.rs"]
mod protocol;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::process::Stdio;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_runtime::Message;
use tokio::io::{AsyncBufReadExt, BufReader};

// Local Imports

/* === Constants === */

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);
criterion::criterion_main!(benches);

/* === Type Aliases === */

/* === Structs === */

struct Fixture {
    child: tokio::process::Child,
    client: tonic::client::Grpc<tonic::transport::Channel>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    async fn start() -> Self {
        let executable = std::env::current_exe().unwrap();
        let name = if cfg!(windows) { "harmos-grpc-benchmark-fixture.exe" } else { "harmos-grpc-benchmark-fixture" };
        let server = executable.parent().unwrap().parent().unwrap().join("bin").join(name);

        let mut command = tokio::process::Command::new(server);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command.spawn().unwrap();

        let stdout = child.stdout.take().unwrap();
        let mut output = BufReader::new(stdout);
        let mut address = String::new();
        tokio::time::timeout(Duration::from_secs(30), output.read_line(&mut address)).await.unwrap().unwrap();
        let address: std::net::SocketAddr = address.trim().parse().expect("the server reports its bound socket");
        let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap();
        let channel = endpoint.connect().await.unwrap();
        Self { child, client: tonic::client::Grpc::new(channel) }
    }

    async fn call(&mut self, payload: &[u8]) -> harmos_runtime::operations::StreamItem<protocol::Payload> {
        self.client.ready().await.unwrap();
        let settings = protocol::wire_settings();
        let codec = harmos_runtime::grpc::Codec::<
            harmos_runtime::grpc::Transcoded,
            protocol::Payload,
            harmos_runtime::operations::Response<protocol::Echo>,
        >::new(settings.decode, settings.encode)
        .unwrap();
        let request = protocol::Echo { bytes: payload.to_vec() };
        let request = harmos_runtime::operations::Response::new(request);
        let request = tonic::Request::new(request);

        let path = tonic::codegen::http::uri::PathAndQuery::from_static("/benchmark.v1.Commands/Echo");
        self.client.unary(request, path, codec).await.unwrap().into_inner()
    }

    async fn finish(mut self) {
        drop(self.child.stdin.take());
        let result = tokio::time::timeout(Duration::from_secs(30), self.child.wait()).await;
        match result {
            Ok(status) => assert!(status.unwrap().success()),
            Err(_) => {
                self.child.kill().await.unwrap();
                panic!("benchmark server did not settle after stdin closed");
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn benchmarks(criterion: &mut Criterion) {
    codecs(criterion);
    let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let mut startup = criterion.benchmark_group("grpc/startup");
    startup.sample_size(10);
    startup.bench_function("process-and-connect", |bencher| {
        bencher.iter_custom(|iterations| {
            executor.block_on(async {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let started = Instant::now();
                    let mut fixture = Fixture::start().await;
                    elapsed += started.elapsed();
                    let answer = fixture.call(&[]).await;
                    assert!(answer.bytes.is_empty());
                    fixture.finish().await;
                }
                elapsed
            })
        });
    });
    startup.finish();

    let mut fixture = executor.block_on(Fixture::start());
    for _ in 0..8 {
        let answer = executor.block_on(fixture.call(&[]));
        assert!(answer.bytes.is_empty());
    }
    let mut commands = criterion.benchmark_group("grpc/commands");
    commands.sample_size(10);
    for bytes in [0, 102, 1024 * 1024 / 10] {
        let payload: Vec<_> = (0..bytes).map(|index| (index % 251) as u8).collect();
        let answer = executor.block_on(fixture.call(&payload));
        assert_eq!(answer.bytes, payload);
        commands.throughput(if bytes == 0 { Throughput::Elements(1) } else { Throughput::Bytes(2 * bytes as u64) });
        commands.bench_with_input(BenchmarkId::new("echo", bytes), &payload, |bencher, payload| {
            bencher.iter(|| executor.block_on(fixture.call(payload)));
        });
    }
    commands.finish();
    executor.block_on(fixture.finish());
}

fn codecs(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("grpc/codecs");
    group.sample_size(10);
    for bytes in [0, 102, 1024 * 1024 / 10] {
        let value = protocol::Payload { bytes: (0..bytes).map(|index| (index % 251) as u8).collect() };
        let postcard = value.encode();
        let mut protobuf = Vec::new();
        harmos_sdk::grpc::encode(&value, &mut protobuf).unwrap();
        let decoded =
            harmos_sdk::grpc::prepare::<protocol::Payload>(harmos_sdk::grpc::DecodeInput::new(&protobuf)).unwrap().decode().unwrap();
        assert_eq!(decoded.bytes, value.bytes);
        group.throughput(if bytes == 0 { Throughput::Elements(1) } else { Throughput::Bytes(bytes as u64) });
        group.bench_function(BenchmarkId::new("postcard-encode", bytes), |bencher| {
            bencher.iter(|| std::hint::black_box(&value).encode());
        });
        group.bench_function(BenchmarkId::new("postcard-decode", bytes), |bencher| {
            bencher.iter(|| protocol::Payload::decode(std::hint::black_box(&postcard)).unwrap());
        });
        group.bench_function(BenchmarkId::new("protobuf-encode", bytes), |bencher| {
            bencher.iter(|| {
                let mut output = Vec::new();
                harmos_sdk::grpc::encode(std::hint::black_box(&value), &mut output).unwrap();
                output
            });
        });
        group.bench_function(BenchmarkId::new("protobuf-prepare-decode", bytes), |bencher| {
            bencher.iter(|| {
                harmos_sdk::grpc::prepare::<protocol::Payload>(harmos_sdk::grpc::DecodeInput::new(std::hint::black_box(&protobuf)))
                    .unwrap()
                    .decode()
                    .unwrap()
            });
        });
    }
    group.finish();
}

/* === Tests === */
