# Harmos Product Guide

Detailed composition, feature selection and runnable examples. Start with the [Quick Start](../README.md#quick-start).

## Examples

The [Schematic Editor](schematic.md) is one application with two
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
| `sidecar` | Native process authoring and hosting with matching module attributes. |
| `guest` | Wasm guest authoring and native sandbox hosting with matching module attributes. |
| `grpc` | gRPC, reflection, TLS, and Web support; settings select their use. Includes `generate-proto`. |
| `generate-proto` | Protobuf descriptor projection used by runtime transport. |
| `signal` | Signal-processing kernels. |
| `signal-fft` | FFT support; includes `signal`. |
| `simulation` | Deterministic runtime simulation. |

Application-local attributes use `harmos::runtime::{operation, catalog}`; plugin-local
attributes use the matching `sidecar` or `guest` module. Reusable operation libraries
use root attributes and declare their own `runtime` feature wired to `harmos/runtime`.

Applications and plugins depend on the `harmos` facade. Shared portable
contracts are available without its runtime feature. `sidecar` and `guest`
select matching authoring modules and attributes; guest builds use target-compatible
SDK execution without the native host runtime. Operations use
`harmos::{Operation, Context, Response}`: the operation is the request, and
`async fn run(self, ctx: Context<'_, Self>) -> Result<Response<T>, Self::Error>`
is its required method. Declare `resources` and `operations` on the operation;
resolve them through `ctx.resource::<T>()` and `ctx.operation(request)`.
`ctx.streams()` exposes derived endpoints. Use dotted IDs and `visibility = Public`
or `Host` when needed; the default is `Private`. Applications and plugins share
`Lifecycle` with async prepare, initialize, stop and cleanup. Plugin lifecycle
contexts are `harmos::sidecar::Supervisor` or `harmos::guest::kit::Context`.
Build tools use `harmos-packaging` for inert inspection and generation.
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

### Tool Installation

The [current release](https://github.com/stokker-technologies/harmos/releases/latest) provides
`cargo-harmos-<version>-<Rust-target-triple>.tar.gz` on Unix and `.zip` on Windows.
Each archive contains `cargo-harmos` (`cargo-harmos.exe` on Windows), LICENSE,
README.md and generated CHANGELOG.md. Verify the archive against `SHA256SUMS`,
extract it, and place the executable on `PATH`, preserving its executable bit on
Unix. Run `cargo harmos --help` to check installation. Mise installation is
verified against the archived binary before release completion. Harmos library
crates are published on crates.io with generated CHANGELOG.md; the Cargo tool
is distributed through release archives.

CI packages retain `schematic-<version>-<platform>.zip` demo bundles for
`linux-x86_64` and `windows-x86_64`. Extract the complete ZIP and run
`schematic-cli` (`schematic-cli.exe` on Windows); `--guest` selects the bundled
Wasm backend. Keep its `plugins/` directory beside the executable. Package
checks run both backends from an isolated extraction. Examples, test reports and
fixtures remain source or CI artifacts. Release downloads include generated
`CHANGELOG.md` and `SHA256SUMS`; the internal upload inventory is not a download.

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
exact artifact and schema paths. Native collection requires Rust's `llvm-tools`
component. Collection removes debug information from shipping copies under `harmos/<producer>/`
beside compiler outputs on every platform. Original compiler outputs and separate
debug files remain available for debugging. The 128 MiB bundle limit applies to
shipping bytes; compiler profiles are unchanged. Each backend's `dev` command watches sources and local
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

Harmos serves no built-in operation by default. Place `builtins::Lifecycle` on a
runtime lane with `setup.operations::<builtins::Lifecycle>(lane)` to provide the
Status, Cancel and Watch operations (gRPC `/harmos.host.v1.Operations/Status`, `/Cancel`
and the server stream `/Watch`) over the manager's instances. Each answers only
for active instances the caller submitted or holds a resident lease on; unknown,
foreign and settled ids are refused alike. A watch streams every status change
from the moment it starts until the instance stops or settles. Plugins reach them like any other operation,
so they must declare them as requirements.

Hosts place `builtins::host::Host` on a chosen lane to serve
`builtins::host::Catalog` (unary gRPC path
`/harmos.host.v1.Host/Catalog` when `grpc` is enabled). `builtins::host::CatalogRequest` selects a
`kind` (`Operations`, `Schemas`, `Streams` or `Objects`) and accepts optional
`schema`, `after`, `rows` and `bytes`; defaults are 64 rows and 64 KiB, with
maxima of 256 rows and 4 MiB. `CatalogPage::entries` is a UTF-8 JSON array of the
kind's entries: operation metadata (original contracts, provider
owners and generations, selected grants and current gRPC availability), stored
schemas or stored identities. The `next` cursor continues a bounded page. Stored
kinds need the storage manager and a `host.catalog` grant scoped `schemas:*`,
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

Storage uses explicit request, response and filesystem settings, with no defaults:

```rust
let settings = StorageSettings {
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
Pass it once to `setup.settings().managed_storage(settings)`; select a record projection
with `setup.settings().store_records(projection)`. Managed state requires Serde encoding
and decoding. Backend journal-entry and checkpoint bounds remain physical storage
limits, separate from runtime snapshot cadence and payload limits.

`SnapshotPolicy.max_bytes` limits logical snapshot payload bytes at both capture
and recovery. Harmos derives the backend read bound with the same JSON/base64
size calculation used for write reservations; encoded queue and physical backend
limits still apply independently.

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

Backend readers return shared page owners. Blocking work retains its input and
output owners through cancellation and completion. Query concurrency, serialized
page sizes and pending writes remain bounded by the configured request and response
settings. Consumers own budgets for their retained results and conversion scratch.

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

Native `read_opened_file(file, range)` reads the supplied process-local handle,
preserving its original inode even when its pathname is replaced. On Windows,
the handle must permit a separate read open; exclusive handles are rejected so
native reads can keep the caller's cursor independent without reopening by path.
Keep a named or shareable delete-on-close temporary owner alive through native
reads and captures; unlinking a temporary before reopening it can fail under
Windows filesystem filters. `StorageManager::freeze_opened_file` creates a private
named immutable copy exposing length and SHA-256. `FrozenFile::read` and
`capture_frozen_file` retain that owner through readers, retries, cancellation
and blocking I/O. Local filesystem errors include the step, path and native cause;
opened-handle paths are resolved only for diagnostics.
The storage manager also exposes `follow_file`, `follow_production` and
`file_progress` alongside retained metadata. `capture_opened_file(file, target,
range, consistency)` synchronously returns an `OpenedCapture` that owns the
original handle and `StorageFile`. Its asynchronous `finish` retains setup,
copy progress and exact pending bytes across errors and caller cancellation.
Existing producer prefixes are compared before a suffix is appended;
`acknowledged()` reports completed copy progress. `StorageFile::write_retained_at`
refreshes pending-write accounting and awaits capacity using the same admission
path as `write_all`. It retains the original chunk across failures and caller
cancellation; `retry_append` resends its original offset and bytes without
cancelling production. Stable capture detects source
changes but does not freeze concurrent writers into an atomic snapshot.

`FollowFile` starts at `Beginning`, `End` or `At(offset)` and streams data with
explicit truncation/replacement events. Staged-production observation uses the
distinct `FollowProduction` operation. Native file slices and shared read batches
retain their backing allocations until their final owner is released.

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
yields native `QueryArrow` items whose shared Arrow buffers remain valid
through clones and slices. The `storage_query` operation, declared in
`harmos-operations`, carries the same items as Arrow IPC (`QueryOutput`), and the
write operations take `DataBatch` values whose data is Arrow IPC; serve them with
`harmos::builtin::storage_registration`.

`ArrowSchema::from_ipc` and `ArrowBatch::from_ipc` consume encoded `Vec<u8>`
buffers. `QueryArrow::output` and `TryFrom<QueryOutput>` consume their inputs;
uniquely owned encoded buffers move through these conversions. Retained aliases
continue to share their existing allocations.

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

Storage bounds active and pending query counts, pending write bytes/batches and
serialized response pages through consumer-supplied settings. Readers wait for
an active query slot within the pending-query limit. Dropping a reader or an
in-progress `next` cancels its work and releases its query slot; returned batches
remain valid until their owners are dropped. Storage does not reserve a shared
read-memory pool or admit consumer conversion scratch. Consumers bound their
own retained results and scratch allocations.

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
    .read_objects::<Experiment>(ObjectRead {
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

Typed `next()` results share their page backing. `collect()` retains pages without
copying records; its outer length is the page count. Borrow rows with
`pages.iter().flat_map(|page| page.iter())`. `page.try_map(convert_owned)` and
`left.try_map_with(right, combine_owned)` consume unique owners into new results;
strong aliases refuse the move. The callback error implements
`From<harmos::storage::Error>`. Consumers bound their own conversion allocations.

Read directly through the storage manager with explicit cursors and versions.
Drop a reader to release its active query slot. Retained pages remain valid after
the reader is dropped. Queries use temporary disk-backed Arrow pages for global
sorting and reductions, retaining their query slot through output. Cancellation
stops detached reduction work and releases temporary files.

Native callers can attach their own allocation owner with
`operations::RequestClaims::retain(owner)` and submit using `Host::call_owned` or
`submit_owned`. Accepted work retains that owner after caller cancellation.
Guest and sidecar bindings refuse process-local claims.

`ArrowBatch::try_map(convert_owned)` projects a RecordBatch into a shared result.
`FileReadBatch::try_map` requires a unique owner. Direct Arrow projections decode
only selected payload fields, and repeated reads can reuse immutable Arrow pages.

`harmos::filesystem::input::read(path, limit)` bounds regular-file reads.
`harmos::filesystem::publication` owns staged atomic installation, replacement,
removal and explicit durability confirmation across Unix and Windows.

Keep write receipts with pending work and retry durability or delivery using
those receipts; reissuing an ordinary `write_object_group` creates new revisions.
Stored objects can enforce domain invariants through `StoredObject::validate`;
object encoding, decoding and catalog validation apply the same check.

`storage::query::Accumulator` folds scalars over rows a caller already holds
with the same semantics as query aggregates: exact rational `Sum`, `Mean` and
`Median`, ordering for `Min`, `Max` and `Last`, and signal kernels for floating
statistics. Exact order statistics keep numeric slots in memory up to the
`StorageAccumulatorSettings::memory_bytes` budget, then spill to disk. The default
is 64 KiB per accumulator; callers can supply their budget explicitly:

```rust
let accumulator = storage::query::Accumulator::with_settings(
    storage::query::scalar::Agg::Of(storage::Aggregate::Median),
    storage::config::StorageAccumulatorSettings { memory_bytes: 16 * 1024 },
);
```

A zero budget selects immediate spilling. Both paths poll the cancellation closure
passed to `finish` and preserve the same exact rank and interpolation semantics.
Snapshot reads run concurrently; closing storage waits for those in
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
the retry window. The receipt ledger has a bounded entry count; reclaimable
completed receipts are evicted oldest first. Pending mutations and admitted
callers retain their completion across replay-window expiry; fresh expired
requests remain refused. A valid retry of an evicted identity runs as a new mutation.

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
still retire production; existing file-count and pending-write limits remain enforced.
A viewer owns its subscription, while the original service owns production.

One file owns each operation. Its definition division contains messages, the operation struct, policy, contract implementations and
contract tests. The runtime division follows the definition's Tests section and contains its own complete skeleton, including runtime-only
tests. Shared models remain in the product domain; resource libraries are optional execution dependencies.

```rust
/* === --- === Definition Module === --- === */

/* === Modules === */


/* === Re-Exports === */


/* === Imports === */

#[cfg(feature = "runtime")]
use device::Device;

/* === Constants === */

const POLICY: harmos::OperationPolicy = crate::POLICY;

/* === Macros === */


/* === Type Aliases === */


/* === Structs === */

#[harmos::message]
pub struct Reading { pub value: f64 }

#[harmos::operation(id = "temperature.read", response = Reading, policy = POLICY,
    visibility = Public, resources = [Device])]
