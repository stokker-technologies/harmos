# Harmos Product Guide

Detailed composition, feature selection and runnable examples. Start with the [Quick Start](../README.md#quick-start).

Embedded language bindings can stage runtime-owned unary operations with
`operations::EncodedBinding::new(contract, decoding_memory, prepare)` and
`Bindings::encoded(binding, lane)`. `Contract::from_schema` derives the canonical
fingerprint from an owned graph. The codec validates payloads and estimates decoded
allocation; its prepared `NativeInstance` runs through normal admission, retries,
cancellation and settlement. Plugin staging validates the frozen contribution
catalog and retains its original generation. Runtime bindings currently use the
canonical encoded carrier without a gRPC projection or stream endpoints.

`submit_encoded_with(operation, payload, retry)` and
`PluginCallback::submit_with_retry(bytes, encode, retry)` apply policy overrides to
one call. Unknown replay safety still refuses enabled retries. Existing deadlines,
authorization and attempt records remain authoritative. Embedded observer owners
use `stage_plugin_observers(resources, scope, backends)` and
`submit_resident_in(id, resources)` to retain plugin scope and resource identity;
generation retirement stops these observers with the plugin.

Operation failures retain their classification, application refusal code and source
diagnostics across carriers. Applications can attach named, non-secret context
with `Failure::with_detail` and read it with `Failure::detail`; a plugin admission
consumer can use these fields for actionable errors without parsing prose.

Deadline refusals name the operation, one-based attempt, applied budget and its
configuration source (`operation policy.timeout`, `operation policy.attempt_timeout`
or a per-call gRPC deadline override). For example: `host call refused (-32607):
operation connectivity.grpc_wait_until_grpc_ready attempt 1 exceeded its 10 s deadline
(configured by operation policy.attempt_timeout)`. Cancellation refusals also name
the operation and attempt. Retry exhaustion preserves the last attempt failure;
retry overrides do not change either deadline.

Startup configuration errors expose `StartupError::report()` with separate root
failures and consequences. Each issue retains its original `Failure` together
with resource and operation identity where applicable. Carriers preserve that
report as the `harmos.startup` structured detail on the existing failure;
`operations::StartupReport::from_failure` reads it without parsing messages.
Original refusal codes, detail maps and diagnostics remain in the report. Display
renders readable root prose rather than Rust Debug output.

Native and plugin stream revisions are derived automatically in the Rust core when a declaration
becomes a runtime definition. The input is `harmos.stream.v1\0`, the window as a
little-endian `u64`, and the canonical row-schema encoding in
`crates/harmos-domain/src/schema/identity.rs`. SHA-256's first four bytes are read
as a little-endian `u32`, with zero mapped to one. Field order, names, wire tags,
kinds, cardinalities and graph edges participate; graph indices and presentation
metadata do not. Field order remains significant for the Postcard row encoding.

A changed row or window receives a new revision on reload, without an author
version or backend restart. Atomic activation commits select the current revision;
failed staging preserves the previous selection. Revisions are opaque hashes:
never choose current by comparing their numbers. `StreamManager::definitions()`
returns current definitions, `current_publication(id)` selects the committed
publication, and `publication(id, revision)` pins a retained revision. Existing
queries and watchers retain their descriptors; live `StreamFeed` batches carry
each row's definition, including newly committed revisions. Retired live windows
are released after storage consumers and live feeds have taken their rows;
readers retain only the windows they still own.

Persist stream rows with `Definition::version()` as the storage `SchemaKey.version`
and `Definition::schema()` as that revision's definition. Keep each revision's
rows under its own key, including across process restarts. A different plugin owner, native collision
or equal hash with a different canonical row remains an admission error. Rename
the stream and report a hash collision. A transport restart inside one committed
activation may only renew its frozen inventory; reload the plugin to change it.

Streams expose `ID`, `WINDOW` and `definition()`; `StreamSpec`, `StreamRow` and
native `Stream<A>` have no authored `VERSION`. Typed native readers select the
current definition by identity and compiled row type. Low-level declarations use
`Definition::declared::<Row>(id, schema, window)` and return a derived definition.
Every stream definition owns a schema; `Definition::schema()` returns it directly.
Publication inventories,
listener sources and `host.publish` do not accept schema ordinals. Listener
publication binding uses `(id, after)` and `frontier(id)`.

`#[record]` and `#[transaction]` derive schema-only revisions through
`harmos_domain::schema::identity::revision(schema)`. Its input is
`harmos.record.v1\0` followed by the same canonical graph encoding; SHA-256's
first four bytes become a little-endian nonzero `u32`. Presentation labels,
Rust module paths and graph indices do not participate. Generated `DEFINITION`
identity tuples are removed: use `T::ID` and `T::definition()` through `Identified`.
Manual `Identified` implementations declare only `const ID: &'static str` and
provide `Described`; the default derives the revision. Typed stored objects use
the same rule, including `ValidatedSchematic`.
`restore(revision, payload)` now accepts only the derived revision.

Records, transactions and their nested Serde domain types declare their schema
with `#[derive(harmos::Described)]`; an existing `#[message]`
already supplies the schema. This schema-only derive retains the existing
Serde payload codec. It describes named and tuple structs, transparent newtypes,
and externally, internally, adjacently tagged or untagged enums. Serialization
names follow `rename`, `rename_all` and `rename_all_fields`, including directional
names. `default` and deserialization-only attributes do not change the stored
shape. `skip` and `skip_serializing` remove named fields or variants;
`skip_serializing_if = "Option::is_none"` and `"Vec::is_empty"` describe omission
separately from nullability and repeated values. Skipped positional fields,
arbitrary omission predicates, `flatten`, custom serializers and conversions are
refused at compile time with the attribute and enclosing type. Such forms require
an explicit, accurate `Described` implementation. Serde wire metadata survives
schema serialization and participates in the same canonical encoding used by
revisions and fingerprints. Existing schemas without that metadata retain their
canonical bytes. Record listeners declare `record = "identity", row = Type`
without `version =`; binding checks identity and the declared row schema.
System facts use `SystemFact::new(source, name, bytes)` for opaque bytes or
`SystemFact::encode(source, name, &value)` for a described Serde payload. Their
stored revision carrier remains necessary; callers no longer supply its value.

Managed journal entries and snapshot definition inventories store derived
revisions. Open refuses a stored host record or transaction whose definition differs,
naming its identity and directing the caller to choose a fresh storage root.
Snapshots check the frozen definition inventory even when the record precedes
the retained tail. Failed open preserves the stored history. Development storage
has no migration; use a fresh root after a host state shape change. Unused group and snapshot
wrapper ordinals are removed. ABI, failure details, transport framing, journal
file framing, artifact manifests and `PayloadEncoding::PostcardV1` retain their
existing compatibility versions. Schema encoding stays v1. Rebuild plugin artifacts
with the matching SDK.


Plugin scopes register streams and listeners, operations and services/observers,
and inert registrations/sections. They do not register typed state records,
transactions or object catalogs: those belong to the compiled host composition.
Listeners and observer activations have no runtime state replayed from snapshots
or the state journal. A plugin requests host transactions through operations;
the host owns their persistent types.

Plugin storage records, objects and streams reference the exact `SchemaKey` and
persisted `StorageSchema` graph written with their bytes. Historical queries
resolve those stored graphs, independently of the installed plugin. Select an
exact schema for incompatible shapes, or explicitly project compatible fields.
A typed object read of an old revision with a different current definition
refuses; it never interprets old bytes as the new type. Current writes follow the
current definition. No automatic conversion feeds old values into current state.
Object revisions whose exact definition is absent from the current catalog are
retained, including after new shapes are written. Backend adapters must honor
`ObjectRetention::All` by leaving those revisions untouched.

`SystemFact::new` and `SystemFact::encode` retain a schema graph for `Guest` and
`Sidecar` sources inside the journal payload and snapshot tail. Recovery checks
that graph against the stored revision and exposes it with `SystemFact::schema()`;
the original payload remains available even without the plugin. Encoded facts
contain their original Serde JSON bytes; opaque facts retain their original bytes
and the byte-vector definition. These historical facts do not mutate host state.
`Work` facts retain their host representation. Rebuild consumers and plugins with
the matching SDK, and use `Prepared::system` and `SystemFact::restore` in custom
journal adapters so definitions survive capture, snapshots and replay. Roots with
older plugin facts lacking their definitions may need a fresh root once. Schema,
storage and payload encoding versions stay v1; there is no migration or reset.

Classification is scoped to one startup/prepare pass. If the pass contains a
runtime preparation/construction failure, every `MissingResource` in that pass
is a consequence, regardless of reporting order. Other failures remain separate
roots. Without a preparation failure, missing resources are roots themselves.
This rule describes a failed pass, not a resource dependency graph; Display
summarizes consequences as operations unavailable because startup failed.

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
| `grpc` | gRPC, reflection and TLS; settings select their use. Includes `generate-proto`. |
| `generate-proto` | Protobuf descriptor projection used by runtime transport. |
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
for calls from other plugins or clients; the default is `Private`. Applications and plugins share
`Lifecycle` with async prepare, initialize, stop and cleanup. Plugin lifecycle
contexts are `harmos::sidecar::Supervisor` or `harmos::guest::kit::Context`.
Build tools use `harmos-packaging` for inert inspection and generation.
`harmos-storage` owns the Arrow query and persistence engine plus private query statistics and downsampling;
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
launching and Wasm componentization. Inert bundle inspection and schema generation
do not enable Cargo discovery, file watching or subprocess supervision. Select
`build-tools` for those authoring APIs; `sidecar` and `guest` imply it.

Applications own project creation and authoring commands. Use the reusable
`harmos_packaging::Build`, `package`, `proto` and `python` APIs from those commands;
Harmos's repository tasks build and package its examples through the same APIs.

### Example Installation

CI packages retain `schematic-<version>-<platform>.zip` demo bundles for
`linux-x86_64` and `windows-x86_64`. Extract the complete ZIP and run
`schematic-cli` (`schematic-cli.exe` on Windows); `--guest` selects the bundled
Wasm backend. Keep its `plugins/` directory beside the executable. Package
checks run both backends from an isolated extraction. Examples, test reports and
fixtures remain source or CI artifacts. Release downloads include generated
`CHANGELOG.md` and `SHA256SUMS`; the internal upload inventory is not a download.

`harmos_packaging::package` consumes the existing build receipt and verifies every artifact and
schema byte stamp before bundling. It never runs Cargo. Each sidecar bundle
records the registrations its executable prints for `CATALOG_EXPORT_FLAG`. Relocated archives use
explicit `Build::relocate(from, to)` path mapping while preserving hashes.
Library consumers can stamp existing outputs with `Artifact::read` and
`Schema::read`, then construct a `Build` without another compiler invocation.

`harmos_packaging::build` prepares selected native plugins or converts guest core
modules into inspected components. Producer build scripts export known catalogs
to their own `OUT_DIR/grpc/<catalog>/`; collected builds retain those exact paths.
Native collection requires Rust's `llvm-tools` component. Shipping copies omit
debug information, while original compiler outputs remain available for debugging.
The 128 MiB bundle limit applies to shipping bytes.

`harmos_packaging::python::generate` consumes an exact exported catalog and
application-owned package settings. Selected contracts without Protobuf bindings
and generated name collisions fail explicitly. Each generated package owns a
private descriptor pool, so producers can reuse Protobuf names without sharing
message-class identity. Connections and operation registration remain with the
application.

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
original declared payload. Response clones share its owner.
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
retain their authenticated context. Existing
explicitly anonymous routes keep their behavior when no authorizer is configured.

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

`SnapshotPolicy.max_bytes` bounds each logical snapshot segment at capture and
recovery. Large state values span segments; undo, receipt and recorded-tail entries must
individually fit that bound. State, undo history, request receipts and the retained
tail remain in the checkpoint. A refusal reports the entry size, segment bound and
number of journal groups the next boot must replay. Unavailable sizes are marked
with `size_known = false` rather than presented as a measured zero.

`RecoverySnapshot.payload` is a `SnapshotPayload`: an immutable temporary-file spool
with an ordered `SnapshotManifest` (logical position, count, total bytes and each
segment's SHA-256). `SnapshotWriter` fills segments up to the bound; `finish_segment`
also permits an earlier boundary when importing stored segments. `reader()` provides
an independent read-only cursor. Readers verify the manifest and all segment hashes
before using the snapshot, then decode in order without a whole-payload byte buffer.
Queue admission reserves a segment rather than the whole checkpoint; the small
manifest remains resident with the spool owner through retries.

The filesystem backend writes a manifest header, ordered checksummed segment frames
and a matching commit manifest to `state.json.pending`. Each frame carries the raw
segment SHA-256; the manifest digest binds their ordered lengths and hashes. Manifest
records have a protocol bound of 4096 bytes. `max_checkpoint_bytes` bounds each encoded
segment frame, including its integrity envelope. The file is flushed and synced before
atomic replacement of `state.json`; only then may the covered journal be compacted.
An interruption between segments leaves the previous snapshot and journal intact.
The journal format is unchanged. Existing single-payload checkpoint files remain
readable; their imported manifest has no separate logical position, so runtime recovery
uses the position in the original payload and its journal-boundary validation.

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
preserving its original file object even when its pathname is replaced or deleted.
On Windows, exclusive handles are rejected. A separate read open normally keeps
the caller's OS cursor independent. If reopening is denied, including for a
delete-pending object, reads use explicit offsets on the supplied handle; these
reads can move the OS cursor shared with caller clones. Capture offsets remain
independent. Keep the source bytes stable through a stable capture or freeze.
`StorageManager::freeze_opened_file` creates a private
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
`operations::RequestClaims::retain(owner)` and submit using `OperationHost::call_owned` or
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
`Median`, ordering for `Min`, `Max` and `Last`, and private statistics for floating
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

Snapshot pin acquisition and release use one separate worker slot alongside
ordered data and recovery writes and immutable file publication. Pruning and
deletion remain exclusive with pin registration and release. Typed writes and
durable frontier advancement retain their ordering.

`storage::query::quantiles::Quantiles` stores finite f64 projections for exact
linear percentiles without decoding source evidence again. Its constructor
bounds numeric payload memory, including spill buffering; `values` consumes the
spool with a separate caller-supplied rank scratch budget and cancellation check.
A zero payload budget spills immediately. Rank scratch exhaustion returns an
error; selection never substitutes an approximation.
Both `push` and `values` may perform blocking file I/O; async consumers run their
fold and rank selection on blocking workers.

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
    visibility = Public, resources = [Device], replay = harmos::operations::ReplaySafety::Idempotent)]
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
`visibility` to `Private`; `Private` and `Public` are the two visibility forms. The operation struct is the request message;
its runtime implementation declares only `Error` and consuming `run` (plus optional error classification).
Private operations cannot declare `grpc(...)`; use `Public` for gRPC exposure.
The macro rejects Private bindings at compile time, and runtime registration refuses them.
Private operations admit their declaring plugin and the host. Public operations admit everyone
connected, including other plugins and external clients subject to the client authorization hook.

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

