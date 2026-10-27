# Harmos

[![Release](https://img.shields.io/github/v/release/stokker-technologies/harmos)](https://github.com/stokker-technologies/harmos/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/ci.yml?branch=dev&label=CI)](https://github.com/stokker-technologies/harmos/actions/workflows/ci.yml)
[![CI / Nightly](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/nightly.yml?branch=dev&label=CI%20%2F%20Nightly)](https://github.com/stokker-technologies/harmos/actions/workflows/nightly.yml)
[![crates.io](https://img.shields.io/crates/v/harmos)](https://crates.io/crates/harmos)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

<img src="https://stokkertechnologies.ch/logo.svg" height="256" alt="Stokker Technologies Mark">

[Website](https://github.com/stokker-technologies/harmos) · [Getting Started](#quick-start) · [Tutorials](examples/schematic/README.md) · [Examples](examples/) · [Documentation](docs/product-guide.md)

**Build stateful Rust applications with history, live data and plugins.**

Harmos brings managed state, undo, recovery, background work and typed streams
into one runtime. Use it for engineering tools and instrument hosts where
changes must be explainable and work must stop cleanly.

Move a diagram symbol, rename a connection, then undo only the move:

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

This excerpt uses the application-defined types in [Schematic](examples/schematic/).
Your application defines each change; Harmos manages its accepted history.

## Features

- **Managed State** [`runtime`]: Apply typed transactions, inspect history and scope undo or redo to selected changes.
- **Operations And Services** [`runtime`]: Define requests and streams with explicit timeout, retry and cancellation policies; keep resources with the work that owns them.
- **Streams And Storage** [`runtime`]: Capture bounded live measurements and supply adapters for history, snapshots, streams and evidence.
- **Crash Recovery** [`runtime`]: Reconstruct state and undo history according to your storage adapter's durability policy.
- **Plugin Lifecycle** [`plugin`]: Register typed plugin contracts and manage activation, routing and reload.
- **WebAssembly Guests** [`plugin-guest`]: Extend an application with grants, fuel and memory limits enforced by the Wasmtime boundary.
- **Native Sidecars** [`plugin-sidecar`]: Integrate supervised executables through typed contracts; native processes do not provide a Wasm sandbox.
- **gRPC Interfaces** [`grpc`, `generate-proto`]: Expose typed operations and generate client contracts.
- **Signal Processing** [`signal`, `signal-fft`]: Resample and summarize waveforms, with optional FFT analysis.
- **Simulation** [`simulation`]: Exercise supported workflows with repeatable seeds.

## Interfaces

### CLI

`cargo harmos` supports plugin authoring and contract generation. The Schematic
example supplies a runnable application CLI that demonstrates plugin routing,
history and recovery without a graphical editor.

#### Examples

After building the example, choose its native sidecar or WebAssembly guest:

```sh
target/build/dev/linux-x86_64/bin/schematic-cli
target/build/dev/linux-x86_64/bin/schematic-cli --guest
```

### Python

Applications can generate Python clients from their operation contracts.
The application owns the generated package, connection and authentication;
Harmos does not ship a separate general-purpose PyPI client.

#### Examples

The [contract and binding examples](docs/product-guide.md#cargo-features) show
how a producer exports its interface. Keep generated clients matched to the
application revision that produced them.

### Rust

Use the `harmos` facade to compose a runtime and define your application's
state, transactions and operations. Optional Cargo features select plugin,
transport and simulation capabilities. The default facade exposes portable contracts;
enable `runtime` for SDK execution and, on native targets, host runtime facilities.
The [operations guide](.agents/skills/harmos-authoring/references/operations.md)
shows one operation file where the request implements consuming execution with declared resources and child operations.

Structured operation and storage tracing supports performance investigation;
applications control collection. See [Performance](docs/product-guide.md#performance).

#### Examples

The [Schematic walkthrough](examples/schematic/cli/src/walkthrough.rs) covers
editing, plugin validation, undo/redo and save/reopen. Its
[storage adapter](examples/schematic/storage/) shows application-owned persistence.
Its sidecar and guest also provide shared transfer endpoints under the `benchmark`
feature for the [performance suites](docs/product-guide.md#performance).
Harmos also ships a filesystem `JsonStorage` backend for records, streams, objects
and files; see the [storage configuration guide](docs/product-guide.md).

## Quick Start

Try the Schematic application with the repository's pinned tools:

```sh
git clone https://github.com/stokker-technologies/harmos.git
cd harmos
mise trust
mise install --locked
mise run prepare:tools
mise run build
target/build/dev/linux-x86_64/bin/schematic-cli
```

On Windows, run the corresponding executable under
`target/build/dev/windows-x86_64/bin/`. For library integration, select a
[published Harmos version](https://crates.io/crates/harmos) and the
[Cargo features](crates/harmos/Cargo.toml) your application needs.

Harmos is pre-1.0; public APIs and plugin contracts can change. Keep application
and plugin revisions aligned and review release notes before updating. Stored data
and wire formats support only the current v1 shape; no migration is provided.

Built-in operation contracts live in `harmos-operations`, re-exported as
`harmos::builtins`. Its `host`, `lifecycle`, `storage` and `streams` catalogs
and `Builtins::catalog()` are available without runtime dependencies.

For agent-assisted consumer development, use the self-contained
[architecture](.agents/skills/harmos-architecture/SKILL.md),
[authoring](.agents/skills/harmos-authoring/SKILL.md) and
[testing](.agents/skills/harmos-testing/SKILL.md) skills. They cover reusable
products and self-contained plugins, including the shipped consumer examples.

## Contributing

Focused fixes, API feedback and useful examples are welcome. Read
[Contributing](CONTRIBUTING.md) for setup, validation and pull requests.

## About

Harmos is developed by [Stokker Technologies](https://stokkertechnologies.ch)
as a foundation for stateful applications, engineering workflows and managed
plugins. It is available under [Apache-2.0](LICENSE).