pub struct Read { pub channel: u32 }

/* === Enums === */


/* === Traits === */


/* === Implementations === */


/* === Public Functions === */


/* === Private Functions === */


/* === Tests === */


/* === --- === Runtime Module === --- === */

#[cfg(feature = "runtime")]
mod runtime {
    /* === Modules === */


    /* === Re-Exports === */


    /* === Imports === */

    use super::*;
    use harmos::{Context, Operation, Response};

    /* === Constants === */


    /* === Macros === */


    /* === Type Aliases === */


    /* === Structs === */


    /* === Enums === */


    /* === Traits === */


    /* === Implementations === */

    impl Operation for Read {
        type Error = device::Error;

        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<Reading>, Self::Error> {
            let device = ctx.resource::<Device>();
            Ok(Reading { value: device.read(self.channel).await? }.into())
        }
    }

    /* === Public Functions === */


    /* === Private Functions === */


    /* === Tests === */
}
```

The operations crate uses `harmos` with `default-features = false` and
`runtime = ["harmos/runtime", "dep:device"]`. A unit operation struct is a unit request. `response` defaults to `()` and
`visibility` to `Private`; `Public` and `Host` are short visibility forms. The operation struct is the request message;
its runtime implementation declares only `Error` and consuming `run` (plus optional error classification).
Private operations cannot declare `grpc(...)`; use `Public` or `Host` for gRPC exposure.
The macro rejects Private bindings at compile time, and runtime registration refuses them.

For a platform-only operation, put `#[cfg(windows)]` on its `mod` declaration and its catalog variant. The catalog preserves
variant attributes for both portable projections and runtime registration. Short built-ins may keep their gated implementation inline.

