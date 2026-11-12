# Harmos Product Guide

Detailed composition, feature selection and runnable examples. Start with the [Quick Start](../README.md#quick-start).

Embedded language bindings can stage runtime-owned unary operations with
`operations::EncodedBinding::new(contract, decoding_memory, prepare)` and
`Bindings::encoded(binding, lane)`. `Contract::from_schema` derives the canonical
fingerprint from an owned graph. The codec validates payloads and estimates decoded
allocation; its prepared `NativeInstance` runs through normal admission, retries,
cancellation and settlement. Plugin staging validates the frozen contribution
catalog and retains its original generation. Runtime bindings currently use the
protobuf encoded carrier without a gRPC projection or stream endpoints.

`submit_encoded_with(operation, payload, retry)` and
`PluginCallback::submit_with_retry(bytes, encode, retry)` apply policy overrides to
one call. Unknown replay safety still refuses enabled retries. Existing deadlines,
authorization and attempt records remain authoritative. Embedded observer owners
use `stage_plugin_observers(resources, scope, backends)` and
`submit_resident_in(id, resources)` to retain plugin scope and resource identity;
generation retirement stops these observers with the plugin.

Built-in host operations use generated `harmos_protocol::{host, storage}` messages
directly. Their declarations and handlers stay together in `harmos-operations`; its
former `runtime` feature is removed. Lifecycle requests/results are
`harmos_protocol::host::{Status, Cancel, Watch, InstanceState}`. Register `harmos_operations::lifecycle_catalog()` by value and invoke
`operations.call_protobuf::<harmos_operations::lifecycle::Status>(request)` for a
native unary call. `submit_protobuf` supplies bounded stream endpoints for Watch.
The consumer configures `preparation_bytes` for encoded admission; the callee owns
its timeout and cancellation policy. Status and cancellation expose only instances
owned by the caller. Guest child controls keep their private status codec until
the guest protocol migration.

Bind an opaque invocation scope with `runtime.operations.with_scope("opaque-scope")`.
The runtime's `harmos_runtime::Scope::as_str()` exposes it to native handlers and child
calls inherit it. Plugin authors require a bound scope by calling `Context::scope()`
in Rust, `ctx.scope()` in Python or C++, or `context.Scope()` in C#.
An absent scope produces a refused failure with code `scope_required` and message
"Invocation Scope Is Required: the caller must bind a scope". Harmos assigns no
meaning to the scope and does not require it for operations that do not ask for it.

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

Streams expose `ID`, `WINDOW` and `definition()` through `StreamRow` and the guest
`StreamSpec` projection, with no authored `VERSION`. Typed native readers select the
current definition by identity and compiled row type. Low-level declarations use
`Definition::declared::<Row>(id, schema, window)` and return a derived definition.
Every stream definition owns a schema; `Definition::schema()` returns it directly.
Publication inventories,
listener sources and `host.publish` do not accept schema ordinals. Listener
publication binding uses `(id, after)` and `frontier(id)`.

Register stream definitions as owned lists; no stream catalog enum or
`Application::Streams` selector is required:

```rust
let streams = vec![Temperature::definition(), Pressure::definition()];
setup.settings().register(streams);
```

Startup validates duplicate identities and empty windows. Typed readers and
publishers resolve the registered identity and row type at runtime.

The `Row` derive reads fields for row metadata, stored schema and supported native
message codecs. Declare identity, retention and presentation with a stream value:

```rust
#[derive(harmos_runtime::Row, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Temperature {
    celsius: f64,
}

impl harmos_runtime::streams::StreamRow for Temperature {
    const ENTRY: harmos_runtime::StreamEntry<Self> =
        harmos_runtime::StreamEntry::new("thermal.samples", 8).label("Temperature");
}
```

`StreamEntry::build()` checks the identity, window, schema and presentation before
returning a definition. Guest streams use the SDK `Stream` builder with protobuf row messages.

Lanes are values configured at application startup. The default lane is
unbounded; additional lanes declare their admission capacity in an owned set:

```rust
let analysis = harmos_runtime::operations::Lane::new(
    "analysis",
    harmos_runtime::operations::Capacity::serial(),
);
let lanes = harmos_runtime::operations::LaneSet::new([analysis])?;
setup.settings().lanes(lanes);
setup.catalog(analysis_catalog(), analysis);
```

Building the set rejects duplicate or reserved names and zero capacity, naming
the invalid lane. Registration and per-call lane selection use the same lane
value; `Application` and operation managers carry no lane type parameter.

Typed record and transaction entries compute schema-only revisions through
`harmos_domain::schema::identity::revision(schema)`. Its input is
`harmos.record.v1\0` followed by the same canonical graph encoding; SHA-256's
first four bytes become a little-endian nonzero `u32`. Presentation labels,
Rust module paths and graph indices do not participate. Entry values own stored identities and revisions; use `entry.definition()`.
Record rows select an entry value, which owns their stable identity:

```rust
impl harmos_runtime::state::Record for ExportCompleted {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> =
        harmos_runtime::state::RecordEntry::new("export.completed");
}
```

The row derives `Described` and Serde traits. `RecordEntry::definition()` computes
the stored revision, and `RecordEntry::restore(revision, payload)` refuses another
shape. The native `Identified` implementation delegates to this entry; it does not
repeat the identity. Manual `Identified` implementations for other stored objects
declare only `const ID: &'static str` and provide `Described`; typed stored objects use
the same rule, including `ValidatedSchematic`.
`restore(revision, payload)` now accepts only the derived revision.

Records, transactions and their nested Serde domain types declare their schema
with `#[derive(harmos_runtime::Described)]`; `#[derive(harmos_runtime::Message)]`
also supplies a schema for supported native message fields. This schema-only derive retains the existing
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
canonical bytes. Listener values use `listeners::Entry::record::<Row>(id, record_id, callback)`
or `Entry::stream::<Row>(id, stream_id, callback)`. The callback is a wire
declaration with a `Delivery<Row>` request and unit response; building checks its
policy, visibility and schema. Binding checks source identity and row schema and
refuses a missing callback at startup. Listeners belong to the native host.
System facts use `SystemFact::new(source, name, bytes)` for opaque bytes or
`SystemFact::encode(source, name, &value)` for a described Serde payload. Their
stored revision carrier remains necessary; callers no longer supply its value.

Record registries own typed preparation, restoration and borrowed projection:

```rust
let records = harmos_runtime::state::RecordRegistry::new()
    .entry(ExportCompleted::ENTRY, Facts::ExportCompleted, |fact| {
        let Facts::ExportCompleted(value) = fact;
        Some(value)
    })
    .build()?;
setup.settings().register(records);
```

`Facts` is an ordinary application data union. `registry.project::<Row>(&fact)`
returns a borrowed row only when the typed entry and matching payload are present.
Restoration selects the registered stored identity and derived revision.
Duplicate identity/revision pairs are refused when built or registered.

Object catalogs also own their definitions. Obtain a checked typed handle before
encoding or decoding; an absent entry is an error:

```rust
use harmos_runtime::storage::{ObjectCatalog, ObjectDefinition};

let objects = ObjectCatalog::new()
    .entry(ObjectDefinition::of::<ValidatedSchematic>())
    .build()?;
let schematic = objects.handle::<ValidatedSchematic>()?;
let stored = schematic.encode(&document, max_payload_bytes)?;
let restored = schematic.decode(&stored, max_payload_bytes)?;
```

`StorageManager::with_catalog(objects, backends, settings)` retains the owned
catalog. The storage manager has no catalog type parameter.

All surviving field-reading derives live in
[`sdk/rust/derive`](../sdk/rust/derive/): SDK `Message` describes protobuf values;
`StoredMessage`, re-exported as `harmos_runtime::Message`, describes native
portable values and their Postcard codec, memory use and row metadata.
`Described` describes stored Serde shapes without adding a payload codec, and
`Row` reads row fields. Host data authors explicitly derive `Clone`, `Debug`, `Default`
and any needed Serde traits. Plugin registration uses builder values; the SDK
has no host dependency.

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
component once. Without a flag the CLI attaches the managed gRPC plugin beside it.
Both modes offer validation, route proposals and streaming analysis. In native mode,
the plugin serves unary advice and directory access while the host owns analysis,
upload and exchange streams. The guest executes its shared operation catalog without
native filesystem access.

The [advanced follow-on](../examples/schematic/cli/src/advanced.rs) demonstrates
bounded streaming, cancellation with settlement, and native directory access.
The example's [JSONL storage adapter](../examples/schematic/storage/) shows how an
application owns its persistence format. The example is a separate Cargo workspace
under `examples/schematic/` and is also the system fixture of the Harmos test suite.

## Cargo Features

The public Rust SDK is `harmos`, located at `sdk/rust`. Plugin and client
applications import `harmos::prelude::*`; no host code is compiled by that dependency.
Use `Plugin`, `Operation`, `Stream` and `Observer` builder values and finish a plugin
with `serve()`. Messages derive protobuf descriptors with `#[derive(Message)]`.

Host embedders depend explicitly on `harmos-runtime`. Managed and remote
`harmos.v1` placement is always available on native targets.

| Runtime Feature | Enables |
| --- | --- |
| `plugin-guest` | Wasmtime guest hosting with `harmos.v1` lifecycle and child-control messages. |
| `grpc` | External host gRPC listener, reflection and TLS; enabled by default. |
| `simulation` | Deterministic runtime simulation. |

`harmos-domain` is the native foundation: stored schemas and canonical revisions,
identities, typed records and transactions, preparation/restoration and inverse pairing,
journal positions, native stream row identity, and storage catalogs, selections and transforms.
Lightweight application models can use it without Arrow, filesystem, async or host dependencies.
`harmos-operations` owns operation declarations, typed dispatch and call context, streaming,
policy admission and native failure codecs.
Portable policy values live in `harmos-protocol`; listener activation stays in runtime.
State entries and record registries are owned values. Native and guest plugin authors
use the same SDK builders. Optional `harmos/wasm` adds component lifecycle and child controls.
`harmos-storage` owns persistence and queries; `harmos-runtime` composes it with
host lifecycle and managed state. Build tools use `harmos-packaging` from `tools/packaging`; it sits outside the core libraries.
Runtime reads verified plugin bundles through `harmos_runtime::bundle`; construction uses
`harmos_packaging::bundle::build`. Runtime has no normal dependency on packaging tooling.
The reference host and plugin live in `tools/conformance`, preserving their executable names
and release archive contents.

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
checks run both backends from an isolated extraction. Examples and fixtures remain source or CI artifacts.

Stable releases attach these downloads:

- `harmos-proto-<version>.zip`: definitions under `harmos/v1/` and their behaviour specification.
- `harmos-conformance-<version>-<platform>.zip|tar.gz`: reference host and reference plugin;
  platform names are Rust target triples. Windows uses ZIP; Linux uses tar.gz.
- `harmos-compliance-<version>.zip`: locked Cargo source SBOMs under `crates/<triple>/`,
  the locked Python SDK SBOM under `python/any/`, and per-platform notices under `notices/`.
- `release.json`: release notes, publication date and the `downloads` section. Each download
  has a kind, OS, architecture, format, URL, size and SHA-256 digest.
- `SHA256SUMS`: every attached payload and `release.json`; the checksum file omits itself.

The download index omits itself and `SHA256SUMS` to avoid circular hashes. Registry
packages and example bundles are not release attachments; the changelog is the release body.
Nightly uses the same layout and adds `harmos-reports-<version>.zip` with available
benchmark and coverage reports, plus fixed-name `benchmarks.json`. Each benchmark carries
name, suite, unit, value, baseline value, revision and regression threshold. The final
notes append coverage per language, benchmark regressions beyond their threshold, all
suite results and the pipeline URL. Missing or failed reports do not prevent publication;
the notes retain their suite outcomes. The internal upload inventory is not a download.

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

Harmos serves no built-in operation by default. Place `builtins::lifecycle_catalog()` on a
runtime lane with `setup.catalog(builtins::lifecycle_catalog(), lane)` to provide the
Status, Cancel and Watch operations (gRPC `/harmos.host.v1.Operations/Status`, `/Cancel`
and the server stream `/Watch`) over the manager's instances. Each answers only
for active instances the caller submitted or holds a resident lease on; unknown,
foreign and settled ids are refused alike. A watch streams every status change
from the moment it starts until the instance stops or settles. Plugins reach them like any other operation,
so they must declare them as requirements.

Register `harmos_operations::host_catalog()` by value on a chosen lane to serve the `Catalog`
declaration at `/harmos.host.v1.Host/Catalog`. Its generated request is
`harmos_protocol::host::Catalog`, with `kind` (`Operations`, `Schemas`, `Streams`
or `Objects`) and optional `schema`, `after`, `rows` and `bytes`. Defaults are
64 rows and 64 KiB; maxima are 256 rows and 4 MiB. `CatalogPage::operations`
contains generated operation metadata, including descriptors, provider identity,
generation, policy, selected grants and current gRPC availability.
`CatalogPage::entries` retains the UTF-8 JSON array for stored schemas and
identities. The opaque `next` cursor continues a bounded page.

Stored kinds need the storage manager and a `host.catalog` grant scoped
`schemas:*`, `streams:*` or `objects:*` (`<kind>:schema:<id>` with `schema`).
Listing operations needs no grant. Discovery applies readiness, caller visibility
and dependency rules; `conditional_access` means invocation still checks
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
`harmos_runtime::builtin::storage_registration`.

`ArrowSchema::from_ipc` and `ArrowBatch::from_ipc` consume encoded `Vec<u8>`
buffers. `QueryArrow::output` and `TryFrom<QueryOutput>` consume their inputs;
uniquely owned encoded buffers move through these conversions. Retained aliases
continue to share their existing allocations.

```rust
let mut reader = storage.query(harmos_runtime::storage::Query { at, source, transforms, slice }).await?;
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
`From<harmos_runtime::storage::Error>`. Consumers bound their own conversion allocations.

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

`harmos_runtime::filesystem::input::read(path, limit)` bounds regular-file reads.
`harmos_runtime::filesystem::publication` owns staged atomic installation, replacement,
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

## Native Operation Values

Keep a request shape, policy, wire description and plain handler together. A
request derive reads the response and optional input/output types; the operation
value owns identity, visibility, replay policy, callees and grants. Resources
belong to the handler registration.

```rust
use harmos_runtime::operations::{
    Catalog, FailureKind, FunctionContext, InvocationError, Operation,
    OperationSpec, Resource, ServingError, Visibility, WireDeclaration,
};
use harmos_runtime::Response;

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Reading { value: f64 }

#[derive(harmos_runtime::Message, harmos_runtime::Request, Clone, Debug, Default)]
#[request(response = Reading)]
struct Read { channel: u32 }

static READ_DESCRIPTION: WireDeclaration =
    WireDeclaration::request::<Read>("temperature.read", POLICY)
        .visibility(Visibility::Public);
const READ: OperationSpec<Read> = OperationSpec::new(&READ_DESCRIPTION);

async fn read(request: Read, ctx: FunctionContext<Read>)
    -> Result<Response<Reading>, InvocationError>
{
    let device = ctx.resource::<Device>()?;
    let value = device.read(request.channel).await;
    Ok(Reading { value }.into())
}

fn operations_catalog() -> Result<Catalog, ServingError> {
    let operation = Operation::function(
        READ, &[Resource::of::<Device>()], read, |_| FailureKind::Permanent,
    )?;
    Catalog::new().entry(operation).build()
}

// Inside Lifecycle::prepare, after opening the application's Device:
setup.resource::<Device>(device.clone());
setup.catalog(operations_catalog()?, lane);
```

`POLICY`, `Device` and `lane` are application values. See the
[resource and nested-call story](../crates/harmos-runtime/tests/operations/resources.rs)
for a complete registration and handler. `Catalog::extend(other)?` composes owned
catalogs; `entry_named(factory, id)?` registers an explicit alias. Put platform
`#[cfg]` attributes on ordinary modules and the corresponding builder statements.
Private operations omit gRPC exposure; registration rejects incompatible metadata.

Application lifecycle preparation owns resources and registers their concrete
`Arc<T>` values. Runtime startup checks catalogs, resources and lanes before
initialization. Test overrides still use `Runtime::launch_with` and replace
resource lookups after application preparation. Owner cleanup closes acquired
resources even when validation or initialization fails.

`ctx.resource::<T>()` is fallible. `ctx.operation(CALLEE, request)` checks the
explicit callee value against `OperationSpec::calls(&CALLEES)`. Undeclared access
fails when attempted and names the registered caller and resource or callee.
Activation validates the complete resource and callee inventory, including
visibility and fingerprints; no type-level membership proof is required.

`#[request(input = Item, output = ResultRow)]` selects typed stream endpoints;
`ctx.streams()` borrows them. Plain functions retain request cloning across retries,
cooperative cancellation, endpoint settlement and declared error classification.
The operation policy supplies defaults; per-call options override one invocation.
The standalone Rust plugin SDK uses its protobuf `Message` derive and its own
operation builders; generated native host declarations are described below.

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
can inject observation readings returning `AttemptStartedAt`; the default uses the host
clock for both readings. This observation override does not change deadlines or timers.
No attempt observation is emitted for admission refusals that never enter a handler.

Each host owns one cloneable `harmos_runtime::clock::Clock`. Its `read()` returns a
monotonic `instant` and a `date` derived from a fixed epoch plus elapsed time.
Operation scopes expose `scope.clock()`; `sleep`, `sleep_until`, `timeout` and
`timeout_at` use that same source. Record envelopes, default attempt observations,
storage expiry and polling, lifecycle bounds and client-facing gRPC bounds share it.
Storage binds to the host before its worker starts; an already running engine with a
different clock is refused. `setup.settings().clock(clock)` selects a source during
composition. `Clock::with_clock(source)` accepts a `clock::Source` owning readings
and timer registration, so callers do not depend on its scheduler.

`Clock::virtual_at(epoch)` returns the same clock type plus a `Controller`. The
source owns a cancellable timer queue ordered by deadline and registration order;
Tokio and transport keep running normally. `controller.read()` exposes both readings.
`advance_by(duration)` visits due timers in order. `advance_until(bound, condition)`
stops when the condition holds and otherwise returns an `AdvanceError` listing pending
timer IDs and their deadlines at the virtual bound. Active external work can keep the
clock frozen; the test runner's separate hang guard bounds that case.

Register work before scheduling it with `let mut activity = clock.activity("operation");`
and retain the guard through settlement. Calls, unanswered observer ticks and host disk,
process or transport work each need a guard; concurrent branches need separate guards.
Use `activity.wait(clock.sleep(duration)).await` only when that branch's sole outstanding
work is the clock wait. A timeout guarding active work must retain active accounting.
The wake restores activity before scheduling the continuation. Dropping a wait removes
its timer and restores its guard; dropping the guard settles the work. These hooks must
be installed by each invocation, publication and I/O owner: unregistered work is invisible
to the controller. Device-driven streams remain active while awaiting external samples.

`clock.interval(period)` schedules its first tick one period later, preserves fixed
cadence and returns the tick's readings. Missed ticks are delivered in deadline order.
Use `activity.wait(interval.tick()).await` between observer answers and retain active
accounting while answering each tick. Zero periods are refused.

Simulation uses this virtual source with its existing manual advancement interface,
a Unix epoch and the same clock across reboot. Settling work drives queued timers;
manual advancement visits each deadline precisely, without Tokio's millisecond rounding.
Recovery, replay and inverse-law oracles retain their existing meaning. Filesystem and
device timestamps remain source evidence. Directory watcher polling and Wasmtime epoch
interruption remain physical mechanisms.

Owners (plugin, artifact, sidecar record and stream owners) are one `[a-z][a-z0-9_]*` segment, such as `obc_plugin`.
Dots and hyphens are refused in owners, including by `#[plugin]` at compile time. Reserved identity rules still apply.
Local operation IDs use dotted segments `segment(.segment)*`, each with that same grammar. The runtime prepends the owner once:
`temperature.read` becomes `plugin_name.temperature.read`. Built-ins include `host.operations.cancel`, `harmos.initialize`
and `storage.append_file`. A manual native message codec remains an ordinary trait implementation.

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
// Inside the plain handler:
ctx.cancelled().await;
Ok(summary.into())
```

Host applications can register Prost-generated request, response and stream messages directly
with `operations::ProtobufOperation` and `Registration::new(lane).protobuf::<Declaration>()`.
Host lifecycle, clock and discovery declarations use the protocol crate's generated messages.
Storage status, file mutation, read/capture/follow and directory operations also accept generated requests;
use `Manager::call_protobuf` for unary calls or `Manager::submit_protobuf` for native streams.
`Manager::download_protobuf` provides typed generated output items through `next` and a generated
completion through `join`. Plugin-scoped callers use the same helper with their original scope.
Supply the caller's preparation budget and stream limits in operation settings and submission.
Storage kernels retain their native storage values and persisted encodings.
`Manager::exchange_protobuf` sends generated input and receives generated receipts; call
`finish_input` before draining the final receipts and joining. `Manager::upload_protobuf`
finishes file input and returns the generated durable file reference. Scope-aware callers
keep per-batch grant checks throughout an exchange.
Schema registration uses the generated `SchemaGraph`: references index its definition table,
scalar fields cannot reference definitions, and representation indices must be unique.
The storage schema validator checks the graph before mutation admission.


The declaration's `GRPC` is optional; private native operations leave it `None`.
Declare typed callees in `OPERATIONS` and invoke them through the execution context:

```rust
const GRPC: Option<Grpc> = Some(Grpc::new("example.host.v1", "Operations", "Run"));
const OPERATIONS: &'static [ProtobufDependency] = &[ProtobufDependency::of::<Callee>()];
// Inside the handler:
let response = context.call_protobuf::<Callee>(request).await?;
```

Messages implement `prost_reflect::ReflectMessage` using their authoritative descriptor set.
The declaration owns `ID`, `GRPC`, `POLICY`, `VISIBILITY`, `RESOURCES` and its handler; unary
endpoints use `()`, input uses `ProtobufInput<T>`, output uses `ProtobufOutput<T>`, and a pair
supports both directions. Handlers receive generated requests and return generated responses.
See the [declaration example](../openspec/changes/grpc-plugins/design.md#generated-host-messages).
Reflection preserves original protobuf names, field encodings and presence. Decoding-memory
admission bounds generated allocations before Prost decoding. Built-ins and native example operations no longer use `Portable` projections.
Portable projection machinery serves native stored message values.

`operations.submit_encoded` and `call_encoded` accept and return protobuf payloads.
Use `call_protobuf`, `download_protobuf`, `upload_protobuf` or `exchange_protobuf` for
typed native declarations. Assignment catalogs use the generated
`harmos.host.v1.OperationAssignments` messages, with frozen identity validation.

Guest commands, answers and child controls use authoritative `harmos.v1` protobuf
messages. Built-in `storage.hash_file` and `host.publish` calls use their generated
contracts; no private guest payload mirrors are retained. Fingerprints, provider
generations and host grants remain checked. Stored data encoding and schema
revisions are unchanged.

### Generated Rust Clients

`harmos::Client` uses a caller-supplied Tonic channel and `ClientSettings`.
The caller sets the wire-message limit, outgoing item count, outgoing byte budget and required
`retained_bytes: NonZeroUsize` from application configuration. For example:

```rust
let retained_bytes = std::env::var("HARMOS_CLIENT_RETAINED_BYTES")?.parse::<std::num::NonZeroUsize>()?;
let settings = harmos::ClientSettings { retained_bytes, message_bytes, input_items, input_bytes };
let client = harmos::Client::new(channel, settings)?;
```

`retained_bytes` bounds descriptor-checked decoded allocations and SDK-owned queued values
across client clones and calls. Repeated, map and nested values are accounted before Prost
decoding. A claim ends when its bare value reaches the caller; the caller owns its memory
thereafter. Admission refuses before decoding when capacity is unavailable; raise the configured
budget or request smaller pages.
`catalog` reads one bounded discovery page; continue with its opaque `next` cursor.
Bind the selected `harmos_protocol::host::OperationContract` to generated application messages:

```rust
use harmos::client::Method;
use schematic_protocol::{ValidateSchematic, Validation};

let method = Method::<ValidateSchematic, Validation>::new(contract)?;
let response = client.call(&method, tonic::Request::new(request)).await?.into_inner();
let running = client.start(&method, tonic::Request::new(next_request)).await?;
let response = running.cancel().await?;
```

`Method::new` checks the advertised fingerprint and the reachable generated descriptor graphs.
For streams, bind every declared direction with `with_input::<Item>()` and
`with_output::<Item>()` before starting. Managed calls expose `send`, `finish_input`,
`next`, `request_cancel`, `cancel` and `join`; they retain the final generated result
separately from output items. `take_input()` transfers the sole input writer to an
independent task so uploads and downloads can progress concurrently; call its `finish()`
after the last item. The receiving handle retains cancellation and final settlement.
Cancellation observes the final response or gRPC status,
including binary status details. Caller request metadata, extensions and timeout overrides
travel with the opening request. The server continues to own operation deadlines and retries.
Dropping a call abandons its result. No reconnect, replay or automatic retry is added.

The [runnable Schematic client](../sdk/rust/examples/client.rs) discovers and calls a
generated application contract without linking domain, storage, operations or runtime.
Build it with `mise run build -- --package harmos --example client`; pass a running
Schematic application's gRPC URL when launching `target/debug/examples/client`, with
`HARMOS_CLIENT_RETAINED_BYTES` set to the application's decoded-memory budget in bytes.
The dependency proof at `sdk/rust/tests/dependency_closure.py` traverses normal/build
workspace dependencies and the client example's root development dependencies.

For gRPC, every exported operation also has a bidirectional `Run<Method>` RPC.
Its opening frame is `request` (tag 1), input rows are `item` (tag 2), and
`cancel` (tag 3, an empty message) requests cooperative cancellation.
`finish` (tag 4, an empty message) closes input while retaining cancellation
control; Rust handles expose `finish_input().await`. Responses
contain output `item` (tag 1) or final `response` (tag 2). Keep the response stream
open through cancellation. An HTTP/2 abort abandons the result. Rust callers use
`harmos_runtime::grpc::client::start::<Operation>(channel, request, transport_settings)`
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
the native `Message` derive; represent ports with `u32` and validate their range in the
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
runtime.unregister(&harmos_runtime::QualifiedId::parse("host.poller")?).await?;
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

### gRPC Plugin Placement

From `harmos-runtime`, attach a `harmos.v1` endpoint with
`runtime.attach_grpc(placement, settings).await`. The types live in
`harmos_runtime::plugin::grpc`. `Placement::Managed(Managed { executable, arguments,
environment })` starts an executable, hashes its bytes with SHA-256, reads its
loopback readiness address and drains standard output. A supplied or inherited
`HARMOS_ADDRESS` overrides that dial address. `Placement::Remote(url)` owns only
the connection and reconnects independently after transport loss.

Pass `PlacementSettings { connect_timeout, reconnect_delay, stop_grace, stream_window }`
with positive durations and a nonzero retained-row budget per stream. Startup, reconnect and shutdown budgets use the host's clock.
`Attached::status()` watches connection state and failures; `activation()` captures
the current connection. An activation exposes its `description`, monotonic `number`,
`previous_started_at` and optional `executable_digest`. Compare the current and
previous `started_at` timestamps to distinguish a reconnect from a process restart.
Retained activations fail after disconnect and never follow the next connection.

Before admission the host verifies the catalog digest, resolves its declared
message names, and verifies operation fingerprints using the shared
[descriptor rule](../proto/harmos/v1/behaviour.md#schema-fingerprints). The Rust SDK,
reference fixture and host use `harmos-protocol` for this computation. Inconsistent
hashes or unresolved message names refuse admission before an activation becomes visible.

The host opens one Publish call for each catalog stream at admission. Interval streams
answer host-clock ticks; device streams publish without ticks. Each sample enters
the existing stream storage consumer path in arrival order. Incoming protobuf rows
are validated against their catalog type and converted to the host's canonical
stored row encoding with its derived schema. Interval rows carry their tick time;
device rows carry host arrival time in `published_unix_micros` and retain an optional
device timestamp as `context.source_time` (seconds and nanos). Disconnect and stop
close demand; reconnect opens fresh calls with sequence one. Lost samples are not replayed.

An `interval_setting` names a fully qualified protobuf section field, such as
`fixture.Settings.interval`, whose value is a positive `google.protobuf.Duration`.
The host resolves it from `Catalog.sections`; it takes precedence over the declared
interval. An absent or invalid setting refuses admission.

Plugins declare requirements for the Public `harmos.clock.sleep` and
`harmos.clock.now` operations. Both use the host clock through ordinary child calls.
Tests can configure `clock::Clock::virtual_at(epoch)` and drive its controller with
`advance_until(bound, condition)`. An invocation or unanswered tick keeps virtual time
still while work is in flight; only clock waits park that activity. Device streams
remain active until demand closes. Transport uses its normal scheduler.

`Attached::stop(reason).await` closes stream demand, calls Stop, closes the connection and, for a managed
plugin, waits for successful process exit within the same grace budget. Host
shutdown and dropping the attachment also request this cleanup. Remote endpoints
remain serving. Cleanup errors remain available in attachment status and its stop
result.

### Guest Hosting

Enable `harmos-runtime/plugin-guest` to host components and `harmos/wasm` in a
guest crate. The SDK has no runtime, storage or operations dependency.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
harmos = { git = "https://github.com/stokker-technologies/harmos", rev = "<tested-commit>", features = ["wasm"] }
```

```rust
use harmos::{Context, Operation, Plugin, Result, wasm::Registration};

#[unsafe(no_mangle)]
pub fn harmos_register() -> Registration {
    Registration::new(
        Plugin::new("example", "1.0.0")
            .operation(Operation::new("example.echo", async |_: &Context, request: String| -> Result<String> {
                Ok(request)
            })),
    )
    .capacity(2)
    .on_initialize(async |_: &Context, configuration: Vec<u8>| {
        serde_json::from_slice::<()>(&configuration).map_err(|error| harmos::Error::Invalid(error.to_string()))?;
        Ok(())
    })
}
```

Registration builds inert metadata. Componentization stamps its protobuf catalog;
loading validates that catalog, and activation verifies it against registration on
the retained store before initialization. Registration must not perform authored
resource initialization. Operation handlers use the native SDK's `Operation`,
`Context::call` and protobuf messages. Plugin operations are unary; declared streams
publish through `Context::publish` and the host stream manager.

The adapter retains Prepare, Initialize, Attempt, Cancel and Finalize phases.
`Context::submit` returns a child with `status`, `cancel`, `join`, `release` and
`detach`; dropping it releases owned work. Submit and control require the original
finite attempt capability. A retained generation context may call declared dependencies
or publish while its generation remains active, but cannot submit or control children.
The runtime owns loading, fuel, memory, deadlines and host authority. Consumers
set resource-scoped `PluginSettings::grants` on declared requirements; a guest
cannot grant itself access through its catalog. Stream-hosting
consumers supply `PluginGuestSettings::stream_window`. The SDK does not depend on `harmos-operations`. Development guest workspaces optimize
`harmos` and `harmos-protocol` at level 2 to keep descriptor validation within the
existing fuel budget; release builds optimize these normally.

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

Storage, Wasm guests and external gRPC commands have separate benchmark suites.
The guest suite enables the `benchmark` feature on the shipped Schematic guest.
Native gRPC plugins build without the retired framed-transport benchmark feature.
Each direction uses 256-record, 4096-sample or 1 MiB file batches. Guest operations
are unary. Generation and content verification are outside timing;
completion includes the receiver acknowledgment. Records, files and samples run
concurrently on one reused host/plugin per carrier, with isolated transfer IDs;
The guest fixture retains at most 64 MiB of data. File transfer
cases measure crossing the carrier boundary.

| Suite | What It Measures |
| --- | --- |
| [Storage Transforms](../crates/harmos-runtime/benches/storage/transforms.rs) | In-memory RMS, histogram and downsampling kernels over balanced three-phase samples. |
| [Guest](../crates/harmos-runtime/benches/guest/main.rs) | Real Wasm component: bidirectional unary protobuf transfer workloads, plus startup and command latency. |
| [External gRPC](../crates/harmos-runtime/benches/grpc/main.rs) | Separate server-process startup, client commands over a socket, and isolated Postcard/Protobuf codec costs. |
| [Streams](../crates/harmos-runtime/benches/streams/main.rs) | Concurrent waveform work and codec costs. |

Build matching executables and carrier fixtures with `mise run build -- --artifact bench`,
then run `mise run bench`.

One million samples represent one second at 1 MHz without pacing ingestion.
Published comparisons need the revision, hardware, payload, batching and carrier
alongside native Criterion results.

Native Criterion HTML and raw measurements live under `.stkr/benchmarks/criterion/`,
partitioned by compatibility identity and benchmark executable. The runner compares
against the compatible `dev` baseline and reports missing baselines or regressions
in its output. HTML rendering uses Plotters and does not require Gnuplot.

## Documentation

- [Repository Tooling](tooling.md) — workstation builds, shared compiler cache and `tools/config/` configuration.
- [Contributing](CONTRIBUTING.md) — development setup and validation.
- [Runnable Examples](examples.md) — application composition and plugin integration.
- [C++ Plugin SDK](tooling.md#c-plugin-sdk) — C++20 authoring with protobuf messages and gRPC plugin serving.
- [Schematic Walkthrough](schematic.md) — the example application, its crates and backends.
- [Public Rust API](../sdk/rust/src/lib.rs) — the standalone protocol SDK.
- [Cargo Features](../crates/harmos-runtime/Cargo.toml) — select the facilities your application needs.
- [Storage Interfaces](storage.md) — saved Arrow reads, indexes, projections, conditional objects and the optional SQLite backend.
- [Storage Adapter](../examples/schematic/storage/) — own your persistence format.


## Built-In Operation Ownership

`harmos-operations` holds native host, lifecycle, storage and stream contracts and
execution. `harmos-runtime` exposes them under `harmos_runtime::builtins`.
Guest lifecycle messages belong to `harmos-protocol`; the optional adapter and
component world live under `sdk/rust/src/wasm/` and `sdk/rust/wit/`.

The native invocation and resource engine lives in
`crates/harmos-operations/src/execution/`. Neither it nor the standalone `harmos`
SDK depends on `harmos-runtime`. Guests use SDK builders and protobuf contracts;
the runtime alone owns Wasmtime and sandbox authority.

## Transaction Reversibility

Keep `Transaction<S>` and `Invertible<S>` implementations beside their data.
Declare each entry as reversible or as a barrier explicitly:

```rust
use harmos_runtime::state::{StateCatalog, TransactionEntry};

let movement = TransactionEntry::<Schematic, MoveSymbol>::new("symbol.move")
    .reversible::<MoveSymbol>();
let placement = TransactionEntry::<Schematic, PlaceSymbol>::new("symbol.place")
    .barrier();
let changes = StateCatalog::new()
    .transaction(movement, Transactions::MoveSymbol, |change| match change {
        Transactions::MoveSymbol(value) => Some(value),
        _ => None,
    })
    .transaction(placement, Transactions::PlaceSymbol, |change| match change {
        Transactions::PlaceSymbol(value) => Some(value),
        _ => None,
    })
    .build()?;
setup.settings().register(changes);
```

Only listed variants are admitted. The complete Schematic registry is
[`transactions_state_catalog`](../examples/schematic/app/src/transactions/mod.rs).
A data union may implement ordinary `From`, `Transaction` and `CatalogCodec`
dispatch; catalog membership and restoration belong to the value registry.
Entry constants such as `MoveSymbol::ENTRY` are reusable values, not membership proofs.

`StateCatalog::build()` and runtime startup reject duplicate identities or typed
entries, undeclared reversibility, missing inverse entries and non-mutual pairs.
`reversible::<I>()` requires the existing `Invertible<S, Inverse = I>` body.
A self-inverse names its own type. Barriers use `.barrier()` even when another
transaction in the same catalog is reversible.

Preparation retains the accepted payload with its stable identity and
schema-derived revision. Replay restores that payload and applies it without
running admission checks again. Undo computes the inverse against the pre-state
and admits it through the existing check/apply path. Snapshot consumers receive
their restoration catalog by value; see the
[snapshot adapter](../examples/schematic/storage/src/snapshot.rs).

## Plugin Source Layout

A native plugin crate depends on `harmos`; its executable assembles builder values
and calls `serve()`. Keep protobuf messages and operation contracts beside their
handlers. A reusable contract crate can implement `Message` for generated types
or use its derive. `use harmos::prelude::*;` supplies the authoring surface.

Guests live in separate `-guest` crates and depend on `harmos` with `wasm`. Shared
operation implementations can live in a protocol-only crate;
shared models remain in a domain crate. Host applications depend on
`harmos-runtime` and own state, storage adapters and placement settings.
Plugin artifact crates declare `publish = false`.

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

## Saved Storage Queries

The built-in storage catalog exposes bounded Arrow scans, ordered indexes, exact-frontier
projections and atomic conditional object updates through generated SDK contracts. Compose the
backend explicitly and retain/release saved cuts; see [Generated Storage Clients](storage.md#generated-storage-clients)
for requests, grants, outcomes and the Schematic query-storage journey. The ordinary query,
streamed write receipts, files and recovery interfaces remain available.