use harmos::operations::InvocationExt;

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

let retry = settings.retry;
let runtime = harmos::Runtime::<Product>::launch(origin, settings).await?;
let reading = runtime.operation(Read { channel: 3 }).retry(retry).lane(Lane::Interactive).await?;
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
- Activation: the complete registration requires all resources and callee contracts. Cross-plugin calls require `Public`; host callers may also invoke `Private` operations; missing targets and incompatible fingerprints are refused before execution.
- Run: operation errors, deadlines and carrier faults remain fallible.

`input = Item` and `output = Item` derive `StreamEndpoints`: `()` for unary calls, `StreamInput<Item>`, `StreamOutput<Item>`,
or their pair. Borrow endpoints with `ctx.streams() -> &Self::StreamEndpoints`; input receives serialize through an asynchronous mutex.
Every authoring surface returns the SDK `Invocation<H: OperationHost, O, E = ()>` from `ctx.operation(request)`.
Await it to receive `Response<T>`, which dereferences to the value; `into_owned()` moves the payload out, cloning only when shared.
`.start().await?` returns an owned instance, and `.endpoints(...)` supplies its declared endpoints. Builders are inert until awaited or started.
Runtime `.lane(...)` is supplied by `operations::InvocationExt`; templates have no lane option. Use `operations.submit_at` for dynamic registrations.

