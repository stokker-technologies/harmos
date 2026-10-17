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

- **Services And Observers**

  Run resident background work as operations that stay up until stopped:
  services restart under a declared policy, observers tick at their declared
  interval. Keep device handles and live clients with the work that owns them.

- **Streams**

  Publish typed measurements with demand-driven production, bounded windows,
  capture, and explicit gap reporting. Samples retain their application-state
  context without adding every measurement to the state journal; dedicated
  [benchmarks](#performance) cover batching and native consumption.

- **Storage**

  Supply adapters for committed history, managed-state journals and snapshots,
  streams, and evidence. Your application chooses its database and file format
  and explicitly supplies storage budgets, queue limits and response batch
  ceilings. Analytical query results use Arrow schemas and record batches.

- **Crash Recovery**

  Reconstruct state from persisted history and snapshots, including the history
  needed for undo. Recovery follows the storage adapter's durability policy;
  an accepted change is not necessarily durable until the adapter stores it.

- **Guests**

  Extend your application with sandboxed WebAssembly components. The host
  controls admitted grants and execution budgets, including fuel and
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
directory. The guest has no filesystem access and omits that operation.

The [advanced follow-on](examples/schematic/cli/src/advanced.rs) demonstrates
bounded streaming, cancellation with settlement, and native directory access.
The [simulation test](examples/schematic/app/tests/app/simulation.rs) checks
repeatability, replay, and inverses. Reusable [schematic storage example](examples/schematic/storage/)
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
| `grpc` | gRPC, reflection, TLS, and Web support; settings select their use. Includes `reflection`. |
| `reflection` | Descriptor projection used by runtime transport. |
| `signal` | Signal-processing kernels. |
| `signal-fft` | FFT support; includes `signal`. |
| `simulation` | Deterministic runtime simulation. |

Applications compose the runtime through `harmos`. Shared domain declarations,
plugins and portable clients use `harmos-domain`; native plugins add
`harmos-sidecar` and guests add `harmos-guest`. Build scripts and development tools
use `harmos-packaging`, which depends on the domain without hosting engines.

```toml
[build-dependencies]
harmos-domain = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>" }
harmos-packaging = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>", features = ["bindings-proto"] }
```

Generators use one entry point per format: `harmos_packaging::proto::generate`,
`python::generate` and `typescript::generate`, gated by `bindings-proto`,
`bindings-python` and `bindings-typescript`. Protobuf output goes to the exact
selected output directory. A producer's build script selects its current
`OUT_DIR/grpc/<catalog>/`; packaging retains that producer/catalog identity.
The same crate owns `bundle`, Cargo discovery, inert inspection, development
launching and Wasm componentization.

```sh
cargo run --locked -p cargo-harmos -- harmos sidecar build -p schematic-sidecar --json
cargo run --locked -p cargo-harmos -- harmos sidecar package -p schematic-sidecar --json
cargo run --locked -p cargo-harmos -- harmos guest build -p schematic-guest --json
cargo run --locked -p cargo-harmos -- harmos sidecar inspect target/debug/schematic-sidecar --json
```

Install the native GitHub Release archive for your Rust target triple by
extracting its `cargo-harmos-<version>-<target>/` directory and placing
`cargo-harmos` (`cargo-harmos.exe` on Windows) on `PATH`. Run `cargo harmos --help`
to check installation. Releases contain the tool, README and Apache license;
examples and test reports are downloadable CI artifacts. Cargo registry
publication is deferred.

`cargo harmos sidecar build` prepares native plugins; `cargo harmos guest build`
drives the selected Wasm build and converts guest core modules
into inspected `.component.wasm` files beside the selected module, and reports
exact artifact and schema paths. It keeps generated components separate from
Cargo's original modules. Each backend's `dev` command watches sources and local
Cargo dependencies, coalesces changes and rebuilds serially. Failed builds keep
the last usable artifact until preparation succeeds. Generated output directories
are excluded; Ctrl+C stops watching and settles the active Cargo process tree.
`dev --json` emits `started`, `built` and `failed` records with attempt numbers.
`inspect` reads metadata without executing the plugin;
`--json` provides machine-readable output. Producer build scripts
export known catalogs to their own `OUT_DIR/grpc/<catalog>/`; build output reports
those exact directories. Schematic bundles use package `schematic.v1` and service
`Schematic`. Schema generation does not start a gRPC server. Applications configure
and start their runtime transport separately; dynamic plugins appear in runtime
reflection when their operations become ready.

The packaging crate's `bindings-python` feature exposes
`harmos_packaging::python::generate`. It emits a producer-owned `src/` Python
package with standard protobuf messages, `.pyi` stubs, `py.typed`, canonical
contracts and descriptors. Producer build scripts export contracts normally;
explicit binding generation needs `protoc`, while installing the resulting
wheel or sdist needs Python 3.13+ and its declared protobuf dependency.
Reserved protobuf message members use a trailing underscore (`Clear → Clear_`)
in static and dynamic classes. Original field tags, finite graph names and JSON
names remain unchanged. Map graph fields through `DESCRIPTOR.fields_by_number[tag]`;
collisions after escaping are rejected.

```sh
cargo harmos sidecar build -p schematic-sidecar --json > build.json
cargo harmos bindings python --build-json build.json \
  --producer schematic-sidecar --catalog operations \
  --distribution schematic-bindings --module schematic_bindings \
  --package-version 0.2.0 --description "Schematic Operation Bindings" \
  --license Apache-2.0 --output generated/schematic-bindings
python -m pip install ./generated/schematic-bindings
```

The generator selects that exact producer/catalog receipt, or accepts an exact
`--export` directory. `--operation` narrows the export by operation identity;
selected contracts without protobuf bindings and generated name collisions
fail explicitly. It creates a new output directory and preserves existing
outputs. Each package owns a private descriptor pool, so separate producers can
reuse protobuf names without sharing message-class identity.

Generated `Operation` descriptors expose identity, semantic version, typed
request/response and stream-item classes, RPC envelopes and path, and the
canonical contract fingerprint. Structural `Invoker` and `ResponseStream`
interfaces let a consumer supply its transport; output streams retain the
operation's declared final response. Connections, cancellation and operation
registration remain with that consumer.

Message descriptions use finite shared definitions and references, including
recursive shapes. `config::SchemaSettings` bounds schema preparation memory and
work separately from payload limits. Artifact metadata uses revision 6; rebuild
hosts and SDK consumers together. Stored schema metadata uses `graph_v1`;
Schema writes retain the finite schema description; finite schema reads use
`Query` with `Source::Schemas`. Product row payload codecs remain explicit. Inline
schema metadata is unsupported: preserve the original data and select a fresh
storage root rather than resetting or silently converting it. Backends can reserve
JSON schema decode memory with `StorageSchema::decoding_memory_bytes(encoded_bytes)`
before decoding; add their own entry bookkeeping and reject `None` on overflow.

Native managed calls return `operations::Response<T>`, which dereferences to the
original declared payload. Response clones and cache entries share its owner.
Stream producers use `output.send_owned(response).await?` to retain the same owner
through queueing and encoding; ordinary stream payloads keep their existing `Send`
contract. Protobuf payloads and RPC paths remain unchanged.

Harmos serves no built-in operation by default. `builtin::Catalog::register`
serves `host.operations.status`, `host.operations.cancel` and
`host.operations.watch` (gRPC `/harmos.host.v1.Operations/Status`, `/Cancel`
and the server stream `/Watch`) over the manager's instances. Each answers only
for active instances the caller submitted or holds a resident lease on; unknown,
foreign and settled ids are refused alike. A watch streams every status change
from the moment it starts until the instance stops or settles. Plugins reach them like any other operation,
so they must declare them as requirements.

Hosts can register `operations::discovery::Discovery` in their existing operation
manager to serve `host.operations` (unary gRPC path
`/harmos.host.v1.Host/Operations` when `grpc` is enabled). `host::Operations` accepts optional `after`,
`rows` and `bytes`; defaults are 64 rows and 64 KiB, with maxima of 256 rows and
4 MiB. `OperationsResponse::catalog` is a UTF-8 JSON array of
`host::DiscoveredOperation`, containing original contracts, provider owners and
generations, selected grants and current gRPC availability. The `next` cursor
continues a bounded page. Discovery applies original readiness and caller
visibility/dependency rules; `conditional_access` means invocation still checks
request-specific grants and middleware. Re-discover after provider reload or
runtime restart; pages do not promise a frozen registry snapshot.

Register one `operations::authorization::Authorizer` with the original operation
manager before external calls. It selects a compatible canonical `Required` from
trusted `CallContext` and live provider metadata; discovery and actual request
grants use the same selection. Application policy owns roles and entitlements.
Registration freezes at first external access, allowing earlier internal boot
work. Internal host calls retain their authority; external calls and descendants
retain their authenticated context and separate cache identity. Existing
explicitly anonymous routes keep their behavior when no authorizer is configured.

For browser gRPC-Web clients, enable `GrpcSettings::web` and list exact
`grpc::Origin` values in `allowed_origins`. Configured origins receive preflight
and response headers; other origins are refused. Authentication still applies
to actual calls. An empty origin list emits no CORS headers.

Detaching or shutting down a gRPC listener ends in-flight calls with `Unavailable`
and closes its connections gracefully; connections that cannot drain, such as
unread streams, are aborted after half of `shutdown_timeout`. The application
runtime stays available for a new attachment. Accepted sockets
use TCP_NODELAY so small flow-control windows do not add Nagle buffering delays.

Storage is configured by three explicit groups, with no defaults:

```rust
let settings = StorageSettings {
    budget: StorageBudget { read_memory_bytes },
    request: StorageRequestSettings {
        max_pending_queries,
        max_active_queries,
        max_pending_write_batches,
        max_pending_write_bytes,
    },
    response: StorageResponseSettings { batch_rows, batch_bytes },
};
let storage = StorageManager::new(StorageBackends { primary, state }, settings)?;
```

Finite storage queries select a source at a full persisted storage cursor,
apply an ordered transform plan, and apply row `offset`/`limit` to the final
result. Delivery emits a schema, bounded Arrow record batches, and a final
operation completion. Response `batch_rows` and `batch_bytes` bound each delivery
batch; they do not truncate the query result. In process, the manager's reader
yields native `QueryArrow` items whose shared Arrow buffers keep their storage
charge through clones and slices. The `storage.query` operation, declared in
`harmos-domain`, carries the same items as Arrow IPC (`QueryOutput`), and the
write operations take `DataBatch` values whose data is Arrow IPC; serve them with
`harmos::storage::operations::register`.

```rust
let mut reader = storage.query(Query { at, source, transforms, slice }).await?;
while let Some(output) = reader.next().await? {
    // One QueryArrow::Schema, then QueryArrow::Batch values.
}
let completion = reader.completion()?;
```

Response batches are bounded to half the shared read budget, leaving room for
read descriptions and encoding work. Waiting for a query slot holds no read bytes.

Every read shares the one read budget. Work that cannot fit while other queries
hold memory waits for released capacity; work that can never fit the budget, even
after splitting source pages down to one row, fails with `ResourceExhausted`
instead of waiting on itself. A reader that already holds memory, including
batches its consumer retains, waits to grow unless no waiting reader could grow
even after every other owner released everything; only that certain cycle fails
with `ResourceExhausted`. Dropping a reader or an in-progress `next` cancels
it and releases its query slot; batches already returned stay charged until
they are dropped.

A stream selection's `after` resumes at a stored position, as `RecordRead::after`
does: only rows stored after it are read, in position order.

Typed historical reads decode saved objects and records from their exact stored
bytes in pages bounded by the response settings, so payloads never expand to
Arrow and object size is not limited by the Arrow field bound. Narrow envelope
fields through backend selections; apply residual conditions to decoded values
in the consumer. Use `Query` for derived columns. A schema that was never
written reads as no rows, whereas a `Query` over it is refused because it has no
schema to emit.

```rust
let mut reader = storage
    .read_objects(ObjectRead::<Experiment> {
        at: at.clone(),
        key: None,
        versions: ObjectVersions::Latest,
    })
    .await?;
while let Some(page) = reader.next().await? {
    // Vec<ObjectRevision<Experiment>>
}
let records = storage
    .read_records::<Reading>(RecordRead { at, selection, after: 0 })
    .await?
    .collect()
    .await?; // Vec<StoredRecord<Reading>>
```

`harmos::filesystem::input::read(path, limit)` bounds regular-file reads.
`harmos::filesystem::publication` owns staged atomic installation, replacement,
removal and explicit durability confirmation across Unix and Windows.

Stored objects can enforce domain invariants through `StoredObject::validate`;
object encoding, decoding and catalog validation apply the same check.

`storage::query::Accumulator` folds scalars over rows a caller already holds
with the same semantics as query aggregates: exact rational `Sum`, `Mean` and
`Median`, ordering for `Min`, `Max` and `Last`, and signal kernels for floating
statistics. Snapshot reads run concurrently; closing storage waits for those in
flight.

`StorageManager::read_file(ReadFile { file, range })` streams a byte range of a
finished, immutable file in response-sized batches. `FollowRecords`,
`FollowStream`, `FollowObjects` and `FollowFile` are the live subscriptions: each delivers saved
history after its `after` cursor, then newly accepted entries.

Mutation receipts make retried file, schema and delete mutations idempotent within
the retry window. The receipt ledger takes a fixed share of the read budget;
when it is full, completed receipts are evicted oldest first, and a retry of an
evicted receipt runs as a new mutation.

Typed stream, record and object writers acknowledge acceptance and durability
separately for each atomic logical batch. A disconnected invocation may have
committed earlier batches; callers must not blindly replay unacknowledged data.
Finished-file reads are separate from producing-file follow/resume operations.
The built-in `storage::state::file::FileBackend` persists checksummed JSON checkpoints
and a JSONL journal, bounded by the storage batch ceiling.

State persistence uses `storage::state::StateBackend`; the state manager owns
transactions and replay while the storage manager owns admission and durability.

A retained, connected `StorageFile` stays live during idle periods under its
original producer owner. Dropping the producer or cancelling its pending write
starts a bounded resume window;
recovered sessions have the same bounded window. Token-only mutation
producers retain bounded inactivity expiry because no native handle owns their
lifetime. An unadopted native resume remains disconnected. Resume fences the old
producer and clears the disconnected deadline. Finish, abort and storage shutdown
still retire production; existing file-count and memory budgets remain enforced.
A viewer owns its subscription, while the original service owns production.

An operation is its request type. One attribute on the request struct declares
it; a handler names nothing, because its `run` request type is the operation:

```rust,ignore
#[harmos::message]
#[harmos::operation(id = "gauge.read", version = 1, policy = POLICY, response = Reading,
    visibility = harmos::operations::Visibility::Shared)]
pub struct Read {
    pub channel: u32,
}

#[harmos::operation]
impl Gauging {
    async fn initialize(gauge: Arc<Gauge>, _: Context<'_>) -> Self { Self { gauge } }
    async fn run(&mut self, _: Context<'_>, request: &mut Read) -> Result<Reading, Error> {
        self.gauge.read(request.channel).await
    }
}
```

Streaming operations add `input = Item` or `output = Item`. Two operations with
the same payload shape declare two request types.

One `operations::Host` serves operation bodies wherever they run: in
process over `OperationManager` and `operations::Context`, in a sidecar over
`Supervisor`, and in a guest over `Context`. `call` and `submit` take the
request, which names its operation. A submitted
`OperationInstance` reports `status`, and supports `cancel`, `join` and
`cancel_and_join`. Dropping an unsettled instance cancels it; `detach` keeps it
running without a handle until its submitting attempt ends, which cancels and
settles it like any other child.

```rust,ignore
use harmos::operations::{Host, Instance};

async fn sample<H: Host>(host: &H) -> Result<u64, H::Error> {
    let reading = host.call(Read { channel: 3 }).await?;
    let pending = host.submit(Calibrate::default()).await?;
    let status = pending.status().await?;
    let calibrated = pending.join().await?;
    host.publish_on::<Progress>(&Progress { percent: 100 }).await?;
    Ok(reading.value + calibrated.offset + u64::from(status.attempt))
}
```

In process, the inherent `submit` admits synchronously. Sidecars
and guests submit unary children; streaming child sessions (`upload`,
`download`, `exchange`) remain in-process for now.

A plugin operation that fails reaches its caller as one `harmos::Failure`:
a `code` (`operations::FailureKind`), a `cause` written once where the handler's error
originated (the deepest message of its source chain), the operation frame the
host adds where the call crosses from the plugin, and optional origin
diagnostics holding the full chain. Sidecar frames, host refusals, guest
records and gRPC status details carry the same value, and every error layer
prints only its own line, so walking `source()` renders
"operation `gauge.read` failed: connection reset" once.

```rust,ignore
match host.operations.call(request).await {
    Err(error) => {
        if let Some(failure) = error.failure() {
            tracing::warn!(
                code = ?failure.code(),
                cause = failure.cause(),
                operation = failure.context().operation(),
                "plugin operation failed",
            );
        }
    }
    Ok(reading) => record(reading),
}
// A gRPC client reads the same value from the status details.
let failure = harmos::Failure::decode(status.details())?;
```

Operations derive child context with `with_value` for a declared
`ContextValue`; every binding borrows its handle and returns the derived one.
The child keeps the original caller, grants, cancellation and deadline while
receiving the selected portable value. Read portable values with
`CallContext::value`; overlays retain the 64-value, 64 KiB context budget.

Operations, observers, services and plugins share lifecycle verbs. An
operation's `async fn initialize(resources, context) -> Self` constructs it on
its first attempt; queued instances cancelled before they start and cache hits
construct nothing. `run` (operations, services) and `observe` (observers) do the
work, `stop` ends domain activity, and `cleanup` releases physical resources
after a successful initialization. Observers initialize from `&Resources`;
plugins keep `prepare` and `initialize(&mut self, context)` and end with `stop`
and `cleanup`.

Observers and services are resident operations of the same `OperationManager`:
request-scoped operations settle, residents run until stopped. Register them
with `register_observer` or `register_service` and activate them with
`submit_with_resources`; activations over the same resources share one instance,
each `OperationInstance` holds one lease on it, and releasing or dropping the
last lease stops it through `stop` and `cleanup`. `cancel` stops it for every
holder. Residents report `Restarting` while a service backs off and end in
`Stopped`, through the same status, `host.operations.status` and
`host.operations.watch`. A service's context is the same `Host` as any other
body and lends `records`, `state`, `streams` and `evidence`. Sidecar and guest
bindings cannot lend process-local resources, so they refuse
`submit_with_resources`.

```rust,ignore
use harmos::operations::{Backoff, Jitter, OperationPolicy, RestartPolicy};
use harmos::services::{POLICY, ServiceDefinition};

// `timeout` is a resident's stop deadline; `restart` is the resident half.
let policy = OperationPolicy {
    timeout: Duration::from_secs(2),
    restart: RestartPolicy::OnTransient {
        credit: 3,
        sustained_health: Duration::from_secs(30),
        backoff: Backoff::Exponential {
            initial: Duration::from_millis(250),
            maximum: Duration::from_secs(10),
            jitter: Jitter::Deterministic,
        },
    },
    ..POLICY
};
runtime.operations.register_service(
    ServiceDefinition { id: "poller".into(), policy },
    |device: Arc<Device>| Poller::new(device),
)?;
let poller = runtime.operations.submit_with_resources_at("poller", device).await?;
poller.ready().await?;
```

Request-scoped declarations keep `restart: RestartPolicy::Never`; SDK lifecycle
operations and listener callbacks refuse any other value. A sidecar that declares
observers serves the built-in `harmos.observe` control; the host submits it per
activation and cancels it to stop the observer, which runs on a thread of its own.

Externally supplied settings and defaults live in each configurable crate's root
`config.rs`. Composition passes each owner its relevant settings rather than
retaining global configuration in every manager.

Host runtimes expose one shared `runtime.plugin` owner; `runtime.handle().plugin`
clones that same owner. Embedded, guest and native registration is inert. Explicit
`initialize()` starts the dependency graph, and runtime shutdown retires plugins
before closing their operation/state infrastructure. The schematic example uses
this owner for both plugin backends.

```rust,ignore
use harmos::config::{PluginRestartSettings, PluginSidecarSettings};

let settings = PluginSidecarSettings {
    plugin: harmos::config::PluginSettings {
        restart: PluginRestartSettings {
            retries: 2,
            ..Default::default()
        },
        ..Default::default()
    },
    ..Default::default()
};
runtime.plugin.register_sidecar::<Operations>(executable, settings, lane).await?;
runtime.plugin.initialize().await?;
```

Automatic restarts default to zero. A configured allowance applies to each
explicit activation cycle, including crashes after readiness; successful startup
does not replenish it. Status retains the lifetime startup count, last failure
class, retry deadline and cleanup state, including stopping while the original
retirement owner is still settling. Invalid configuration/contracts are terminal. Explicit stop cancels restarts for the plugin and its dependents;
failed cleanup prevents a fresh physical attempt. Plugin restart never replays
uncertain operation calls or streaming transfers. Guest `shutdown_timeout` bounds
the complete stop wait, including SDK settlement, authored shutdown and physical
finalization. A timeout retains the same invocation and cleanup owners; retrying
stop resumes settlement without launching another plugin attempt. `reload(id)`
restarts a frozen definition and its active dependents; `reload_sidecar`, `reload_guest` and
`reload_embedded` accept replacement inputs through the same validation and
retirement owner. Caller abandonment, explicit stop and runtime shutdown interrupt
reload initialization and fence each new generation; cleanup retains ownership
until settlement. A committed replacement remains the source for later explicit
activation. `remove(id)` requires registered dependents to be removed first.

Rust-only embedded catalogs use `register_embedded` and `reload_embedded`.
`register_embedded_wire` and `reload_embedded_wire` additionally project the
handler catalog through `AuthoredContracts`, register its portable codecs and
expose declared gRPC methods when enabled. Exact requirements and grants apply
through the same dependency graph and admission owner; no SDK host is needed.
Native SDK callers can require those embedded contracts through
`PluginSettings::requires`. Reloading their provider also restarts active
dependents; unrelated guest generations remain running.
Encoded calls require an explicit `operations::Settings::preparation_bytes`
budget.

`PluginManager` remains the lower-level extension for applications supplying their own backend lifecycle.

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
- [Storage Adapter](examples/schematic/storage/) — own your persistence format.

## Project Status & Roadmap

Harmos is in active development. Pin the revision you integrate and expect
changes to public APIs and plugin contracts before 1.0.

Reproducible published stream and plugin-transport results remain a documentation
milestone. GitHub hosts the
authoritative repository and native CI; GitLab retains a one-way mirror.

Follow [issues](https://github.com/stokker-technologies/harmos/issues)
for development discussions and [the changelog](CHANGELOG.md) for recorded changes.

Dynamic clients consume discovered canonical contracts through
`harmos::grpc::dynamic::Operation` or a generated Python package's
`dynamic.from_contract`. Both project the same finite graph and streaming envelopes.
The consuming application owns authentication, connections, metadata and cancellation.

## Development Setup

Use the setup commands in [Quick Start](#quick-start), then choose a task:

```sh
mise run run:schematic:sidecar # Run the native plugin example
mise run run:schematic:guest # Run the sandboxed Wasm example
mise run check                  # Run repository checks
mise run test                   # Run the test suite
mise run bench                  # Record benchmark results
```

Every proposed integration SHA runs fresh checks, release builds and full tests,
including squash, merge and history-reset commits with identical trees. `dev`
requires Linux validation and its separate `development.json` artifact proof.
Windows runs independently with visible outcomes and remains advisory for `dev`
until its requirement is explicitly restored. `main` and releases always require
both Linux and Windows. Coverage is an optional diagnostic through
`STKR_COVERAGE=1 mise run test`; ordinary validation keeps the full selection.
Dev pushes may use the restricted local Linux runner. PR full validation requires
approval; integration into `dev` and `main`
starts automatic validation of the resulting SHA. Caches accelerate those builds.
Installed tools, dependency downloads and profile-specific compiler outputs use
separate caches. Preparation jobs save them; test consumers restore them.
Cargo verifies restored compilation inputs, and package hashes and provenance
remain independently verified.
`mise run packages:reuse` retrieves successful artifacts only for the current SHA,
with matching inputs and intact hashes; it never rebuilds or changes provenance.
Use `HARMOS_VALIDATION_RUN_ID` to select an explicit same-SHA Actions run.
The native validation pipeline runs source checks first, then compiled checks and
release builds on the same runner. Local `mise run check` retains both check groups.
Native Rust integration tests use the release profile without instrumentation.
Coverage is an optional diagnostic through `STKR_COVERAGE=1 mise run test`; it
preserves the ordinary test selection. Feature pushes run source checks through
`mise run ci:feature`. Compiled checks and full correctness tests run in the
approved native validation workflow. The full-validation gate requires both native
platforms; the temporary dev gate verifies Linux independently. Dev pushes may
use the restricted local Linux runner; other refs use hosted runners.

Package restoration verifies transferred bytes in a temporary directory before
replacing the staged package. Failed verification preserves existing payloads;
symlink destinations are refused.

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