```rust
#[cfg(windows)]
mod desktop;

#[harmos::catalog]
pub enum Operations {
    Read(temperature::read::Read),
    #[cfg(windows)]
    Desktop(desktop::Inspect),
}

impl harmos::Lifecycle for Product {
    type Config = ProductSettings;
    type Error = DeviceError;
    type Context = harmos::Startup<Self>;

    async fn prepare(config: Self::Config, setup: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        // Resources
        let device = std::sync::Arc::new(Device::open(config.device).await?);
        setup.resource::<Device>(device.clone());

        // Lanes
        setup.lane(Lane::Interactive, harmos::operations::LaneSettings { capacity: config.capacity });

        // Operations
        setup.operations::<Operations>(Lane::Interactive);
        Ok(Self { device })
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> { self.device.close().await }
}

let runtime = harmos::Runtime::<Product>::launch(origin, settings).await?;
let reading = runtime.operation(Read { channel: 3 }).retry(retry).lane(Lane::Interactive).run().await?;
runtime.shutdown().await?;
```

Applications implement the shared `Lifecycle` and keep their resource owners in
`Self`. `prepare` receives one mutable `Setup<Self>`; settings live behind
`setup.settings()`, and all declared application lanes are available automatically.
`launch` reports typed duplicate resources/catalogs, missing resources and unknown
lanes together before initialize runs. Distinct instances of one Rust type need
distinct newtypes. `Arc` insertion specifies the concrete type explicitly.
Shutdown drains supervised work, then calls owner stop and cleanup; owner lifecycle
hooks close devices and connections. Cleanup releases prepare's acquisitions even
when validation prevents initialize, and still runs when stop fails. Put physical
resource closure in cleanup so failed closure can retain its owner for retry.