The operation macro owns the default policy. An option overrides only that call; a later call without options uses the declaration unchanged.
Pass `.retry(settings.retry)` from settings, parameters or configuration. Retry overrides never change operation or attempt deadlines.
Enabling call-site retry requires declared `Idempotent` or `Deduplicated` replay safety; unknown safety is refused before execution.
Declaration defaults retain their existing semantics: transient failures may retry with unknown safety, while ambiguous failures require replay safety.
Sidecar and guest child envelopes carry the optional retry policy to host admission. The host execution loop owns every attempt;
`operations::middleware::Middleware::attempt` receives this process-local observation for every handler attempt, including refusals, timeouts and retries:

```rust,ignore
AttemptObservation {
    attempt: usize, // Zero-based, matching the handler context.
    outcome: OperationOutcome,
    started_at: AttemptStartedAt {
        instant: tokio::time::Instant,
        timestamp: state::Timestamp,
    },
    elapsed: std::time::Duration,
}
```

Timing surrounds the handler call, including asynchronous suspension and cancellation grace.
It excludes queueing, admission, initialization, child draining, cleanup and middleware callbacks.
Each retry takes fresh readings. The timestamp identifies the start; elapsed uses only the
monotonic readings. Before the first invocation, `operations.set_attempt_clock(AttemptClock::new(clock))`
can inject a clock returning `AttemptStartedAt`; the default reads the operation executor's
monotonic clock and the system timestamp. This does not change deadline clocks or timers.
No attempt observation is emitted for admission refusals that never enter a handler.

