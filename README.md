# Harmos

[![Release](https://img.shields.io/github/v/release/stokker-technologies/harmos)](https://github.com/stokker-technologies/harmos/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/ci.yml?branch=dev&label=CI)](https://github.com/stokker-technologies/harmos/actions/workflows/ci.yml)
[![CI / Nightly](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/nightly.yml?branch=dev&label=CI%20%2F%20Nightly)](https://github.com/stokker-technologies/harmos/actions/workflows/nightly.yml)
[![crates.io](https://img.shields.io/crates/v/harmos)](https://crates.io/crates/harmos)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

<img src="https://stokkertechnologies.ch/logo.svg" height="256" alt="Stokker Technologies Mark">

[Website](https://github.com/stokker-technologies/harmos) · [Getting Started](#quick-start) · [Tutorials](docs/schematic.md) · [Examples](docs/examples.md) · [Documentation](docs/product-guide.md)

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

- **Managed State**: Apply typed transactions, inspect history and scope undo or redo to selected changes.
- **Operations and Services**: Define requests and streams with explicit timeout, retry and cancellation policies, including typed final responses from cooperative cancellation; keep resources with the work that owns them.
- **Streams and Storage**: Capture bounded live measurements and supply adapters for history, snapshots and streams.
- **Crash Recovery**: Reconstruct host state and undo history from bounded snapshot segments and journal replay; retain plugin history with its written definitions.
- **WebAssembly Guests** [`plugin-guest`]: Extend an application with grants, fuel and memory limits enforced by the Wasmtime boundary.
- **gRPC Plugin Placement**: Invoke managed or remote `harmos.plugin.v1.Instance` plugins, capture their streams and drive plugin waits and observers from the host clock.
- **Explicit Visibility**: Private operations run inside their SDK process, Protected operations remain host/owner callable, and Public operations admit declared peers and clients through the host.
- **[Plugin Management](docs/plugin-manager.md)**: Persist, install, enable, restart and remove managed or remote plugin entries.
- **Access and Host Status**: Compose library-owned TLS listeners, durable pairings, live trust rotation, contributed health and redacted diagnostics; see [Access and Host Status](docs/access-status.md).
- **gRPC Interfaces** [`grpc`]: Register generated protobuf messages directly, expose typed operations and generate client contracts.
- **Simulation** [`simulation`]: Exercise supported workflows with repeatable seeds.

## Interfaces

### CLI

The Schematic example supplies a runnable application CLI that demonstrates plugin routing,
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

Plugin and client authors depend on `harmos` and import `harmos::prelude::*`.
Its protobuf messages, plugin builders and generated-contract clients compile without host code.
Clients bind `client::Method` to generated application types, then use `Client::call`,
`Client::watch` for watches, or `Client::start` for an explicitly declared cooperative final result; see the [Rust client example](sdk/rust/examples/client.rs).
Host applications depend explicitly on `harmos-runtime` for state, storage,
managed or remote `harmos.plugin.v1.Instance` placement and optional guest hosting.
See the [product guide](docs/product-guide.md#cargo-features) for crate boundaries.
Host operations, state catalogs and record registries are values; data shapes use
field-reading derives. See [Native Operation Values](docs/product-guide.md#native-operation-values).

Portable messages support standard Rust maps with deterministic key ordering
and bounded decoding, plus root optional payloads that preserve value presence. The [message guide](docs/product-guide.md)
describes their repeated key/value entry representation.

`harmos-storage/sqlite` adds an optional durable reference backend with bounded Arrow scans,
ordered indexes, exact-frontier projections, grouped time buckets and atomic object updates.
See [Storage Interfaces](docs/storage.md) for native and generated SDK APIs, backend obligations and saved-cut query examples.

Structured operation and storage tracing supports performance investigation;
applications control collection. See [Performance](docs/product-guide.md#performance).

#### Examples

The [Schematic walkthrough](examples/schematic/cli/src/walkthrough.rs) covers
editing, plugin validation, undo/redo and save/reopen. Its
[storage adapter](examples/schematic/storage/) shows application-owned persistence.
Its native plugin serves unary advice over gRPC; analysis and transfer streams run
in the host. The guest uses SDK builders with `harmos/wasm` and exposes unary transfer endpoints under its `benchmark` feature
for the [performance suites](docs/product-guide.md#performance).

### C++

The C++20 SDK serves `harmos.plugin.v1.Instance` with protobuf messages and builder values for
operations, streams and observers. See the [C++ SDK guide](docs/tooling.md#c-plugin-sdk)
for toolchain setup and managed or remote serving.

#### Examples

Build the SDK and run its plugin against the reference host:

```sh
mise run build -- --language cpp
mise run build -- --package harmos-conformance
mise run test -- --language cpp
```

These opt-in tasks require a C++20 compiler and CMake; ordinary Rust builds do not.

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
[SDK](sdk/rust/Cargo.toml) or [runtime features](crates/harmos-runtime/Cargo.toml) your application needs.

The conformance tool package lives in `tools/conformance`; build and generation tooling
lives in `tools/packaging`, outside the core libraries.

[Releases](https://github.com/stokker-technologies/harmos/releases) attach
`harmos-proto-<version>.zip`, one `harmos-conformance-<version>-<platform>.zip|tar.gz`
per platform, `harmos-compliance-<version>.zip`, `release.json` and `SHA256SUMS`.
The changelog is the release body; crates and language SDKs use their registries.
The mutable [Nightly](https://github.com/stokker-technologies/harmos/releases/tag/nightly)
adds `harmos-reports-<version>.zip` and fixed-name `benchmarks.json`. Its notes record
coverage, benchmark regressions, suite results and the workflow link; failed suites
can accompany a published build.

Harmos is pre-1.0; public APIs and plugin contracts can change. Keep application
and plugin revisions aligned and review release notes before updating. Stored data
formats have no migrations. Managed recovery refuses changed host state types;
choose a fresh storage root for those product changes. Plugin history retains its
written definitions across shape changes and plugin removal. Wire compatibility
versions remain explicit.

Built-in operation contracts live in `harmos-operations`, re-exported as
`harmos_runtime::builtins`. Its `host_catalog()`, `lifecycle_catalog()`, `storage_catalog()`,
`streams_catalog()` and `builtins_catalog()` functions return owned catalogs
without depending on `harmos-runtime`.

## Contributing

Focused fixes, API feedback and useful examples are welcome. Read
[Contributing](docs/CONTRIBUTING.md) for setup, validation and pull requests.
Read [Architecture](docs/architecture.md) before proposing architecture or public API changes.
Run targeted tasks locally; hosted CI and Nightly provide full validation.
Tool configuration lives in `tools/config/`; see [Repository Tooling](docs/tooling.md).

## About

Harmos is developed by [Stokker Technologies](https://stokkertechnologies.ch)
as a foundation for stateful applications, engineering workflows and managed
plugins. It is available under [Apache-2.0](LICENSE).