Tests use the same preparation and catalog path, replacing prepared allocations
by type with `Runtime::launch_with(origin, settings, |setup| {
setup.resource::<Device>(fake.clone()); })`. Overrides do not replace catalogs.
Prepare still runs first: configure acquisition for the test environment. The
lifecycle owner closes the resources it acquired; overrides change execution lookups.

`ctx.call()` exposes portable call metadata. `ctx.resource::<T>()` returns `Arc<T>` infallibly for a type listed in `resources`.
`operations = [other::Read]` permits `ctx.operation(other::Read { channel: 3 }).await`; catalogs aggregate these callees into
plugin requirements. Explicit grants still belong to the plugin. There are three verification layers:

- Compile: generated `Requires<T>` and `Invokes<T>` bounds reject undeclared resource and operation access.
- Activation: the complete registration requires all resources and callee contracts. Cross-plugin calls require `Public`, or `Host`
  for a host caller; missing targets and incompatible fingerprints are refused before execution.
- Run: operation errors, deadlines and carrier faults remain fallible.

`input = Item` and `output = Item` derive `StreamEndpoints`: `()` for unary calls, `StreamInput<Item>`, `StreamOutput<Item>`,
or their pair. Borrow endpoints with `ctx.streams() -> &Self::StreamEndpoints`; input receives serialize through an asynchronous mutex.
Invocation builders accept `.endpoints(...)` and remain inert until `.start()` or `.run()`.