Owners (plugin, artifact, sidecar record and stream owners) are one `[a-z][a-z0-9_]*` segment, such as `obc_plugin`.
Dots and hyphens are refused in owners, including by `#[plugin]` at compile time. Reserved identity rules still apply.
Local operation IDs use dotted segments `segment(.segment)*`, each with that same grammar. The runtime prepends the owner once:
`temperature.read` becomes `plugin_name.temperature.read`. Built-ins include `host.operations.cancel`, `harmos.initialize`
and `storage.append_file`. `codec = manual` preserves an explicitly authored message codec.

Started operations return the same typed `Response<T>` from `join()` and
`cancel().await`. `request_cancel()` sends only the signal. An implementation
can await `ctx.cancelled()` and return its normal response or a defined failure;
both retain their result. A missing response becomes `Refusal::Cancelled`. Set
`OperationPolicy::cancellation_timeout` to the grace duration. Zero preserves
immediate abort. Grace never extends the remaining attempt or overall timeout,
and cancellation never retries. Final responses still wait for owned children
and cleanup. Dropping a handle requests cancellation without retaining its result.

```rust
let handle = runtime.operation(request).start().await?;
let summary = handle.cancel().await?;
// Inside Operation::run:
ctx.cancelled().await;
Ok(summary.into())
```

