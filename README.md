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

let movement = MoveSymbol { symbol, to: Point::new(10, 45) };
let rename = RenameNet { net, to: NetName::parse("VCC") };
editor.commit(movement).await?;
editor.commit(rename).await?;

let selection = Focus::matching(move |change: &Transactions| {
    change.symbol() == Some(symbol)
});
editor.undo(alice, selection).await?;
// The symbol returns to its previous position. The connection remains VCC.
```

This excerpt uses the application-defined transactions and diagram objects in
[Schematic](examples/schematic/). Harmos manages the accepted history;
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
mise run run:schematic
```

Install [mise](https://mise.jdx.dev) first. The walkthrough runs in the terminal
and demonstrates editing, plugin validation and routing, undo/redo, and recovery.
It needs no graphical editor or connected hardware.

To use Harmos in your application, pin a tested Git commit:

```toml
[dependencies]
harmos = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>" }
```

Select optional facilities with [Cargo Features](#cargo-features).

## Examples

The [Schematic Editor](examples/schematic/README.md) is one application with two
plugin backends. Read the [walkthrough](examples/schematic/cli/src/walkthrough.rs)
from top to bottom: place two symbols, connect them, ask the plugin to validate
and propose a route, apply the proposal as a host transaction, undo/redo, and
save/reopen the document.

```sh
mise run run:schematic:sidecar
mise run run:schematic:guest
```

Both plugins implement `ValidateSchematic`, `ProposeRoute`, and the streaming
`AnalyzeSchematic`. The native sidecar additionally implements `ListDirectories`,
which lists immediate subdirectories of its explicitly configured working
directory. The guest has no filesystem capability and omits that operation.

The [advanced follow-on](examples/schematic/cli/src/advanced.rs) demonstrates
bounded streaming, cancellation with settlement, and native directory access.
The [simulation test](examples/schematic/app/tests/app/simulation.rs) checks
repeatability, replay, and inverses. Reusable [JSONL test support](tests/support/jsonl-adapter/)
provides the example's persistence adapter; shutdown stress scenarios live in
[test fixtures](tests/fixtures/lifecycle/).

## Cargo Features

The host runtime, managed state, streams, storage, recovery, and authoring macros
are always available. Optional facilities are disabled by default.

| Feature | Enables |
| --- | --- |
| `plugin` | Shared plugin management, activation, routing, and reload. |
| `plugin-sidecar` | Native process hosting; includes `plugin`. |
| `plugin-guest` | Sandboxed Wasmtime hosting; includes `plugin`. |
| `grpc` | gRPC, reflection, TLS, and Web support; settings select their use. |
| `signal` | Signal-processing kernels. |
| `signal-fft` | FFT support; includes `signal`. |
| `simulation` | Deterministic runtime simulation. |

Portable declarations use `harmos-contract`. Plugin authors use the dedicated
`harmos-sidecar` or `harmos-guest` SDK, without importing the host runtime:

```toml
[dependencies]
harmos-guest = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>", features = ["kit", "streaming"] }
```

`harmos-guest/componentize` packages Wasm components. `harmos-bundle` packages
plugin bundles without a host runtime. Host consumers reach bundle support at
`harmos::plugin::bundle`.

Operation metadata belongs on its owning struct in a vertical file; catalogs
assemble those types. `harmos::prelude::*` provides a bounded set of common
authoring imports. See the [crate manifest](crates/harmos/Cargo.toml) and
[authoring skill](.agents/skills/harmos-authoring/SKILL.md) for details.

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
- [Storage Adapter](tests/support/jsonl-adapter/) — own your persistence format.

## Project Status & Roadmap

Harmos is in active development. Pin the revision you integrate and expect
changes to public APIs and plugin contracts before 1.0.

Reproducible published stream and plugin-transport results remain a documentation
milestone. GitHub hosts the
authoritative repository and native CI; GitLab retains a one-way mirror.

Follow [issues](https://github.com/stokker-technologies/harmos/issues)
for development discussions and [the changelog](CHANGELOG.md) for recorded changes.

## Development Setup

Use the setup commands in [Quick Start](#quick-start), then choose a task:

```sh
mise run run:schematic:sidecar # Run the native plugin example
mise run run:schematic:guest # Run the sandboxed Wasm example
mise run check                  # Run repository checks
mise run test                   # Run the test suite
mise run bench                  # Record benchmark results
```

Every new commit SHA runs fresh Linux and Windows checks, release builds, and
covered tests, including squash, merge, and history-reset commits with identical
trees. PR full validation requires approval; integration into `dev` and `main`
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