Owners (plugin, artifact, sidecar record and stream owners) are one `[a-z][a-z0-9_]*` segment, such as `obc_plugin`.
Dots and hyphens are refused in owners, including by `#[plugin]` at compile time. Reserved identity rules still apply.
Local operation IDs use dotted segments `segment(.segment)*`, each with that same grammar. The runtime prepends the owner once:
`temperature.read` becomes `plugin_name.temperature.read`. Built-ins include `host.operations.cancel`, `harmos.initialize`
and `storage.append_file`. `codec = manual` preserves an explicitly authored message codec.

Standard `HashMap<K, V>` and `BTreeMap<K, V>` work as message fields and
complete operation payloads. Keys are `bool`, `u32`, `u64`, `i32`, `i64` or
`String`; values are supported scalars, bytes or named messages. Encoding sorts
keys, so equal contents have the same bytes, fingerprint and digest across
container types, hash seeds and insertion orders. Decoding accepts unordered
entries and rejects duplicate keys while retaining the existing work and depth limits.
Container capacity and nested allocations participate in memory admission.
Custom hashers implement `MemorySize`; their default state's reported allocation
must bound the state constructed during decoding.

Maps use the existing repeated entry-message wire shape: each entry has
`key = 1` and `value = 2`; a root map wraps these in `entries = 1`.
Generated protobuf contracts and schema projections expose entry lists,
rather than native protobuf maps or JSON objects. Nested containers such as
`HashMap<String, Vec<Row>>` remain unsupported; use a named message for the value.

Root `Option<T>` payloads use an optional `value = 1` field. `None` and
`Some(default)` keep their distinct presence in both Postcard and protobuf.
The contained type can be a scalar, bytes, a named message or another described
root container; existing optional message fields retain their current shape.

`Message` supplies encoding and decoding; `Portable` supplies structural fingerprints and payload digests. Retries run a clone
of the original operation request. Large payloads belong in streams. The low-level `Carrier` lifecycle is used for carrier proxies
and lifecycle controls; services retain their separate `Service` lifecycle.

`OperationPolicy` controls overall timeout, attempt timeout and request retries.
Resident restart behavior belongs to `ServicePolicy { stop_timeout, restart }`
using `RestartPolicy::OnFailure`. Register one typed service against the frozen
resource context:

```rust
runtime.services().register::<Poller>("poller", service_policy).await?;
runtime.unregister(&harmos::QualifiedId::parse("host.poller")?).await?;
```

The host assigns provider owners. Local IDs use dotted lowercase segments; routing uses `owner.local_id` exactly once.
Registration validates a whole catalog before publishing it. Withdrawal rejects
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

Embedded operation contributions and host contributions register resource-free
catalogs through `register::<Operations>(lane)`. Plugin scopes retain their
host-assigned ownership and authority checks. Native and guest authors declare
one constant slice:

```rust
#[harmos::sidecar(id = "device_tools", requires = ())]
impl harmos::Plugin for DevicePlugin {
    const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
}
```