For gRPC, every exported operation also has a bidirectional `Run<Method>` RPC.
Its opening frame is `request` (tag 1), input rows are `item` (tag 2), and
`cancel` (tag 3, an empty message) requests cooperative cancellation.
`finish` (tag 4, an empty message) closes input while retaining cancellation
control; Rust handles expose `finish_input().await`. Responses
contain output `item` (tag 1) or final `response` (tag 2). Keep the response stream
open through cancellation. An HTTP/2 abort abandons the result. Rust callers use
`harmos::grpc::client::start::<Operation>(channel, request, transport_settings)`
and the returned handle's `cancel().await` or `join().await`; both return the
normal typed response or a gRPC status, including `Code::Cancelled`.
The caller supplies transport and buffer settings; operation deadlines come
from the server's operation policy. Generated Python descriptors include
`run_grpc_path`, `run_request` and `run_response`, with `Invoker.start` and
`OperationHandle.cancel` transport interfaces. Harmos emits protobuf contracts
for external TypeScript generators and does not generate a TypeScript execution
client. Ordinary RPC methods remain available alongside managed calls.

Portable request and response integer fields support `u32`, `u64`, `i32` and
`i64` across in-process, sidecar, guest and gRPC carriers. `u16` is rejected by
the message macro; represent ports with `u32` and validate their range in the
operation. `u8` is supported as byte buffers (`Vec<u8>`), not as a scalar field.
`usize`, `isize`, narrower signed integers and 128-bit integers are unsupported.

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

`OperationPolicy::timeout` bounds total duration for operations without streams.
For an operation with an input or output stream, it bounds idle time instead:
each transferred item rearms the deadline, so a continuously progressing transfer
can outlive `timeout`. Silence before the first item and while waiting for the
final response is also bounded. Streaming execution uses this idle budget rather
than a total `attempt_timeout`; unary attempts retain their total attempt bound.
Admission applies the `grpc-timeout` cap only to non-streaming operation budgets.
Streaming clients omit total `grpc-timeout` to follow the idle policy. The listener bounds
middleware and stream opening separately, and applies the operation policy as a
traffic idle deadline through buffered output draining. Operation execution also
tracks item movement, so partial wire frames cannot keep a stalled handler alive. FOLLOW streams must emit heartbeats within their idle budget, even
when no data changes. Request retry rules and all size limits are unchanged.
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

