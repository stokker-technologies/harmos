# Harmos Product Guide

Detailed composition, feature selection and runnable examples. Start with the [Quick Start](../README.md#quick-start).

## Examples

The [Schematic Editor](../examples/schematic/README.md) is one application with two
plugin backends. Read the [walkthrough](../examples/schematic/cli/src/walkthrough.rs)
from top to bottom: place two symbols, connect them, ask the plugin to validate
and propose a route, apply the proposal as a host transaction, undo/redo, and
save/reopen the document.

```sh
target/build/dev/linux-x86_64/bin/schematic-cli
target/build/dev/linux-x86_64/bin/schematic-cli --guest
```

`mise run build` builds the workspace and the example's CLI, sidecar and guest
component once. Without a flag the CLI loads the sidecar beside it. Both plugins implement `ValidateSchematic`, `ProposeRoute`, and the streaming
`AnalyzeSchematic`. The native sidecar additionally provides client-streaming
`AnalyzeUpload` and bidirectional `AnalyzeExchange`, with typed progress
publications. Its `ListDirectories` operation lists immediate subdirectories of
an explicitly configured working directory. The guest omits native filesystem access.

The [advanced follow-on](../examples/schematic/cli/src/advanced.rs) demonstrates
bounded streaming, cancellation with settlement, and native directory access.
The example's [JSONL storage adapter](../examples/schematic/storage/) shows how an
application owns its persistence format. The example is a separate Cargo workspace
under `examples/schematic/` and is also the system fixture of the Harmos test suite.

## Cargo Features

The default facade exposes portable models and authoring macros. Enable `runtime`
for SDK execution on all targets, plus managed state, streams, storage and recovery on native targets. Other execution
features imply `runtime`.

| Feature | Enables |
| --- | --- |
| `runtime` | SDK lifecycle, registration and host runtime facilities. |
| `plugin` | Shared plugin management, activation, routing, and reload. |
| `plugin-sidecar` | Native process hosting; includes `plugin`. |
| `plugin-guest` | Sandboxed Wasmtime hosting; includes `plugin`. |
| `grpc` | gRPC, reflection, TLS, and Web support; settings select their use. Includes `generate-proto`. |
| `generate-proto` | Protobuf descriptor projection used by runtime transport. |
| `signal` | Signal-processing kernels. |
| `signal-fft` | FFT support; includes `signal`. |
| `simulation` | Deterministic runtime simulation. |

The `harmos` facade composes `harmos-domain` and `harmos-runtime`.
Applications compose the runtime through `harmos`. Shared portable declarations use `harmos-domain`. Async plugin and client execution
uses `harmos-sdk`; native plugins add
`harmos-sidecar` and guests add `harmos-guest`. Build scripts and development tools
use `harmos-packaging`, which depends on the domain without hosting engines.
`harmos-storage` owns the Arrow query and persistence engine plus signal kernels;
`harmos-runtime` composes that engine with host lifecycle and managed state.

```toml
[build-dependencies]
harmos-domain = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>" }
harmos-packaging = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>", features = ["generate-proto"] }
```

Generators use one entry point per format: `harmos_packaging::proto::generate`
and `python::generate`, gated by `generate-proto` and `generate-python`.
TypeScript consumers generate their bindings from the exported Protobuf files
with the official Buf generator after the Cargo build. Protobuf output goes to
the exact selected output directory. A producer's build script selects its current
`OUT_DIR/grpc/<catalog>/`; packaging retains that producer/catalog identity.
The same crate owns `bundle`, Cargo discovery, inert inspection, development
launching and Wasm componentization.

```sh
cargo run --locked -p cargo-harmos -- harmos sidecar build --manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --json > sidecar-build.json
cargo run --locked -p cargo-harmos -- harmos sidecar package --build-json sidecar-build.json --output target/packages --json
cargo run --locked -p cargo-harmos -- harmos guest build --manifest-path examples/schematic/Cargo.toml -p schematic-guest --json
cargo run --locked -p cargo-harmos -- harmos sidecar inspect examples/schematic/target/x86_64-unknown-linux-gnu/debug/schematic-sidecar --json
```

The inspection path above is Cargo's default Linux target directory for the
separate Schematic workspace. On other platforms or with a custom target
directory, use the artifact path reported in `sidecar-build.json`.

The [current release](https://github.com/stokker-technologies/harmos/releases/latest) provides raw
`cargo-harmos-<version>-<Rust-target-triple>` executables (`.exe` on Windows).
Download the matching asset, verify it against `SHA256SUMS`,
rename it to `cargo-harmos` (`cargo-harmos.exe` on Windows), and place it on
`PATH`. On Linux, make it executable. Run `cargo harmos --help` to check
installation. Mise installation is verified against those same published bytes
before release completion. Harmos library crates are published on crates.io;
the Cargo tool ships as a release executable.

The release also declares `schematic-<version>-<platform>.zip` demo bundles for
`linux-x86_64` and `windows-x86_64`. Extract the complete ZIP and run
`schematic-cli` (`schematic-cli.exe` on Windows); `--guest` selects the bundled
Wasm backend. Keep its `plugins/` directory beside the executable. Package
checks run both backends from an isolated extraction. Release downloads include
`SHA256SUMS` and `release.json`; test reports and fixtures remain CI artifacts.

`package` consumes the existing build receipt and verifies every artifact and
schema byte stamp before bundling. It never runs Cargo. Relocated archives use
explicit `--relocate-from` and `--relocate-to` path mapping while preserving hashes.
Library consumers can stamp existing outputs with `Artifact::read` and
`Schema::read`, then construct a `Build` without another compiler invocation.

Create a plugin from the maintained starter with an explicit source selection:

```sh
target/debug/cargo-harmos harmos new sidecar target/my-plugin --harmos-path .
# For a separately maintained project, select a published revision:
cargo harmos new guest my-plugin --harmos-rev <full-published-sha>
```

The starters include their managed Python runner and repository task graph.
Keep generated guidance and tooling current through the owning authoring tool.

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

The packaging crate's `generate-python` feature exposes
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
  --package-version 0.1.0 --description "Schematic Operation Bindings" \
  --license Apache-2.0 --output generated/schematic-bindings
python -m pip install ./generated/schematic-bindings
```

The generator selects that exact producer/catalog receipt, or accepts an exact
`--export` directory. `--operation` narrows the export by operation identity;
selected contracts without protobuf bindings and generated name collisions
fail explicitly. It creates a new output directory and preserves existing
outputs. Each package owns a private descriptor pool, so separate producers can
reuse protobuf names without sharing message-class identity.

Generated `Operation` descriptors expose identity, typed
request/response and stream-item classes, RPC envelopes and path, and the
canonical 32-byte contract fingerprint (64 lowercase hex characters in JSON),
with owner-qualified routing identity and a lossless u64 generation. Structural `Invoker` and `ResponseStream`
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

Harmos serves no built-in operation by default. `builtins::Lifecycle::registration(Dependencies::new())`
provides the Status, Cancel and Watch operations; register the batch on a lane (gRPC `/harmos.host.v1.Operations/Status`, `/Cancel`
and the server stream `/Watch`) over the manager's instances. Each answers only
for active instances the caller submitted or holds a resident lease on; unknown,
foreign and settled ids are refused alike. A watch streams every status change
from the moment it starts until the instance stops or settles. Plugins reach them like any other operation,
so they must declare them as requirements.

Hosts register the batch from `builtin::catalog_registration(storage)` on a
chosen lane to serve `builtins::host::Catalog` (unary gRPC path
`/harmos.host.v1.Host/Catalog` when `grpc` is enabled). `builtins::host::CatalogRequest` selects a
`kind` (`Operations`, `Schemas`, `Streams` or `Objects`) and accepts optional
`schema`, `after`, `rows` and `bytes`; defaults are 64 rows and 64 KiB, with
maxima of 256 rows and 4 MiB. `CatalogPage::entries` is a UTF-8 JSON array of the
kind's entries: operation metadata (original contracts, provider
owners and generations, selected grants and current gRPC availability), stored
schemas or stored identities. The `next` cursor continues a bounded page. Stored
kinds need the storage manager and a `host_catalog` grant scoped `schemas:*`,
`streams:*` or `objects:*` (`<kind>:schema:<id>` with `schema`); listing
operations needs no grant. The operation listing applies original readiness and caller
visibility/dependency rules; `conditional_access` means invocation still checks
request-specific grants and middleware. Re-discover after provider reload or
runtime restart; pages do not promise a frozen registry snapshot.

Register one `operations::authorization::Authorizer` with the original operation
manager before external calls. Its `authorize` method selects
`Authorization::Restricted(Required)` for exact grants or
`Authorization::Full(Contract)` for all scopes on that original operation;
`None` denies access. Both selections validate the live provider contract.
Full control still refuses a grant naming a different operation and cannot
override a calling plugin's own capability requirements. Trusted `CallContext`
and live provider metadata drive the same selection for discovery and actual
requests. `Provider.plugin` reports whether the original registration belongs
to a plugin scope; an arbitrary host owner name cannot establish that provenance.
Discovery lists finite selected grants without inventing wildcards;
invocation always checks the actual request. Application policy owns entitlements.
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

Several `grpc().attach(settings)` calls may serve the same operation registry
concurrently, each with independent TLS, connection/call limits and shutdown.
Retain each `Attached` handle and use its `local_addr()`; dropping or detaching
it ends only that listener. Runtime shutdown ends every attachment. The manager's
singular `local_addr()` returns an address only when exactly one listener is
active. Register shared services and middleware before attachment; they remain
frozen while any listener is active.

Storage uses explicit budget, request, response and filesystem settings, with no defaults:

```rust
let settings = StorageSettings {
    budget: StorageBudget { read_memory_bytes },
    request: StorageRequestSettings {
        max_pending_queries,
        max_active_queries,
        max_active_file_operations,
        max_pending_write_batches,
        max_pending_write_bytes,
    },
    response: StorageResponseSettings { batch_rows, batch_bytes },
    filesystem: StorageFilesystemSettings { poll_interval, max_entries, event_capacity },
};
let storage = StorageManager::new(StorageBackends { primary, state }, settings)?;
```

Managed persistence composes replay, capture, snapshot policy and journal retention
in `config::ManagedStorageSettings { replay, capture, snapshots, journal_tail }`.
Pass it once to `builder.managed_storage(settings)`; select a record projection
with `builder.store_records(projection)`. Managed state requires Serde encoding
and decoding. Backend journal-entry and checkpoint bounds remain physical storage
limits, separate from runtime snapshot cadence and payload limits.

For filesystem persistence, use the shipped JSON backend:

```rust
let primary = JsonStorage::new(
    JsonStorageSettings { root, max_entry_bytes },
    history,
);
let backends = primary.into_backends(StorageStateSettings {
    max_entry_bytes,
    max_checkpoint_bytes,
});
let storage = StorageManager::new(backends, settings)?;
```

Import `StorageStateSettings` from `storage::state`. The root holds bounded JSONL
segments in `data/`, staged and finished binary files, recovery metadata, and the
existing state backend under `state/`. A root has one active backend owner.
Choose the JSON entry bound to accommodate encoded metadata and payloads; JSON
encoding can be larger than the original binary payload. This adapter is a
simple persistence option, not a performance bound for other backends.

Backend writes are independently durable typed calls: `write_schemas`,
`write_objects`, `write_records`, `write_stream` and `write_files` consume
`StorageBatch<T>`; file appends consume `FileChunk`. The manager owns ordering
and retries. Each call must recognize replayed positions without duplicating
data. A successful earlier call remains durable if a later call fails; there
is no atomic transaction across these methods. Batch clones and slices share
immutable backing storage. Manager record and stream admissions consume owned
`RecordInput` and `StreamInput` vectors; inspection can still borrow them.

`max_active_file_operations` bounds independent file operations. Chunks for one
file remain ordered; final publication and durable cursors wait for their
predecessors. Recovery writes consume `StorageBatch<RecoveryEntry>` and snapshots
consume a singleton `StorageBatch<RecoverySnapshot>`, preserving shared ownership
through retries and blocking filesystem work.

Backend reads receive an owned `RetainedMemory` guard. Move it into blocking
work and retain it with the completed result until the awaiting caller takes
ownership; cancellation must not release the charge while that work still owns
data. Admit additional parsing scratch with `try_grow_to` before allocation.
Direct backend callers obtain a guard through `StorageManager::read_memory`
and keep a clone for as long as they retain the returned data. Manager readers
return shared owners that retain these charges automatically.

File operations share `FileSource::Stored(reference)` and `FileSource::Local(path)`.
`ReadFile` streams a selected byte range; `HashFile` returns its byte count and
SHA-256. `CaptureFile` creates an independent durable stored file while hashing.
`CaptureConsistency::Stable` rejects detected source changes; it does not provide
an atomic snapshot. `Bounded` copies the initially selected range. Local paths
refer to the executing host and may be anywhere its process can access, including
requests from gRPC, native plugins and Wasm guests. There are no configured-root
boundaries, path-scoped grants or extra per-plugin allowlists for local sources.
Normal operation exposure and existing stored-file scope grants still apply.
Transport paths use UTF-8 and reject unrepresentable paths; native readers retain
the host path representation. Symlink traversal is rejected.

`FollowFile` starts at `Beginning`, `End` or `At(offset)` and streams data with
explicit truncation/replacement events. Staged-production observation uses the
distinct `FollowProduction` operation. Native file slices and shared read batches
retain their memory charge until their final owner is released.

`ListDirectory` and `WatchDirectory` share a `DirectorySelection` containing the
root, recursion flag and ordered include/exclude rules. Patterns match
case-sensitive slash-relative paths: `*` stays within one segment and `**` crosses
segments. The last matching rule wins; unmatched paths are excluded. An excluded
ancestor does not suppress a later matching descendant. Watching subscribes before
initial enumeration, emits `Existing` then `InitialScanComplete`, and reports
notification uncertainty or overflow as `RescanRequired`. A modification event
does not establish that a producer has finished writing a file.

Finite storage queries select a source at a full persisted storage cursor,
apply an ordered transform plan, and apply row `offset`/`limit` to the final
result. Delivery emits a schema, bounded Arrow record batches, and a final
operation completion. Response `batch_rows` and `batch_bytes` bound each delivery
batch; they do not truncate the query result. In process, the manager's reader
yields native `QueryArrow` items whose shared Arrow buffers keep their storage
charge through clones and slices. The `storage_query` operation, declared in
`harmos-operations`, carries the same items as Arrow IPC (`QueryOutput`), and the
write operations take `DataBatch` values whose data is Arrow IPC; serve them with
`harmos::builtin::storage_registration`.

`ArrowSchema::from_ipc` and `ArrowBatch::from_ipc` consume encoded `Vec<u8>`
buffers. `QueryArrow::output` and `TryFrom<QueryOutput>` consume their inputs;
uniquely owned encoded buffers move through these conversions. Retained aliases
continue to share their existing allocations and memory charges.

```rust
let mut reader = storage.query(harmos::storage::Query { at, source, transforms, slice }).await?;
while let Some(output) = reader.next().await? {
    // One QueryArrow::Schema, then QueryArrow::Batch values.
}
let completion = reader.completion()?;
```

Direct storage APIs take resource-owned inputs: `Query`, `FileRead`,
`FileCapture` and `FileFollow`. Hashing also takes `FileRead`. Operation handlers
construct these inputs from their wire requests; the storage engine does not
depend on operation declarations.

Every read shares the one read budget. Work that cannot fit while other queries
hold memory waits for released capacity; work that can never fit the budget, even
after splitting source pages down to one row, fails with `ResourceExhausted`
instead of waiting on itself. A reader that already holds memory, including
batches its consumer retains, may wait while another owner can release capacity.
Growth fails with `ResourceExhausted` only when the waiting holders form a certain
cycle: none could grow even if every owner outside that set released its bytes. Dropping a reader or an in-progress `next` cancels
it and releases its query slot; batches already returned stay charged until
they are dropped.

A stream selection's `after` resumes at a stored position, as `RecordRead::after`
does: only rows stored after it are read, in position order.

Typed historical reads decode saved objects and records from their exact stored
bytes in pages bounded by the response settings, so payloads never expand to
Arrow and object size is not limited by the Arrow field bound. Filter decoded
values in the consumer, or use a `Query` transform for column predicates. A schema that was never
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
    // ReadBatch<Vec<ObjectRevision<Experiment>>>
}
let pages = storage
    .read_records::<Reading>(RecordRead { at, selection, after: 0 })
    .await?
    .collect()
    .await?; // ReadBatch<Vec<ReadBatch<Vec<StoredRecord<Reading>>>>>
```

Typed `next()` results retain each page and its memory charge. `collect()` retains
a collection of those pages without copying their records; its outer length is
the page count. Borrow rows with `pages.iter().flat_map(|page| page.iter())`.
For owned conversion, `page.try_map(additional_peak, convert_owned)` returns a
new retained batch. The callback consumes the unique input; strong or weak
aliases are refused. Bound all new output allocations and simultaneous conversion
scratch before calling it. Admission is nonblocking and precedes the callback;
Harmos accounts private owner growth. Existing charges remain retained without
automatic shrinking, and failure consumes this handle. Nested pages must retain
or map their own guards. Conversion errors and external captures must not retain
payloads. The callback error type implements `From<harmos::storage::Error>`.

`left.try_map_with(right, additional_peak, combine_owned)` moves two unique
payloads into one retained result. It preserves every input claim and admits
internal guard storage before allocation. This supports folding command pages
into a retained parent object without cloning payloads or detaching their guards.
Use one read session for dependent inputs; combining owners does not retroactively
merge independent admission groups. Both handles are consumed on failure.

Use one `storage.read_session()` when a read retains results while starting other
object, record, Arrow or file reads. Pass that session to all participating helpers; its
`read_objects`, `read_records`, `read_object`, `query` and `read_file` calls share admission dependencies.
Explicit cursors and versions still select the data: a session is not a snapshot.
Grouped reader creation returns resource exhaustion when an active query slot is
unavailable. Independent manager reads retain their queued admission behavior.

`session.collection::<Vec<ProjectedRow>>()` creates an empty retained collection
for bounded pagination. Grow it with `try_map`, reserving the full replacement
Vec backing before allocation; child pages keep their own charges. Stop reading
at the selected limit and drop the reader to release its active query slot.
Retained results remain valid after the reader and session are dropped. Do not
use an independent `read_memory` reservation for a collection that subsequent
reads depend on: it does not establish the shared admission dependency.

For mutable metadata, `session.try_memory(payload_bytes)` reserves a nonblocking
claim including private scaffolding. Keep the claim with the table it covers and
call `try_grow_by(additional_peak)` before allocating or mutating. Refusal preserves
the existing claim and table. `bytes()` includes scaffolding; `try_grow_to(total)`
is an absolute high-water operation, not an atomic increment across aliases.

`ArrowBatch::try_map` consumes its RecordBatch handle into a retained DTO result,
admitting new allocations and scratch first. Immutable Arrow buffer aliases keep
the shared charge; batches created by unadmitted decoding utilities refuse this
projection. `arrow::encode_payloads_memory(schema, payload, encoding, max_bytes)`
counts output collection and row storage using the encoding walker without heap
scratch. First admit and retain encoded rows, then inspect their bytes for the
consumer codec decoding bound and admit the typed projection separately.
`FileReadBatch::try_map` delegates to its unique retained owner, so
existing file-slice aliases refuse an owned move. Runtime query streaming retains
the Arrow source until destination admission, then converts to IPC inside the
admitted send closure.

`harmos::filesystem::input::read(path, limit)` bounds regular-file reads.
`harmos::filesystem::publication` owns staged atomic installation, replacement,
removal and explicit durability confirmation across Unix and Windows.

Before a write whose receipt must remain in pending ownership, use
`storage.object_receipts_memory(count)` for the producer's response bound, add
caller-owned pending/response scaffolding, and preclaim through the same session.
Move the final receipt's cursor and claim into pending ownership before the next
await. Retry durability or delivery using that receipt; reissuing an ordinary
`write_object_group` creates new revisions. The bound uses declared producer
history and covers receipt-vector capacity and both history copies per receipt.
Stored objects can enforce domain invariants through `StoredObject::validate`;
object encoding, decoding and catalog validation apply the same check.

`storage::query::Accumulator` folds scalars over rows a caller already holds
with the same semantics as query aggregates: exact rational `Sum`, `Mean` and
`Median`, ordering for `Min`, `Max` and `Last`, and signal kernels for floating
statistics. Snapshot reads run concurrently; closing storage waits for those in
flight.

For storage-authoritative refresh, query an explicit `cursor_saved()` and then
await `wait_saved_after(&cursor)` before querying the returned newer cursor.
The passive wait coalesces advances without missing changes between snapshot
and registration. It never requests persistence or retries failures. Cancellation
drops only the waiter. Available newer durable progress is returned first;
otherwise close, not-ready and persistence failures terminate the wait.
`await_durable(&receipt)` remains the active completion wait for a known write.
`FollowRecords`, `FollowStream` and `FollowObjects` deliver saved history after
their `after` cursor, then newly accepted entries. File following uses the
source/start contract above; staged producers use `FollowProduction`.

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
`FileBackend::with_journal_opener` accepts a caller-owned journal opener for
controlled file access, including fault testing. The opener receives the journal
path under the exclusive root lease and must preserve its existing bytes; it is
also used when reopening after checkpoint replacement.

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

An operation declaration and its implementation share one vertical file. The
request is a separate portable message; live resources are runtime-only fields.
This example assumes a resource crate whose `Error` converts `DependencyError`:

```rust
#[cfg(feature = "runtime")]
use {device::{Device, Error}, harmos::{Operation, OperationContext, Response}, std::sync::Arc};

#[harmos::message]
pub struct ReadRequest { pub channel: u32 }

#[harmos::message]
pub struct ReadResponse { pub value: f64 }

#[harmos::operation(id = "read", request = ReadRequest, response = ReadResponse,
    policy = crate::POLICY, infer_request)]
pub struct Read {
    #[cfg(feature = "runtime")]
    device: Arc<Device>,
}

#[cfg(feature = "runtime")]
impl Operation for Read {
    type Request = ReadRequest;
    type Response = ReadResponse;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: OperationContext) -> Result<Self, Error> {
        Ok(Self { device: context.dependency::<Device>()? })
    }

    async fn run(&mut self, _: OperationContext, request: &mut ReadRequest,
        _: &mut ()) -> Result<Response<ReadResponse>, Error> {
        Ok(ReadResponse { value: self.device.read(request.channel).await? }.into())
    }
}
```

The operations crate depends on `harmos` with `default-features = false` and
uses `runtime = ["harmos/runtime", "dep:device"]`. Shared models remain in the
product domain; independent resource libraries stay optional dependencies.
Cargo feature unification can enable runtime elsewhere in a dependency graph.

```rust
#[harmos::catalog]
pub enum Operations { Read(temperature::read::Read) }

let definitions = Operations::catalog();
let mut dependencies = harmos::Dependencies::new();
dependencies.insert(device)?;
use harmos::RegistrationExt;
runtime.register(Operations::registration(dependencies).lane(Lane::Interactive)).await?;
let reading = runtime.operation(ReadRequest { channel: 3 }).retry(retry).run().await?;
let pending = runtime.operation_for::<Read>(ReadRequest { channel: 3 }).start().await?;
let reading = pending.join().await?;
runtime.unregister(&harmos::QualifiedId::parse("host.read")?).await?;
```

`Application::Lanes` names the application's closed lane enum. Registration
uses `.lane(lane)` or `.defaults::<Lane>()`, with optional
`.default_lane_for::<Read>(lane)` member overrides. Builders are inert until
`start` or `run`. Request-specific retry overrides leave the declaration unchanged.
`infer_request` generates one `RequestFor` implementation; omit it when the
same request serves several operations and invoke with `operation_for`.

Streaming declarations add `input = Item` or `output = Item`. Implementations
use typed `StreamInput`, `StreamOutput` or their pair as `Endpoints`, passed
through the invocation's `.endpoints(...)` method. Native and carrier handlers
implement the same `Operation` trait and obtain resources through
`OperationContext::dependency`. Portable call metadata remains separate from
those process-local dependencies. Carrier dependencies must support their target.

`Message` exposes `CODEC`, `encode`, `encode_into`, `encoded_size`, `decode`
and `decoding_memory`. `Portable::fingerprint()` identifies the complete typed
contract; `digest()` identifies a payload, including exact floating-point bits.
Operation definitions use fingerprints rather than manual operation versions.
Stored application schema and journal versions retain their existing formats.

Operations initialize lazily with `initialize(OperationContext) -> Result<Self, Error>`.
`run` returns `Response<T>`; `stop` settles work and `cleanup` releases resources.
Services implement the separate `Service` trait, using the same context and
lifecycle verbs. The runtime's service context is named `ServiceContext`.

`OperationPolicy` controls overall timeout, attempt timeout and request retries.
Resident restart behavior belongs to `ServicePolicy { stop_timeout, restart }`
using `RestartPolicy::OnFailure`. A service factory joins the same batch:

```rust
let entries = harmos::RegistrationBatch::new(dependencies)
    .service::<Poller>("poller", service_policy);
runtime.register(entries.defaults::<Lane>()).await?;
runtime.unregister(&harmos::QualifiedId::parse("host.poller")?).await?;
```

The host assigns provider owners. Local IDs use lowercase letters, digits and
underscores, starting with a letter; routing uses `owner.local_id` exactly once.
Registration validates a whole batch before publishing it. Withdrawal rejects
new calls, cancels active work and awaits bounded cleanup. Failed cleanup retains
the closing owner, and another `unregister` resumes settlement. Re-registration
allocates a fresh generation so stale frames cannot affect the replacement.

The intended configuration contract gives each subsystem ownership of its settings
and validation in its own `config.rs`; root configuration composes those types.
This migration is incomplete: `crates/harmos-runtime/src/config.rs` still defines
decode, encode, gRPC, operation and plugin settings. Consumers own defaults and
pass each owner its relevant settings rather than retaining global configuration
in every manager.

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

Sidecar wire credits bound in-flight encoded frames. Each connection also owns
one reusable writer scratch buffer bounded by the protocol frame ceiling,
separate from those credits. Scalar-only writers coalesce small frames to avoid
separate header and payload writes. Frame size helpers count envelopes without
encoding them. Child-call admission includes the actual portable context;
`Frame::child_payload_encoded_len` supports that calculation. The general
request helper covers routed calls and children with empty portable context.
Logical frame sizes exclude the four-byte packet prefix; wire credits include it.

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

Embedded operation contributions use the same `RegistrationBatch` factories and
typed lane registration as host contributions. Plugin scopes retain their
host-assigned ownership and authority checks. Carrier serving tables receive
`Operations::registration(dependencies)` through `WireTable::register`; carrier
setup creates the owner-bound table after host assignment.
Encoded calls require an explicit `operations::Settings::preparation_bytes`
budget.

`PluginManager` remains the lower-level extension for applications supplying their own backend lifecycle.

Operation metadata belongs on its owning struct in a vertical file; catalogs
assemble those types. `harmos::prelude::*` provides a bounded set of common
authoring imports. See the [crate manifest](../crates/harmos/Cargo.toml) and
[authoring skill](../.agents/skills/harmos-authoring/SKILL.md) for details.

## Performance

Harmos emits structured `tracing` spans around operations, managed state and
storage interfaces. Spans remain available in release builds without a separate
feature. Applications own subscriber installation, runtime filters and export;
Harmos does not install a process-wide subscriber.

Request context follows internal queues and tasks. Shared storage work exposes
one causal contribution per request; shared batch fields connect these to the
physical write, including across scoped subscribers. Existing operation identities,
invocation IDs and storage positions support correlation. Cross-process lifecycle phases expose
invocation identities for joins; process-local span IDs are not portable trace IDs.
Instrumentation excludes request and response payloads. Native system-test traces
also contain exporter metadata, including executable and command-line paths;
they are not sanitized artifacts.

Span lifetimes measure elapsed time, including suspension. Queue, admission,
preparation and durability waits are distinct from backend execution. Overlapping
spans cannot be added to obtain total latency, and span duration is not CPU time.
Use a CPU or allocation profiler to investigate work inside an expensive stage.

System stories and the schematic fixture applications opt into native `.pftrace`
capture through `STKR_TEST_REPORT_DIR`, with process-specific files in its `traces/`
child. Normal completion explicitly flushes the exporter; a killed process may
leave a partial trace. Local parent and causal-flow relationships are native trace
data. Cross-process phases use invocation and process fields for joins.

For shared writes, join `storage.batch.contribution` to `storage.batching.write`
using `store`, `epoch`, `domain`, `first_index`, `last_index` and `through`, then
match overlapping attempt lifetimes. Contributions also carry `request_index`,
`request_sequence` and `request_domain`. Retries create new spans with the same
logical batch fields. Checkpoint contributions use the storage identity and
`frontier`, with file or mutation fields identifying their sources.

Context propagation restores the owning subscriber during polling and cancellation.
Some asynchronous handoffs allocate a boxed future for this teardown guarantee;
instrumentation overhead has not been benchmarked.

Storage, sidecar hosts, Wasm guests and external gRPC commands have separate
benchmark suites. Lower-level stream and framing suites retain their codec and
transport measurements. The carrier suites enable the `benchmark` feature on the
shipped Schematic sidecar and guest, sharing their transfer contracts and handlers.
Each direction uses 256-record, 4096-sample or 1 MiB file batches, comparing unary
calls against a stream. Generation and content verification are outside timing;
completion includes the receiver acknowledgment. Records, files and samples run
concurrently on one reused host/plugin per carrier, with isolated transfer IDs;
unary and streamed variants remain sequential within each dataset. The fixture retains at most
64 MiB of data, with four queued batches and a 4 MiB stream budget. File transfer
cases measure crossing the carrier boundary; the storage suite measures persistence.
The real carrier hash check runs outside timing; its storage worker is started
only for that check and stopped before command and transfer measurements.

| Suite | What It Measures |
| --- | --- |
| [Storage](../crates/harmos-runtime/benches/storage/main.rs) | Shipped JSON persistence: 30 MiB upload/download, 10,000 records and 1,000,000 samples with symmetric writes/reads, concurrent files and queries; local/stored read, hash, capture and follow; directory listing and change delivery. |
| [Sidecar Host](../crates/harmos-runtime/benches/sidecar/system.rs) | Real native process: bidirectional 10,000-record, 30 MiB file and 1,000,000-sample transfers, comparing unary batches and streams; startup and command latency. |
| [Guest](../crates/harmos-runtime/benches/guest/main.rs) | Real Wasm component: the same bidirectional unary/streamed transfer workloads, plus startup and command latency. |
| [External gRPC](../crates/harmos-runtime/benches/grpc/main.rs) | Separate server-process startup, client commands over a socket, and isolated Postcard/Protobuf codec costs. |
| [Streams](../crates/harmos-runtime/benches/streams/main.rs) | Concurrent waveform work, batch summaries, codec costs, and native consumers using 64 KiB and 1 MiB payloads. |
| [Plugin Transport](../crates/harmos-sidecar/benches/transport/main.rs) | Native transport throughput and latency, including 64-byte, 1 KiB, and 64 KiB bursts and a paced 10 MiB/s workload. |

Build matching executables and carrier fixtures with `mise run build:rust:bench`,
then run `mise run bench`. Storage uses 1 MiB chunks, 4096-row input ceilings,
128 MiB read memory and 64 MiB pending-write memory. Reads start from persisted
data; setup writes and verification are excluded from read timings. These are
warm filesystem measurements, not forced cold-cache reads.
The filesystem cases use 30 MiB files; directory listing and initial watching
use 1,000 entries. Hash/capture include their required hashing work. Directory
change latency includes creating the file and receiving its notification.

One million samples represent one second at 1 MHz without pacing ingestion.
The paced transport rate is also an input, not a claimed throughput result.
Published comparisons need the revision, hardware, payload, batching and carrier
alongside native Criterion results. The JSON backend gives one concrete reference;
it does not establish a worst-case bound for other adapters.

Native Criterion HTML and raw measurements live under `.stkr/benchmarks/criterion/`,
partitioned by compatibility identity and benchmark executable. The runner compares
against the compatible `dev` baseline and reports missing baselines or regressions
in its output. HTML rendering uses Plotters and does not require Gnuplot.

## Documentation

- [Runnable Examples](../examples/) — application composition and plugin integration.
- [Public Rust API](../crates/harmos/src/lib.rs) — the SDK facade and module documentation.
- [Cargo Features](../crates/harmos/Cargo.toml) — select the facilities your application needs.
- [Storage Adapter](../examples/schematic/storage/) — own your persistence format.


## Built-In Operation Ownership

`crates/harmos-operations/src/{host,lifecycle,storage,streams}/` holds the 30
built-in operations. Each operation file owns its request and response shapes,
operation declaration and runtime-gated implementation. Shared portable values
and lifecycle protocol schemas remain in `harmos-domain`. The facade exports
these catalogs under `harmos::builtins`; `Builtins::catalog()` includes the two
reserved plugin lifecycle controls.

The operations crate enables all built-in execution with `runtime`, which includes
`harmos-sdk` and `harmos-storage`. Carrier kits select its lightweight `lifecycle`
feature for the two SDK plugin controls, without linking host storage into a
Wasm component. It never depends on `harmos-runtime`. Runtime composition supplies SDK-owned
instance, discovery, publication, lifecycle and grant-access handles through
`OperationContext`, along with the erased storage engine. State owners retain
authorization, generation fencing and resource lifecycle ownership.

Protocol, artifact, envelope, journal and schema formats are v1. Only current
shapes are supported; there are no old-peer checks, historical shape converters,
legacy decoders or journal upgrades.