The same descriptors drive inert inventory and executable binding. The SDK
owner collects resources/settings from shared Lifecycle prepare, installs constant
catalogs and observer attachments, validates all errors, then initializes. Plugin
setup has no operations method; authors never construct serving tables or resource
stores. Encoded calls still require an explicit
`operations::Settings::preparation_bytes` budget through setup settings.

`PluginManager` remains the lower-level extension for applications supplying their own backend lifecycle.

Operation metadata belongs on its owning struct in a vertical file; catalogs
assemble those types. `harmos::prelude::*` provides a bounded set of common
authoring imports. See the [crate manifest](../crates/harmos/Cargo.toml) for details.

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

- [Runnable Examples](examples.md) — application composition and plugin integration.
- [Schematic Walkthrough](schematic.md) — the example application, its crates and backends.
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
`ExecutionContext`, along with the erased storage engine. State owners retain
authorization, generation fencing and resource lifecycle ownership.

Artifact contract inventory emits one `contracts` fragment per provided or required
operation, with `facility`, `owner` and an `entry` containing `direction` and
`contract` (plus `grants` for requirements). A separate `lifecycle` entry carries
the initialization/shutdown policies, including for an empty plugin. Each JSON
fragment is limited to 64 KiB. Inspection merges entries into one resolved
manifest, enforcing entry limits and duplicate rules across fragments; shared
`preparation_bytes` and `preparation_work` budgets bound the aggregate metadata
and schema resolution. Aggregate provides/requires fragments are unsupported.

Protocol, artifact, envelope, journal and schema formats are v1. Only current
shapes are supported; there are no old-peer checks, historical shape converters,
legacy decoders or journal upgrades.

## Transaction Reversibility

Implement `harmos::state::Invertible<S>` with `type Inverse` and
`fn inverse(&self, before: &S) -> Self::Inverse` for a reversible transaction.
The catalog infers an undo barrier when that implementation is absent.
`#[harmos::transaction(id = "ledger.deposit", inverse = Withdraw)]` optionally
checks that the inverse declaration matches the implementation and its mutual pair.
There is no explicit irreversibility marker.

## Plugin Source Layout

Runtime-selected operation IDs bind an existing type through
`operations::Registration::new(lane).operation::<Read>(id)?`, then
`runtime.register_batch(batch).await?` or plugin `register_embedded_batch`.
The operation type supplies its schema, policy, resources and handler; the chosen
ID enters the fingerprint. Invoke it with `operations.submit_at::<Read>(id, request)`.
Static catalog composition and ordinary collision refusal remain unchanged.

The plugin owner `host` is admitted like any other owner. Its name confers no
application authority or grant exemption. Qualified identity collisions remain
refusals; application caller authority comes from the runtime context.

Keep one `-sidecar` or `-guest` crate under `crates/`. Several plugins belong
under `plugins/sidecars/<name>` and `plugins/guests/<name>`. Carrier crates use
`publish = false`.

Each operation has one portable definition in `operations/<op>/mod.rs`, with
parallel `windows.rs` and `unix.rs` implementations declared there:

```rust
#[cfg(all(feature = "runtime", target_os = "windows"))]
mod windows;
#[cfg(all(feature = "runtime", unix))]
mod unix;
```

Keep the request, response, identity and policy in that one definition. Gate
module and re-export declarations only, never items or catalog variants. A
plugin has one catalog, with no platform catalogs or backend traits. Its
`Lifecycle::prepare(config, setup)` acquires resources through
`setup.resource::<T>(value)`; `Plugin::CATALOGS` names the catalog's `CATALOG`.

Rust's orphan rule prevents a runtime crate from implementing Harmos's
`Operation` for a type declared in a separate domain crate. Keep the operation
types and their implementations in an operations crate with a `runtime`
feature; shared models remain in domain.

Observer `interval_ms` accepts constant `u64` expressions such as
`interval_ms = policy::POLL_INTERVAL_MS`. The generated observer inventory
rejects zero at compile time; the cadence remains part of the static declaration.