Sidecars start with a cleared environment. On Windows, Harmos passes through
these parent variables when present: `SystemDrive`, `SystemRoot`, `windir`,
`ProgramData`, `ALLUSERSPROFILE`, `ProgramFiles`, `ProgramFiles(x86)`,
`ProgramW6432`, `CommonProgramFiles`, `CommonProgramFiles(x86)`,
`CommonProgramW6432`, `PUBLIC`, `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`,
`LOCALAPPDATA`, `APPDATA`, `USERNAME`, `USERDOMAIN`, `COMPUTERNAME`, `PATHEXT`,
`ComSpec`, `NUMBER_OF_PROCESSORS`, `PROCESSOR_ARCHITECTURE`, and `OS`.
`SystemRoot` falls back to the Windows system directory if missing or empty;
`TMP` and `TEMP` use the host's temporary directory. On Unix, only a nonempty
`TMPDIR` is inherited; `HOME`, `USER`, `LANG`, and `LC_*` remain cleared.
`PATH` and arbitrary parent variables are cleared on both platforms.
Use `Executable::variable` to supply additional variables or override inherited
values, and `Executable::directory` to choose the child's working directory.

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
failed cleanup prevents a fresh physical attempt of that generation and its dependents.
An unrelated reload does not retry retained failed cleanup; explicit initialization
or a reload selecting the failed owner retries it. Plugin restart never replays
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

Native retirement joins observers before SDK shutdown. An observer retains access
to its own generation's operations while its stop is joining; ordinary callers
remain fenced. A carrier already physically joined satisfies native shutdown,
while authored cleanup failures remain failures. Unexpected exits while serving
remain visible faults.

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
cases measure crossing the carrier boundary.

| Suite | What It Measures |
| --- | --- |
| [Storage Transforms](../crates/harmos-runtime/benches/storage/transforms.rs) | In-memory RMS, histogram and downsampling kernels over balanced three-phase samples. |
| [Sidecar Host](../crates/harmos-runtime/benches/sidecar/system.rs) | Real native process: bidirectional 10,000-record, 30 MiB file and 1,000,000-sample transfers, comparing unary batches and streams; startup and command latency. |
| [Guest](../crates/harmos-runtime/benches/guest/main.rs) | Real Wasm component: the same bidirectional unary/streamed transfer workloads, plus startup and command latency. |
| [External gRPC](../crates/harmos-runtime/benches/grpc/main.rs) | Separate server-process startup, client commands over a socket, and isolated Postcard/Protobuf codec costs. |
| [Streams](../crates/harmos-runtime/benches/streams/main.rs) | Concurrent waveform work, codec costs, and native consumers using 64 KiB and 1 MiB payloads. |
| [Plugin Transport](../crates/harmos-sidecar/benches/transport/main.rs) | Native transport throughput and latency, including 64-byte, 1 KiB, and 64 KiB bursts and a paced 10 MiB/s workload. |

Build matching executables and carrier fixtures with `mise run build:rust:bench`,
then run `mise run bench`.

One million samples represent one second at 1 MHz without pacing ingestion.
The paced transport rate is also an input, not a claimed throughput result.
Published comparisons need the revision, hardware, payload, batching and carrier
alongside native Criterion results.

Native Criterion HTML and raw measurements live under `.stkr/benchmarks/criterion/`,
partitioned by compatibility identity and benchmark executable. The runner compares
against the compatible `dev` baseline and reports missing baselines or regressions
in its output. HTML rendering uses Plotters and does not require Gnuplot.

## Documentation

- [Repository Tooling](tooling.md) — workstation builds and shared compiler cache.
- [Runnable Examples](examples.md) — application composition and plugin integration.
- [C++ Sidecar Prototype](sidecar-cpp-prototype.md) — C++20 authoring over the shared Rust core through a C ABI, with build and demo commands.
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
and schema resolution. Consumers pass their `SchemaSettings` through
`Declaration::inspect_with_settings`, `Bundle::build_with_settings`,
`Bundle::inspect_with_settings` and `Executable::at_with_settings`.
Sidecar authors use `run_plugin_with_settings` or
`serve_plugin_channels_with_settings` for the matching native self-inspection.
The existing convenience methods retain Harmos defaults. Memory and work
refusals report attempted usage and the allowed amount; bundle errors also name
the package and executable. Aggregate provides/requires fragments are unsupported.

