# Harmos

[![Release](https://img.shields.io/github/v/release/stokker-technologies/harmos)](https://github.com/stokker-technologies/harmos/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/ci.yml?branch=dev&label=CI)](https://github.com/stokker-technologies/harmos/actions/workflows/ci.yml)
[![CI / Nightly](https://img.shields.io/github/actions/workflow/status/stokker-technologies/harmos/nightly.yml?branch=dev&label=CI%20%2F%20Nightly)](https://github.com/stokker-technologies/harmos/actions/workflows/nightly.yml)
[![crates.io](https://img.shields.io/crates/v/harmos)](https://crates.io/crates/harmos)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

<img src="https://stokkertechnologies.ch/logo.svg" height="256" alt="Stokker Technologies Mark">

[Website](https://github.com/stokker-technologies/harmos) · [Getting Started](#quick-start) · [Tutorials](docs/schematic.md) · [Examples](docs/examples.md) · [Documentation](docs/index.md)

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
  Query rows with [scale, offset and containment transforms](docs/guides/storage.md#expression-transforms),
  [shared percentiles, box summaries and bounded preview envelopes](docs/guides/storage.md#statistics-transforms).
  Retain [scope-qualified evidence cuts across restarts](docs/guides/storage.md#object-and-scoped-deletion)
  and observe compound writes and columnar sample appends at complete watch frontiers.
  Run [storage operations during preparation](docs/guides/storage.md#storage-before-full-startup) or offline maintenance before full startup.
- **Crash Recovery**: Reconstruct host state and undo history from bounded snapshot segments and journal replay; retain plugin history with its written definitions.
- **WebAssembly Guests** [`plugin-guest`]: Extend an application with grants, fuel and memory limits enforced by the Wasmtime boundary.
- **gRPC Plugin Placement**: Invoke managed or remote gRPC plugins, capture their streams and drive plugin waits and observers from the host clock or [a scoped unit's clock](docs/project/architecture.md#scoped-plugin-bindings).
- **Explicit Visibility**: Every operation call is scheduled by the host. Private operations admit their owning plugin, Protected operations remain host/owner callable, and Public operations admit declared peers and clients.
- **[Plugin Management](docs/guides/plugin-manager.md)**: Persist settings and revisioned plugin recovery state, and manage native or WebAssembly plugin entries.
- **Access and Host Status**: Compose library-owned TLS listeners, durable pairings, live trust rotation, contributed health and redacted diagnostics; see [Access and Host Status](docs/guides/access-status.md).
- **gRPC Interfaces** [`grpc`]: Declare product wrappers over native operations and generate product client contracts.
- **Simulation** [`simulation`]: Exercise supported workflows with repeatable seeds.

Plugin setting declarations carry their type in the default value's discriminator and
support inclusive bounds, allowed choices and required values. The host owns declaration validation and validates settings at admission. Saving invalid settings
returns an actionable InvalidArgument error naming the setting. Observer sampling demand
is acknowledged separately from the sample carrier: start and stop hooks acquire and
release resources on the first and last demand, and run again for the next sampling period.
Rust and C++ expose `on_start` / `on_stop`, Python uses observer decorator arguments of
the same names, and C# uses `onStart` / `onStop` registration arguments.


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

The native and guest walkthroughs run end to end, including library-stored typed
settings through their plugin entries; see [Schematic](docs/schematic.md).

### Python

Install `harmos-sdk` (import `harmos`) for plugin carrier helpers. Products own their author-facing SDKs and generated clients.
Applications generate their own protobuf messages and own connection and authentication.

gRPC plugin contexts call operations by identifier with a JSON object. The host resolves the
served Protobuf contract and rejects unknown fields. Replies use Protobuf field names
(snake_case), include default-valued fields and represent absent optional fields as null.
Requests and replies are JSON objects; a message with a scalar JSON mapping uses
`{"value": ...}` instead of a scalar root.
The same invocation owns admission, retries, cancellation and deadlines:

```python
reply = await ctx.operation("example.radio.set", {"radio": "wifi", "state": "off"})
```

Python has one public generic operation call. Typed host resources remain available through
`ctx.host`.
Rust keeps typed request values and accepts `ctx.operation(harmos::untyped(id, payload))`
for `serde_json::Value` replies. C# offers `ctx.Operation(id, payload)` returning `JsonNode`
alongside typed declarations; the invocation's identifier is `OperationId`.
C++ offers `ctx.operation(id, harmos::json(text))` with Protobuf `Struct` objects.
All four SDKs take and return JSON objects for identifier-based calls.
Typed and identifier-based calls share `operation` in Rust and C++, `Operation` in C#,
and `operation` in Python. Rust accepts either a typed request or `untyped(id, payload)`.

Operation failures expose kind, code, message, action, operation and retry evidence
(`retryable`, `attempts`, `max_attempts`, `next_after_s`). Rust returns `Err(Error)`;
Python raises `OperationError`, C# throws `OperationException`, and C++20 throws
`harmos::OperationError`. Plugins can register one failure mapper; an unmapped
handler error becomes Permanent and retains the exception text and handler identity.


#### Examples

The [Python SDK Guide](docs/sdks/python.md) covers typed clients, product roots and
the runnable [client](sdk/python/examples/client.py) and
[plugin](sdk/python/examples/plugin.py) examples. Keep generated clients matched
to the application revision that produced them.

### Rust

Plugin carriers and generated product clients use `harmos` for shared messages,
connection plumbing and internal authoring helpers. Products generate client resources
from their own descriptors; Harmos has no host-client resource root. See the
[Schematic client](examples/schematic/protocol/examples/client.rs) and the
[reference product fixture](tools/conformance/src/product.rs). Plugin contexts keep
time, own settings, own invocation controls and admitted discovery on the plugin carrier.
The [served plugin example](sdk/rust/examples/plugin.rs) declares a typed setting default
and reads its calling entry through `Context::settings`. With managed recovery storage,
`Context::state()` supplies a typed protobuf handle: declare `PluginsStateGet` and
`PluginsStatePut`, call `get::<T>()`, then `put(&intent)` before an external effect.
Put compares the observed revision and returns only after durable journal flush.
After undoing the effect, `clear()` removes its stored intent while retaining the
revision; entry removal retires the state. The host configures `PluginManagerSettings.state.max_bytes`.
Host applications depend explicitly on `harmos-runtime` for state, storage,
managed or remote gRPC placement and optional guest hosting.
Assemble with `Builder::new(origin)`, register catalogs, then call `start()` and
end with `Runtime::stop()`. `builder.run(stop_request)` starts, waits for the
product's stop-request future and stops.
See the [product guide](docs/index.md#cargo-features) for crate boundaries.
Native calls bind `runtime.for_call(&ctx)`, then select `storage()`, `streams()`,
`plugins()`, `time()`, `settings()`, `status()` or `invocations()`. Each call uses its registered
operation policy and retains typed payloads without encoding. See the
[product guide](docs/index.md) for call contexts and storage-only assembly.

Host operations, state catalogs and record registries are values; data shapes use
field-reading derives. See [Native Operation Values](docs/index.md#native-operation-values).

Portable messages support standard Rust maps with deterministic key ordering
and bounded decoding, plus root optional payloads that preserve value presence. The [message guide](docs/index.md)
describes their repeated key/value entry representation.

`harmos-storage` ships a process-lifetime memory backend, manager and backend contracts.
A runtime selects memory storage automatically unless setup registers a product backend.
`harmos-query` executes Arrow scans, transforms, time merges and reductions below storage;
products register their own durable primary and recovery backends.
See [Storage Interfaces](docs/guides/storage.md) for native APIs, backend obligations, bounded pages and retained-cut query examples.
Applications expose their own wire operations over private native storage.
Applications can expose resumable unary file productions and bounded content ranges through product operations.
Hosts can register [application-owned immutable content](docs/guides/storage.md#application-owned-content)
for those ranged reads without copying it into library persistence.

Structured operation and storage tracing supports performance investigation;
applications control collection. See [Performance](docs/index.md#performance).

#### Examples

The [Schematic walkthrough](examples/schematic/cli/src/walkthrough.rs) covers
editing, plugin validation, undo/redo and save/reopen. Its
[document Save/Open](examples/schematic/app/src/document.rs) shows application-owned serialization,
while its runtime stories use the public memory adapter.
Its native plugin serves unary advice over gRPC; analysis and transfer streams run
in the host. The guest uses SDK builders with `harmos/wasm` and exposes unary transfer endpoints under its `benchmark` feature
for the [performance suites](docs/index.md#performance).

### C++

The C++20 SDK serves gRPC plugins with protobuf messages and builder values for
operations, streams and observers. See the [C++ SDK guide](docs/project/tooling.md#c-plugin-sdk)
for toolchain setup and managed or remote serving.

Clients take request content directly, own their transport context and project single-field
responses. Products generate their own client roots and keep wire packages internal;
Harmos retains the carrier and connection helpers used by those SDKs.
See [SDK Call Contracts](docs/project/architecture.md#sdk-call-contracts).

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
Every SDK includes an importable fixture for [layered SDK conformance](docs/project/tooling.md#conformance-for-a-layered-sdk).

[Releases](https://github.com/stokker-technologies/harmos/releases) attach
`harmos-proto-<version>.zip`, one `harmos-conformance-<version>-<platform>.zip|tar.gz`
per platform, `harmos-compliance-<version>.zip`, `harmos-docs-<version>.zip`,
`release.json` and `SHA256SUMS`.
The changelog is the release body; crates and language SDKs use their registries.
The mutable [Nightly](https://github.com/stokker-technologies/harmos/releases/tag/nightly)
adds `harmos-reports-<version>.zip`, fixed-name `benchmarks.json` and the
documentation archive when its independent build succeeds. Its notes record
coverage, benchmark regressions, suite results and the workflow link; failed suites
can accompany a published build.

Harmos is pre-1.0; public APIs and plugin contracts can change. Keep application
and plugin revisions aligned and review release notes before updating. Stored data
formats have no migrations. Managed recovery refuses changed host state types;
choose a fresh storage root for those product changes. Plugin history retains its
written definitions across shape changes and plugin removal. Wire compatibility
versions remain explicit.

Built-in operation contracts live in `harmos-operations`. Hosts register the explicit
`host_catalog()`, `lifecycle_catalog()`, `storage_catalog()`, `streams_catalog()` or
`builtins_catalog()` exports under `harmos_runtime::catalog`. Native storage request
and result types are exported under `harmos_runtime::facade::storage`.

## Contributing

Focused fixes, API feedback and useful examples are welcome. Read
[Contributing](docs/CONTRIBUTING.md) for setup, validation and pull requests.
Read [Architecture](docs/project/architecture.md) before proposing architecture or public API changes.
Run targeted tasks locally; hosted CI and Nightly provide full validation.
Tool configuration lives in `tools/config/`; see [Repository Tooling](docs/project/tooling.md).

## About

Harmos is developed by [Stokker Technologies](https://stokkertechnologies.ch)
as a foundation for stateful applications, engineering workflows and managed
plugins. It is available under [Apache-2.0](LICENSE).
