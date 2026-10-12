# Harmos

**Build stateful Rust applications with history, live data, and plugins.**

Harmos brings managed state, undo, recovery, background work, and typed streams
into one runtime. Use it to build diagram editors, engineering tools, and
instrument hosts where changes must be explainable and work must stop cleanly.

[Quick Start](#quick-start) · [Examples](#examples) ·
[Performance](#performance) · [Documentation](#documentation)

Move an object in a diagram, rename a connection, then undo just the move:

```rust
let editor = runtime.state.as_principal(alice.clone());

editor.commit(MoveSymbol { symbol, to: Point::new(10, 45) }).await?;
editor.commit(RenameNet { net, to: NetName::parse("VCC") }).await?;

let selection = Focus::matching(move |change: &Transactions| {
    change.symbol() == Some(symbol)
});
editor.undo(alice, selection).await?;
// The symbol returns to its previous position. The connection remains VCC.
```

This excerpt uses the application-defined transactions and diagram objects in
[Minischematic](examples/minischematic/). Harmos manages the accepted history;
your application defines what each change means.

## Features

- **Managed State**

  Apply typed, validated transactions to ordinary Rust state. Read historical
  values and scope undo or redo to a user and a selected part of the application.

- **Operations**

  Define typed requests, responses, and streaming calls with explicit timeout,
  retry, and cancellation policies. Operation declarations can also generate
  protobuf contracts and expose gRPC methods through the runtime.

- **Services**

  Run resident background work with managed startup, shutdown, and resource
  ownership. Keep device handles and live clients with the work that owns them.

- **Streams**

  Publish typed measurements with demand-driven production, bounded windows,
  capture, and explicit gap reporting. Samples retain their application-state
  context without adding every measurement to the state journal; dedicated
  [benchmarks](#performance) cover batching and native consumption.

- **Storage**

  Supply adapters for committed history, snapshots, streams, and evidence.
  Your application chooses its database and file format; Harmos defines the
  contracts they implement.

- **Crash Recovery**

  Reconstruct state from persisted history and snapshots, including the history
  needed for undo. Recovery follows the storage adapter's durability policy;
  an accepted change is not necessarily durable until the adapter stores it.

- **Guests**

  Extend your application with sandboxed WebAssembly components. The host
  controls admitted capabilities and execution budgets, including fuel and
  memory limits, through the Wasmtime boundary.

- **Sidecars**

  Integrate native executables through supervised processes and typed contracts.
  A binary transport separates data from lifecycle control, with
  [benchmarks](#performance) for throughput and latency. Native process isolation
  does not provide the Wasm guest sandbox.

- **Simulation**

  Exercise supported workflows with repeatable seeds. Check that repeated runs
  agree, replay reconstructs state, and inverse transactions restore it.

## Quick Start

Harmos is pre-1.0. APIs may change, and the crate is not yet published to
crates.io. Windows and Linux are the supported development platforms.

Try the diagram-editor walkthrough with the repository's pinned toolchain:

```sh
git clone https://github.com/stokker-technologies/harmos.git
cd harmos
mise trust
mise install --locked
mise run sync
mise run run:minischematic
```

Install [mise](https://mise.jdx.dev) first. The walkthrough runs in the terminal
and demonstrates editing, scoped undo/redo, stream capture, and recovery; it does
not require a graphical editor or connected hardware.

To use Harmos in your application, pin a tested Git commit:

```toml
[dependencies]
harmos = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>" }
```

Select optional facilities with [Cargo Features](#cargo-features).

## Examples

### A Diagram Editor

A drawing application owns its shapes, connections, and edit rules. Harmos owns
which changes were accepted and in which order. Minischematic uses circuit
symbols and nets to demonstrate this division.

Its [walkthrough](examples/minischematic/cli/src/commands/run.rs) places symbols,
connects pins, moves an object, and undoes that move without reverting a later
connection rename. It then reopens persisted state and demonstrates that undo
still works after recovery.

The same model runs under deterministic simulation. Its
[simulation test](examples/minischematic/app/tests/app/simulation.rs) checks:

```rust
let first = scripted(42);
let second = scripted(42);

first.assert_same(&second);
assert_eq!(first.trace(), second.trace());
first.assert_replay_equivalent();
first.assert_inverses_restore();
```

Here, `scripted` is the example's helper that performs edits and reboots the
simulated application. For persistence, the
[JSONL adapter](examples/jsonl-adapter/) demonstrates a journal, snapshots,
stream capture, and evidence files in an application-owned format.

### An On-Board Charger Test Bench

A charger test application needs to configure the unit, control charging, and
observe voltage, current, temperature, and status. Put charger communication
and device resources in a plugin; let the host coordinate the test and consume
its measurements.

The following is an illustrative adaptation of the
[standalone plugin observer](examples/standalone-sidecar/src/observers/drift.rs).
`ChargerPlugin`, `Resources`, and the simulated device's `read` method are
application-defined; a complete charger example is not bundled yet.

```rust
#[harmos::stream(id = "charger.readings", window = 4096)]
#[harmos::message]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reading {
    pub voltage_v: f64,
    pub current_a: f64,
    pub temperature_c: f64,
}

#[derive(Default)]
pub struct Measurements;

#[harmos::observer(
    interval_ms = 100,
    streams(Reading),
    activation = "demand"
)]
impl Measurements {
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Resources, Publisher<ChargerPlugin>>,
    ) -> Result<(), PublishError> {
        let reading = ctx.resources().charger.read();
        ctx.publish_on::<Reading>(&reading).await
    }
}
```

The plugin declares this observer in its catalog and attaches it to its original
initialized resources. The host leases the stream when it needs measurements;
operations on the same plugin configure the charger and start or stop work.

Those operations can expose gRPC from the same declaration. For example, an
operation serving a charger reading would select:

```rust
grpc(package = "charger.v1", service = "Charger", method = "Read")
```

This metadata belongs on the operation declaration. The build or packaging step
exports its protobuf schema; normal operation registration makes the declared
method available through the configured runtime gRPC listener. Requests and
responses remain typed, without a second hand-maintained protocol definition.

For runnable native-plugin examples, start with
[Minibench](examples/minibench/) and the
[standalone sidecar](examples/standalone-sidecar/). The
[Autoroute guest](examples/minischematic/README.md#autoroute-guest) demonstrates
Wasm capability admission and execution budgets using the diagram application.

Building a complete test application?
[Dokime](https://github.com/stokker-technologies/dokime) builds on
Harmos with test procedures, operator workflows, live visualization, and
captured evidence reports.

## Cargo Features

The `harmos` crate enables `runtime` and `derive` by default. Choose additional
facilities for your application, or disable defaults when authoring a plugin
that does not need the host runtime.

| Feature | Default | Enables |
| --- | --- | --- |
| `runtime` | Yes | Host runtime, managed state, streams, storage, and recovery. |
| `derive` | Yes | Authoring macros for plugins, operations, observers, and catalogs. |
| `load` | No | In-process plugin loading and lifecycle management. |
| `sidecar` | No | Native sidecar authoring SDK. |
| `sidecar-host` | No | Native plugin hosting and process transport. |
| `guest` | No | Wasm guest authoring SDK. |
| `guest-host` | No | Sandboxed Wasm execution through Wasmtime. |
| `grpc` | No | gRPC integration and reflection. |
| `grpc-tls` | No | TLS support for gRPC. |
| `grpc-web` | No | gRPC-Web on the gRPC listener. |
| `signal` | No | Signal-processing kernels. |
| `fft` | No | FFT support for signal processing. |
| `sim` | No | Runtime simulation support. |
| `artifact` | No | Plugin artifact declarations and admission interfaces. |
| `componentize` | No | Packaging core Wasm modules as components. |
| `plugin` | No | Plugin bundle packaging. |

`load`, `sidecar-host`, `guest-host`, and `sim` enable `runtime` automatically.
`grpc-tls` and `grpc-web` enable `grpc`; `fft` enables `signal`. Authoring flags
`sidecar` and `guest` do not enable their corresponding hosts.

For example, a Wasm guest can select its authoring SDK and macros without the
host runtime or Wasmtime engine:

```toml
[dependencies]
harmos = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>", default-features = false, features = ["guest", "derive"] }
```

See the [crate manifest](crates/harmos/Cargo.toml) for exact feature dependencies.

## Performance

Streams and native plugin transport have separate benchmark suites, so codec
cost, transport throughput, and application consumption can be examined
independently.

| Suite | What It Measures |
| --- | --- |
| [Streams](crates/harmos/benches/streams/main.rs) | Concurrent waveform work, batch summaries, codec costs, and native consumers using 64 KiB and 1 MiB payloads. |
| [Plugin Transport](crates/harmos-sidecar/benches/transport/main.rs) | Native transport throughput and latency, including 64-byte, 1 KiB, and 64 KiB bursts and a paced 10 MiB/s workload. |

The paced rate is a benchmark input, not a claimed throughput result. Run
`mise run bench` to measure your checkout. Published comparisons need the
revision, hardware, payload, batching, and carrier alongside the results;
these native benchmarks do not establish Wasm guest performance.

## Documentation

- [Runnable Examples](examples/) — application composition and plugin integration.
- [Public Rust API](crates/harmos/src/lib.rs) — the SDK facade and module documentation.
- [Cargo Features](crates/harmos/Cargo.toml) — select the facilities your application needs.
- [Storage Adapter](examples/jsonl-adapter/) — own your persistence format.

## Project Status & Roadmap

Harmos is in active development. Pin the revision you integrate and expect
changes to public APIs and plugin contracts before 1.0.

The next documentation milestones are a complete simulated charger walkthrough
and reproducible published stream and plugin-transport results. GitHub hosts the
authoritative repository and native CI; GitLab retains a one-way mirror.

Follow [issues](https://github.com/stokker-technologies/harmos/issues)
for development discussions and [the changelog](CHANGELOG.md) for recorded changes.

## Development Setup

Use the setup commands in [Quick Start](#quick-start), then choose a task:

```sh
mise run run:minibench           # Run the native instrument-plugin example
mise run run:minischematic:guest # Run the sandboxed Wasm example
mise run check                  # Run repository checks
mise run test                   # Run the test suite
mise run bench                  # Record benchmark results
```

Every new commit SHA runs fresh checks, release builds and complete tests,
including squash, merge and history-reset commits with identical trees. `dev`
requires Linux validation and its separate `development.json` artifact proof.
Windows runs independently with visible outcomes and remains advisory for `dev`
until its requirements are explicitly restored. `main` and releases
always require both native platforms. Coverage is an optional diagnostic through
`STKR_COVERAGE=1 mise run test`; ordinary validation keeps the full selection.
Qualified dev pushes may use the restricted local Linux runner. PR full validation requires approval; integration into `dev` and `main`
starts automatic validation of the resulting SHA. Caches accelerate those builds.
`mise run packages:reuse` retrieves successful artifacts only for the current SHA,
with matching inputs and intact hashes; it never rebuilds or changes provenance.
Use `HARMOS_VALIDATION_RUN_ID` to select an explicit same-SHA Actions run.

Task PRs squash into `dev`. Stable `release/<version>` branches start from `dev`
and merge into `main` preserving ancestry, then incorporate their history into
`dev`. Harmos currently enables the release-branch route into `main`; a published-tag
hotfix needs its explicit release context before enabling that route.

See [Contributing](CONTRIBUTING.md) for prerequisites and the full development
workflow.

## Contributing

Bug reports, focused fixes, API feedback, and useful examples are welcome.
[Open an issue](https://github.com/stokker-technologies/harmos/issues)
to discuss a change, and read the [contribution guide](CONTRIBUTING.md) before
submitting it.

## License

Harmos is licensed under [Apache-2.0](LICENSE-APACHE).