Operation envelopes are v2 and carry optional call-site retry policy. Protocol,
artifact, journal and schema formats remain v1. Only current shapes are supported;
incompatible envelope versions are refused, with no historical converters or
journal upgrades. Rebuild sidecars and guests against the same SDK.

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

Process observers require a positive declared initial cadence. The macro sets
`Observer::INTERVAL`; an optional synchronous method overrides it once at
activation from the producing plugin's execution resources:

```rust,ignore
#[harmos::observer(interval_ms = 1_000)]
impl Poller {
    // Optional: omit this method to use the declared 1,000 ms cadence.
    fn interval(resources: &harmos::ExecutionContext) -> Result<Duration, Error> {
        let settings = resources.resource::<Settings>()?;
        Ok(settings.poll_interval)
    }

    async fn observe(&mut self, context: &ObserverContext<'_>) -> Result<(), Error> {
        // Collect and publish through context.
        Ok(())
    }
}
```

`interval_ms` is required, accepts a constant expression and must be at least
1 ms, even when an override exists. Manual `Observer` implementations declare
`const INTERVAL: Duration` and inherit the same fallback method. Streams have no
cadence. Static observer inventory contains identity, activation, outputs and
demand keys; it does not advertise the declared default as an effective interval.
A zero runtime duration, resolution error or panic refuses activation before
initialization. Initialization retries and sampling use the captured interval;
changed resources apply on the next activation.

Cadence evidence uses `harmos_sidecar::CadenceReport`:

```rust,ignore
CadenceReport {
    observer: String,
    activation: String,
    interval: Duration,
    scheduled_at: Duration,
    started_at: Option<Duration>,
    callback_duration: Duration,
    skipped: bool,
    refused: bool,
}
```

Scheduled and actual start times are monotonic offsets from the activation's sampling
origin, after initialization. A skipped slot has no actual start, zero callback duration
and `refused = false`. A completed callback records its duration before reporting;
errors and panics set `refused`. Late callbacks still run under the existing Tokio Skip
policy; gaps between returned scheduled deadlines identify omitted slots. Skipped slots
are reported when the next scheduled tick is observed. Cancellation of an unfinished
callback produces no completion report. Existing refused/recovered log lines remain unchanged.

`ObserverHost::report_cadence(&self, &CadenceReport)` defaults to no action. Attachments
invoke that hook and send a `harmos.observer.cadence` push over the existing bounded
transport. A full outgoing queue defers delivery in a task instead of suspending the
observer; reporting adds no timer. Pending reports share the connection/runtime lifetime.
Consumers order received evidence by activation and scheduled time. Direct/native instances
opt in with `Instance::with_cadence_reporting`, which can call the same host hook.
There are no additional timers or sidecar log lines.

The receiver validates the report's schema-derived revision, invokes an optional
`CadenceReporter` application resource and delivers `Notice::ObserverCadence` through
`Supervised`'s existing notice callback. Register `CadenceReporter::for_host(host)` to
forward remote reports to the same `ObserverHost` hook, or `CadenceReporter::new(callback)`
for an application callback. The report is an additive push payload; other transport
shapes and tags are unchanged. Group by observer and activation; differences between
non-skipped actual starts give mean interval and maximum gap, while the maximum
callback duration identifies the longest callback.

## Reloadable gRPC Certificates

`crates/harmos-runtime/src/grpc/tls.rs` owns `grpc::ServerCertificate`.
Create it with `ServerCertificate::from_pem(certificate, private_key)` and pass a
clone to `runtime.grpc().attach_with_certificate(settings, certificate.clone(), handshake_timeout)`.
The settings must have `tls: None`; the positive handshake timeout is caller-owned.
This server-authentication-only path uses HTTP/2 ALPN and no client certificates.
Existing `attach(settings)` and fixed Tonic TLS settings remain available.

Call `certificate.reload(next_certificate, next_private_key)` to validate the PEM
chain and key match before atomically replacing the resolver's certified key.
Clones share the same identity. Invalid replacements leave the old identity active.
New connections authenticate with the replacement; TLS resumption is disabled on
this path so every connection presents the current certificate. Existing TLS
connections and RPC streams retain their negotiated state and continue untouched.
The listener's bind address, admission, middleware and shutdown ownership do not change.
