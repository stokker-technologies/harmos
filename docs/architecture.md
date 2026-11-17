# Architecture

Transfers remain unary, amended by the owner on 2026-11-13. The generated
`StorageTransferSettings.chunk_bytes` under `StorageSettings.transfer` owns the
single 4 MiB default for native append and ranged-read sizes and product wrappers. Generated
`MessageSettings.message_bytes` declares the separate 8 MiB message default once.
Client SDKs, listeners, plugin carriers and Wasm guests use it and refuse ceilings
that cannot carry the configured chunk plus framing at startup, naming the action and
setting. Streaming transfer handlers, proto requests, bindings, SDK surfaces,
stories and benchmark cases are deleted; Watch and native readers remain.

Application-owned immutable content registers through StorageManager's single
`ContentSource`. Resolution receives the inherited caller scope and returns length,
SHA-256 and a bounded lease with explicit expiry. Native GetFileBytes and product wrappers retain
that lease in the existing bounded read-cut registry, bound to the reference and
scope. Expiry refuses rather than renewing a continuation. Native hosts use
`open_content(&reference, scope, at)` and `ContentRead::get_bytes(offset, maximum)`.
Library-history references keep their saved-storage path; no application bytes or
metadata enter library persistence. Diagnostics and bundle installation use the
same content boundary. See [Application-Owned Content](storage.md#application-owned-content).

## Plugin Boundary

**Locked owner decision, 2026-11-16; not implemented:** several products embed
Harmos. Harmos owns plugin runtime machinery; a product owns policy and everything
an author touches. [Plugin Boundary](../openspec/changes/plugin-boundary/design.md)
amends `operation-boundary` and `plugin-lifecycle` together.

| Harmos | A Product |
| --- | --- |
| Internal plugin manager; register → admission → initialize → finalize; operation routing | Plugin contract, four author SDKs, registrars and starters |
| Descriptor closure, identities, fingerprints and declaration digests | Admission callback on its registered plugin kind; settings resolution |
| Process/Wasm hosting, transport, reconnect/fencing, sampling/windows, deadlines/cancellation | Registry, startup selection, installation, persisted settings and management operations/projections |
| Minimal carrier, Rust/Python/C#/C++ carrier helpers, shared descriptor/codegen tooling, bounded inspection, generic conformance | Bundle format/build/verification, manifests/artifact digests, distribution, product conformance and one self-contained contracts archive per release |

The carrier contains connection identity/revision, admission descriptors, invocation
correlation with opaque payloads, cancellation, host-call frames, window/sample
framing and opaque product sections. Product SDKs hide Harmos types from authors
without import aliases. Carrier helpers retain reusable cancellation and windows.
Consolidate the consuming product's richer generators and fixes here; replace its
spawn/connect/Get/Stop probe with bounded inspection. Move bundle code to the
product and delete the duplicate builder. Preserve atomicity, replay receipts and
explicit incompatible-root refusal; no migration or silent reset.

Proposed API through the typed facade; these are not landed symbols:

```rust
setup.plugin_kind("product.grpc",
    grpc::Kind::<ProductContract>::new().admit(product::admit));
let host = runtime.for_call(&ctx);
let plugin = host.plugins().load(Load {
    kind, placement: Managed { command }, settings,
}).await?;
```

The kind supplies admission and settings-resolution hooks. Generic validation
precedes product admission; resolved settings precede initialize. Products decide
what and when to load; their registry calls load at startup. Bind and start-samples
return opaque generation-fenced handles. No `PluginBackend` trait. Installation
policy never becomes a second activation owner. Regenerate once with the operation
and lifecycle changes; generated output stays tracked pending the separate decision.

## Typed Native Calls

`Runtime::for_call` and `Handle::for_call` accept a `CallContext` or a trusted
`HostContext`. `StorageRuntime::for_call` offers the same storage calls before full
startup. The facade's `storage()` group covers the native storage catalog;
`streams().publish()` submits one native publication. Request and result DTOs live
under `harmos_runtime::facade::{storage, streams}`. Watch methods take typed output
endpoints and return cancellable invocation handles.

A `CallOptions` value on the call context replaces the registered operation policy
for that invocation. Binding removes it from the admitted context: descendants
use their own policies, while caller metadata still propagates. A scoped binding
retains parent cancellation and lane ancestry, and refuses a parent belonging to
another runtime. The facade has no scheduler or retry policy of its own. Native
payloads retain their allocation and are never encoded for dispatch.

The runtime's whole-crate `domain` and `builtins` reexports are removed. Builder
catalogs have explicit exports under `harmos_runtime::catalog`. Backend-authoring
contracts and their value closure have explicit exports under
`harmos_runtime::backend`, without managers or readers. Storage-only sessions expose
`for_call` and cleanup, with no operation-manager getter. Builder ownership, the
remaining manager privacy, facade groups and private host transport are still
governed by the pending Operation Boundary inventory below.

## Storage and Query Ownership

**Locked direction, 2026-11-14:** the dependency order is
`harmos-domain <- harmos-query <- harmos-storage <- harmos-operations <- harmos-runtime`.
The domain contracts, query engine and storage layers are implemented in their
named crates. `harmos-storage::memory::{Backend, RecoveryBackend}` provides the
public process-lifetime implementation. `StorageBackends::in_memory` composes it;
the runtime builder supplies the runtime clock (locked facade direction). Consumers still supply their budgets.

**Runtime composition, locked direction (2026-11-16):** storage remains
always present and the builder owns backend/catalog/settings admission and every
manager lifecycle. Products persist plugin entries, settings and bundle content
through storage under Plugin Boundary. Products do not construct or register a
manager. A storage-only assembly admits prerequisite writes and offline maintenance
without listeners or plugins; implementation is tracked in `operation-boundary`.

Compound mutation now applies ordered conditional object writes, deletion and record
appends in one backend commit, publishing a cut and replayable receipt together.
`harmos-storage/src/compound.rs` owns admission; `memory/compound.rs` stages bounded
changes and publishes through the backend hook; `storage.mutate` is the native declaration.
Unsupported backends refuse; sequential compensation is not an implementation.
The invariant “Object/scoped deletion preserves retained cuts and delays file
reclamation until those cuts release” preserves evidence while readers use it.
`backend.rs` now separates durable snapshot ownership from session reader leases.
Native `RetainSnapshot` owns exactly one opaque scope; backend scope deletion
releases its owners atomically, while independent readers keep their original
images until release. A retained scope never pins another scope's deleted data.
Native handles reopen through the backend after manager/registry restart; durable
products persist their handles and ownership across process restart. The reference
memory backend retains them across sessions within its process lifetime.

Durable followers scan committed backend data, including compound, conditional and
columnar sample writes. Backend observation metadata rounds resume positions down to complete
mutation frontiers and rejects crossed deletions with a resync gap. Bounded row
pages carry the previous complete cursor until progress publishes the complete
frontier; a resume can replay positions but cannot skip a mutation suffix. Global
projection publication requires a complete global snapshot. Session reopening
invalidates transient reader tokens while retained owners remain reopenable.

Statistics kernels live in the crate-internal
`harmos-query::statistics` module: exact reductions and rank spools, scaled population
moments, histograms, Tukey box summaries, ordered mergeable summaries and adaptive
bucket summaries. Arrow transform modules own schema validation and row plumbing;
the grouped time-bucket projection and durable-lag benchmark delegate to the same
kernels. Products request these through `storage.list_rows` transforms and persist
bucket continuation bytes themselves. Continuation retains the source history/cut,
requires strictly increasing new source positions and doubles aligned bucket widths
until the consumer's bucket cap holds. Floating-point RMS and population standard
deviation use scaled moments; no approximate percentile replaces exact rank selection.
Several percentiles and a floating median of one field share one numeric spool and
one multi-rank selection; an exact rational median retains exact scalar ordering.

`AppendSamples` admits one immutable Arrow batch through
`StorageBackend::append_sample_columns(&writes::SampleAppend)`. Admission validates
columns, reserves contiguous positions in Arrow row order, and fingerprints the
whole IPC unit; it never constructs sample row payloads. One session-local cache
entry reuses the last resolved immutable sample schema so repeated windows do not
open read cuts over growing history. A different schema resolves normally. Providers advertise
`columnar_append` and atomically commit the complete batch and mutation receipt.
An accepted position alone is insufficient for acknowledgement: the native receipt
has `completed=true` and `durable_at` only after that commit. Identical replay
preserves the original accepted frontier, including after backend reopen.
The memory reference retains Arrow batches and materializes row payloads on reads;
its durability lasts only as long as its backend instance. Existing typed row
callers retain their row API; Arrow ingestion has no row-conversion fallback.

### Inventory Before the Split

This is the required pre-move inventory. Every path is relative to `crates/harmos-storage/src/`.
The categories describe responsibility, not the current module visibility.

| Responsibility | Current Modules |
| --- | --- |
| Interface | `backend.rs`, `batch.rs`, `config.rs`, `content.rs`, `definitions.rs`, `error.rs`, `file_batch.rs`, `file_checkpoint.rs`, `persistence_work.rs`, `read_batch.rs`, `receipts.rs`, `record_view.rs`, `schema_sets.rs`, `writes.rs` |
| Recovery Interface | `state/mod.rs`, `state/config.rs`, `state/reader.rs`, `state/segments.rs`; the snapshot payload is immutable memory-backed data |
| Manager Composition | `lib.rs`, `manager/**`, `local.rs`, `worker.rs` |
| Manager Admission and Persistence | `admissions.rs`, `batching.rs`, `capacity.rs`, `deletion.rs`, `object_admissions.rs`, `objects.rs`, `queue.rs`, `state/admission.rs` |
| Manager Files and Follows | `directory/**`, `file_capture.rs`, `file_follow.rs`, `file_hash.rs`, `file_production.rs`, `file_reads.rs`, `follow.rs`, `frozen_file.rs`, `opened_capture.rs`, `staging/**` |
| Manager Encoding and Execution | `execution.rs`, `execution/**`, `schema_encoding.rs`, `selection.rs`, `snapshot.rs`, `writes/**` |
| Manager Query Integration | `query/manager.rs`; snapshot retention, admission, schema resolution, source conversion and backend dispatch currently embedded in `query/{reader,typed,object_schemas,columnar,index,projection}.rs` |
| Engine | `aggregate.rs`, `arrow.rs`, `arrow/**`, `statistics/**`, `query.rs`, `query/{accumulate,execute,order_stats,ordering,quantiles,reduction,source,spill,table,time_buckets}.rs`, their descendant modules and `query/transform/**`; engine parts of the six mixed modules above |
| Backend | `memory/**`; primary data and recovery remain process-lifetime state |
| Benchmark Fixtures | `bench_support/**`; migrate workloads to the in-memory adapter |

The in-memory implementation previously lived in `tests/support/{storage,read_backend,
file_backend,record_store}.rs`. Its data, recovery and deterministic control state now live in `src/memory/`;
fixture settings remain under test support. The snapshot payload is immutable
`Arc<[u8]>` with the existing segmented digest checks, without a temporary file.

### Query Import Inventory

The original 27 `use crate::storage...` declarations are listed below. Line numbers
refer to the tree before the split. Self-imports are included so the count is
reproducible; they are not all dependencies on manager state.

| Query File | Lines and Imported Responsibility |
| --- | --- |
| `reader.rs` | 34 Arrow wrappers; 35 scan page; 36 scan policy; 37 manager `Inner`; 38 selection `Entry`; 39 retained `Snapshot`; 40 manager and stored values |
| `typed.rs` | 29 scan page; 30 manager `Inner`; 31 selection `Entry`; 32 retained `Snapshot`; 33 manager and stored values |
| `object_schemas.rs` | 18 project execution; 29 scan page; 30 scan policy; 31 manager `Inner`; 32 retained `Snapshot`; 33 object/schema values |
| `source.rs` | 37 file/object/record/stream/schema values |
| `transform/aggregate.rs` | 20 field execution; 30 accumulator |
| `transform/filter.rs` | 28 field execution |
| `transform/downsample.rs` | 30 field execution |
| `transform/histogram.rs` | 20 field execution |
| `transform/project.rs` | 18 field execution |
| `transform/sort.rs` | 18 field execution; 27 scalar ordering |

Other direct imports also need migration: `columnar.rs:38-39` imports schema,
backend, snapshot, page and work-limit types; `index.rs:27` and `projection.rs:25`
import backend/snapshot/work-limit contracts; `manager.rs:26` imports the manager.
`reader.rs` and `typed.rs` additionally name execution context and response settings
in fields. `accumulate.rs` and `order_stats.rs` import accumulator settings;
`accumulate.rs` imports aggregate/statistics implementations. Shared domain values
move to direct domain imports, engine self-imports become local imports, and the
engine gets its own errors and work limits. None of those require a storage edge.

### Manager and Backend Calls Into Query

| Caller | Current Calls or State |
| --- | --- |
| `manager/arrow.rs` | `query::schema_at` resolves the admitted schema at a cut |
| `snapshot.rs` | `columnar::list_column_rows`, `index::list_index_entries`, `projection::advance_projection` |
| `query/manager.rs` | index/projection registration, definition/status lookup, publication reads and retention delegate to the primary backend |
| `query/reader.rs` | `NormalizedObjects::resolve`, `source::{records,stream,projected,objects,files,schemas}`, project execution and `Plan::{new,page,finish,source_limit,is_inline,is_satisfied,output_schema}` |
| `query/typed.rs` | schema lookup and typed decoding coupled to `Entry`, admission and retained snapshots |
| `query/columnar.rs` | native capability dispatch, stored-row fallback, flattening, filtering, projection and page continuation validation |
| `query/index.rs` | native capability dispatch, fallback columnar scans, index key encoding and bounded candidate sorting; `IndexCatalog` owns definition revisions |
| `query/projection.rs` | native advancement dispatch, definition/status lookup, columnar scans, bucket accumulator and fingerprints, previous-output reads and atomic publication; `ProjectionCatalog` owns revisions and retained outputs |
| `tests/support/storage.rs` | `IndexCatalog` and `ProjectionCatalog` registration/read/publication; promote these catalogs with the in-memory backend, not into the engine |

The catalog and publication orchestration belongs in storage. Its public engine
calls operate on batches, transforms, bucket accumulators and fingerprints.
No engine API exposes `Sample` or storage catalog state.

### Batch Source at a Cut

The implemented seam is in `harmos-query/src/batches.rs`. `Source`,
`StorageCursor`, filters and projections are domain contracts. `BatchScan`,
`BatchPage`, `QueryFuture` and the two traits belong to `harmos-query`.

```rust
pub trait BatchSource: Send + Sync {
    fn open<'a>(
        &'a self, selection: &'a Source, at: &'a StorageCursor,
        scan: &'a BatchScan,
    ) -> QueryFuture<'a, Box<dyn BatchReader>>;
}

pub trait BatchReader: Send {
    fn schema(&self) -> SchemaRef;
    fn at(&self) -> &StorageCursor;
    fn next(&mut self) -> QueryFuture<'_, BatchPage>;
}
```

`BatchScan` carries bounded row/byte work limits, an optional payload projection
and a pruning filter; selection carries the time bounds. Opening fixes the cut and output schema. Source
implementations must honor selection and cut, may prune false rows, and must
produce exactly the requested projected schema. Residual engine filters preserve
correctness when pruning is declined. A page carries a batch and an explicit
completion bit: an empty pruned page can make progress without completing.
Dropping the reader releases its cut. The source owns seek positions and snapshot
tokens; neither is interpreted by the engine.

The storage implementation owns snapshot validation/retention, active query
admission, object schema normalization, bounded backend calls and row-to-batch
conversion. Manager readers own admission while a page is active; operation pagination releases
it between pages. Standalone batch sources acquire admission while opening and
reading each page, so a two-input engine reader also works with one active slot.
Cancellation releases owned work.
A backend with native batches can implement the same source, including projection
and pruning, without exposing its persistence model. Engine readers own plans,
transforms, reductions, ordering, spill and typed batch decoding. The manager calls
their public constructors and transforms; the engine never imports manager,
session, receipt, staging, executor or recovery types. This admits the crate split.

### Implemented Crate Boundaries

`harmos-domain` owns selections, cursors, `Query`, `TimeMerge`, shared read limits
and payload schema contracts. `harmos-query` owns Arrow wrappers, the batch-source
traits, execution plans, transforms, reductions, statistics, ordering, spill,
typed readers, columnar execution helpers and index key encoding. It has no
storage dependency. Storage owns admission, snapshots, source decoding and
backend calls, index/projection catalogs and atomic publication. The disk backend and file recovery journal are removed. Arrow in storage is used
for admitted-batch conversion and manager query integration.

Products implement `StorageBackend` and `StateBackend` through the facade's
authoring contracts and pass the backend pair, catalog and storage settings to the
builder. The in-memory adapters retain cuts, receipts, files and recovery only for
their shared-instance lifetime; a product backend owns process durability. Primary
compound transactions do not promise a cross-owner transaction with state recovery.
See [Storage Interfaces](storage.md#backend-registration). The builder migration
remains proposed; the source inventory records the existing constructors to remove.

### Unconditional Arrow Dependencies

A normal `harmos-runtime` dependency tree, including `--no-default-features`, still
contains these Arrow 60.1.0 crates. Disabling gRPC does not disable storage or query.

| Crate | Code That Retains It |
| --- | --- |
| `arrow-array` | Query batches/transforms/merge; storage `manager/arrow.rs`, `writes/convert.rs`, `query/{batches,object_schemas,projection}.rs` |
| `arrow-buffer` | Query `source/columns.rs` builds nullable/list arrays; transitive array/data/select buffers |
| `arrow-data` | Query `arrow/retention.rs` accounts array allocations; storage `query/object_schemas.rs` rebuilds normalized arrays |
| `arrow-schema` | Query schemas and validation; storage batch-source schemas and object normalization |
| `arrow-ipc` | Query `arrow/ipc.rs` validates/decodes admitted IPC and `spill.rs` serializes spill tables; storage `writes/decoded.rs` calls that wrapper |
| `arrow-select` | Query take/concat/merge execution; storage `query/object_schemas.rs` concatenates normalized batches |
| `arrow-cmp` | Transitive dependency of `arrow-select`, not directly imported by this repository |

There are no production Arrow imports in runtime or native operations themselves.
Their unconditional `harmos-query` edges retain engine errors and query results;
native storage operations also expose engine-owned columnar/index contracts.
The plugin manager needs conditional objects and content, not query execution.

### One Ordered Time Merge

`Query.merge: Option<TimeMerge>` names one secondary stream at the query's exact
cut. Both inputs must be nondecreasing by their named Int64 or nanosecond timestamp
fields. The engine refuses descending input. Output nests complete rows under
`left` and `right`; ordinary transforms then project, group or aggregate it.
`TimeMerge.work` bounds retained rows and Arrow bytes; output follows the ordinary
page limits. The same sweep operator implements all four uses:

```rust,ignore
let merge = |mode| TimeMerge {
    source: samples.clone(),
    primary_time: FieldPath::from("payload.time"),
    secondary_time: FieldPath::from("payload.time"),
    mode,
    work,
};
// Record [start, end) windows, including overlapping windows.
query.source = records.clone();
query.merge = Some(merge(TimeMergeMode::Range { end: FieldPath::from("payload.end") }));
// Latest stream value at or before each record; absent values are null.
query.merge = Some(merge(TimeMergeMode::AtOrBefore));
// Rows in [record time, next record time), then existing grouped aggregates.
query.merge = Some(merge(TimeMergeMode::Intervals));
query.transforms = vec![grouped_aggregate];
// The same at-or-before operation with a stream as the primary input.
query.source = another_stream;
query.merge = Some(merge(TimeMergeMode::AtOrBefore));
```

At-or-before ties select the last secondary row at that time. Interval ties select
the last primary record; rows before the first record are omitted and the final
interval extends to the end of the stream. Range output follows secondary time
then primary record order. This is an ordered temporal operation, not a general
join. In-process `ListRowsRequest.merge` uses this domain shape without a wire
change. Four adapter stories live in `tests/integration/time_merge.rs`; the query
crate's `integration-time-merge` bench measures each use over 1,000,000 rows.

## How to Use This Document

Python client resources are committed generated classes derived from protocol
descriptors by `tools/python/src/harmos_tools/build/resource_roots.py`. Each RPC
has explicit request, response and per-call option types; server streams retain
typed async iteration and native cancellation. `sdk/python/src/harmos/client.py`
initializes the library root and owns connections created by `Client.connect`.
Products generate and compose their own typed resources. No runtime attribute
binding or serialized resource table participates in any SDK's client naming.
See the [Python SDK Guide](python-sdk.md).

Read this contract before proposing architecture, protocol, public API or ownership
changes. A proposal touching an invariant must quote it, explain the purpose of the
mechanism it changes, and update this document in the same delivery. Never remove,
merge or replace a mechanism until its purpose is established from its handler,
tests and design, or from the owner.

**landed** means verified in this tree, not newly executed proof. **locked direction**
names an agreed change whose complete contract has not landed. Partial proofs below
do not prove a future contract. All missing guards have unchecked tasks in
`openspec/changes/operation-grammar/tasks.md`; its first task covers visibility and
both directions of plugin/host invocation. A face is never a substitute for visibility.

Paging was amended by the owner on 2026-11-13: only data rows that can grow
without bound are paged. Operations, Schemas, Streams, Plugins and Access.Pairings
return the complete set without page requests, tokens or application byte bounds.
The carrier message ceiling is their sole response guard; overflow names the action.
Records, samples, log lines, trace events, files and rows retain position-resolved paging.

## Protocol Source Convention

Use the selected proto skeleton, preserving Imports, Options, Services, Resources
and Enums even when empty. A resource owns one file, its service and its wrappers.
After Services, replace the generic Requests and Responses section with one
`// === Name === //` section per operation in service order; each contains the
request directly before the response. Nested resources live in subfolders.
Every service, call and message has a contract comment. Describe optional selectors,
opaque revisions/cuts, refusal and replay from their owning contracts. Buf owns
canonical layout, field-per-line formatting and comment lint. Proto2 transfer
presence/defaults remain unchanged.

```proto
syntax = "proto3";
package harmos.host.v1;
// === Imports === //
import "harmos/model/v1/callers.proto";
// === Options === //
// === Services === //
// Example resource; each operation owns its caller declaration.
service Resource {
  // Read the resource.
  rpc Get(ResourceGetRequest) returns (ResourceGetResponse) {
    option (harmos.model.v1.callers) = ALL;
  }
}
// === Get === //
// Selects the resource.
message ResourceGetRequest {}
// Returns the resource.
message ResourceGetResponse {}
// === Resources === //
// === Enums === //
```

The independent library and example Buf modules run through `mise run format` and
`mise run check -- --repository`; the latter also enforces mandatory caller options.
Regenerate with `mise run generate -- --sdk rust`, then python, csharp and cpp.
Generation owns every output and removes obsolete generated modules. The shared
Rust generator lives in `crates/harmos-protocol/codegen/`; the generation example
and integration target resolve it through ordinary module declarations. Its
reflection renderer serves both library and product-fixture packages. Generated
child packages also use ordinary module declarations, following the proto tree.

`tools/python/src/harmos_tools/build/resource_roots.py` owns descriptor composition
and generation order. Its `build/resources/descriptor.py` owns the shared method
inventory; `python.py`, `rust.py`, `csharp.py` and `cpp.py` each own one language's
rendering. `source/protocol.py` validates declared caller options from a
single dependency-resolved descriptor pool, resolving the shared option schema
once for the inventory. Call rendering and payload exports consume the same
method inventory so audience ownership has one source. Descriptor artifacts are the
composition inputs; no caller override map is read. Generated bindings
remain tracked and regeneration is the only authoring path. The protocol
drift test compares committed sources and descriptor projections byte for byte.
`build/resources/sdk.py` owns complete SDK generation and drift comparison;
`build/resources/configuration.py` carries namespace, original library descriptors,
selector policy and compiler location. Products pin the `harmos-tools` package and
retain their invocation inputs rather than copying these renderers. The shared
C# wire projector lives in `build/csharp_protocol.py`; the native C# task selects
its pinned compiler and delegates to it. The descriptor inventory retains map,
oneof, explicit presence, copy and nested enum metadata. Public payload ownership
uses the supplied library descriptor inventory and each declared audience.
Generated output remains tracked. See [Repository Tooling](tooling.md)
for the complete-SDK invocation.
See the mapping below for the migrated identities and consumer ownership.

## Invariants

The `operation-boundary` contracts above supersede the wire composition and host-client
root direction in invariants 1, 9 and 17. Those entries describe the preceding migration's
landed source until the breaking groups replace it; new product consumers follow the
native facade and product-wrapper direction. Manager ownership remains internal.

Wire operation admission uses the protocol-owned validator in
`crates/harmos-protocol/src/schema/mod.rs`, independently of native dotted identities.
The [wire identity contract](../openspec/changes/operation-grammar/design.md#wire-operation-identity-admission)
covers numeric template bindings and application-owned face packages. Declaration admission,
guest admission and host protobuf declarations share it; visibility, descriptor fingerprints
and caller-face authorization remain independent checks.

1. **Executor Packages and Declared Callers.** Model has no operations. Host and
   plugin packages identify where operations run; the method option declares who
   may call each one. Shared operations have one identity and wrapper pair.
   The host mediates cross-plugin calls; Private operations stay in-process.
   **Why:** avoid resource twins while preserving caller-specific admission.
   **Where:** proto/harmos, the mapping and work lists below.
   **Proof:** Buf plus caller-option checks and generator audience/projection tests
   cover definitions. Runtime denial cases 33/34 remain with their consumer.
   **Status:** definitions/generated outputs landed; runtime and SDK consumers pending.

2. **Admission Boundaries.** The host refuses catalogued Private declarations and
   unspecified visibility. Protected admits the declaring owner and host; Public
   admits declared peers and authorized clients. Visibility is checked before
   provider selection branches, including host-provided built-ins.
   **Why:** matching requirements and fingerprints do not grant host visibility.
   **Where:** `crates/harmos-runtime/src/operations/manager/plugin_grpc.rs`,
   `crates/harmos-runtime/src/operations/manager/entry.rs` and
   `crates/harmos-runtime/src/plugin/grpc/catalog.rs`.
   **Proof:** `protected_host_operation_refuses_a_plugin_with_a_matching_requirement`
   and `protected_peer_operation_is_refused_across_plugins` are the focused guards.
   **Status:** landed with focused native proof.

3. **Three-Level Visibility.** Private is callable only inside the declaring plugin
   process, absent from its declarations, invisible to the host, never on any wire
   (including the host/plugin carrier), without a gRPC method; the SDK runs it
   locally with ordinary policy, retries, deadlines and cancellation. Protected is
   catalogued and callable by the plugin and host, with host calls using only that
   plugin's carrier, never a client-facing gRPC method; other plugins and clients
   are refused; Public admits other plugins by declared requirement and clients
   through the host and may bind a client-facing gRPC method; every SDK requires
   an explicit choice with no default, and host built-ins follow the decided mapping.
   **Why:** the owner's intent is "a private operation never appears on the wire, is consumed only internally by the plugin and does not allow association to a grpc endpoint" and "a plugin should be able to call another operation in another plugin if exposed".
   **Where:** Explicit Three-Level Operation Visibility in
   `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`;
   Decided Built-In Visibility Mapping in `openspec/changes/operation-grammar/design.md`.
   Host-only requests of a plugin must be Protected; Private dispatch stays inside
   the declaring SDK process.
   **Proof:** common conformance case 24 guards local dispatch, declaration omission
   and direct carrier refusal in all four SDKs. Rust uses its local operation map;
   Python, C# and C++ use their registered typed handlers without a child frame.
   Cases 26–28 additionally guard host-clock retry, attempt deadlines, child cleanup,
   isolated attempt resources and reason-bearing cooperative cancellation in all four SDKs.
   Native reserved controls are Protected; the complete storage mapping belongs to
   its declaration owner.
   **Status:** native SDK dispatch and policy implemented; combined built-in census open.

4. **Host-Mediated Public Calls.** A plugin calls a Public peer as plugin → host →
   plugin: the host resolves one registry, requires the declared requirement and
   matching fingerprint and an admitted provider, and carries inherited scope,
   failures and cancellation through the child invocation.
   **Why:** exposure must enable peer composition without bypassing host authority.
   **Where:** `crates/harmos-runtime/src/operations/manager/plugin_grpc.rs`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`; requirement: Host-Mediated
   Plugin Calls in `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** `child_calls_reach_host_and_public_peer_without_start_order` in
   `crates/harmos-runtime/tests/plugin_grpc/child_calls.rs` proves host and peer
   routing plus mismatched/absent providers; scope proof is invariant 7.
   `cancellation_reaches_the_plugin_before_settlement` in
   `crates/harmos-runtime/tests/plugin_grpc/cancellation.rs` proves cooperative
   settlement. `public_peer_inherits_scope_cause_and_caller_cancellation_through_the_host`
   combines peer scope, structured causes and cooperative child settlement;
   common conformance cases 24 and 29 guard local-only Private isolation and peer visibility.
   The client invocation story in invariant 1 covers Public and Protected client calls.
   **Status:** landed for native gRPC plugins and all four SDKs.

5. **Unary Plugin Operations.** A plugin operation has one request and one final
   response per attempt, even though its carrier multiplexes control/child frames;
   samples and observation are separate streams.
   **Why:** transport duplexing must not silently change the operation contract.
   **Where:** `proto/harmos/plugin/v1/instance.proto`, `proto/harmos/plugin/v1/behaviour.md`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`; requirement: Unary Plugin
   Attempts in `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** `host_invokes_registered_plugin_operation` in
   `crates/harmos-runtime/tests/plugin_grpc/host_invoke.rs` checks the request/result
   journey; `malformed_payload_fails_only_its_invocation` in
   `crates/harmos-runtime/tests/plugin_grpc/malformed_payload.rs` guards
   malformed unary input.
   **Status:** landed.

6. **Explicit Streaming and Cooperative Control.** Never generate an automatic
   streaming twin beside a method; streams serve samples and Watch or protocol
   carriers. A specifically promised typed final result uses one replacement
   managed method; Cancel interrupts running work, while Stop ends at a safe boundary.
   **Why:** preserve typed cooperative settlement without multiplying interfaces or
   confusing observation loss with work ownership; queued control refusals stay explicit.
   **Where:** No Automatic Streaming Twins and Distinct Stop and Cancel Actions in
   `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** the face descriptor guard excludes generated twins; the discovered-client
   journey in `crates/harmos-runtime/tests/operations/wire/sdk_client.rs` exercises
   explicit managed replacement and cooperative final results. The product-control
   story preserves a queued Cancel refusal and its cause, then awaits a separate Stop
   boundary without issuing Cancel. Peer and local-policy stories guard child settlement.
   The final storage stream census awaits its declaration owner.
   **Status:** landed for non-storage projection/control; combined inventory open.

7. **Opaque Invocation Scope.** Carry caller-bound scope unchanged through children;
   never interpret it as authorization or require it for operations that do not ask.
   **Why:** applications choose its meaning; a correlation/selection value is not a grant.
   **Where:** `crates/harmos-runtime/src/operations/scope.rs`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`, `sdk/rust/src/context.rs`;
   Opaque Scope and Canonical Identity in
   `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   **Proof:** `bound_scope_reaches_handlers_and_children_and_missing_scope_is_refused`
   in `crates/harmos-runtime/tests/plugin_grpc/scope.rs`; `external_request_grants`
   in `crates/harmos-runtime/tests/plugin_grpc/external_authorization.rs` separately
   proves request-grant enforcement.
   **Status:** landed.

8. **Manager Ownership.** A manager exists when an owner coordinates live resources
   and lifecycle; plain storage operations need none. Service methods delegate
   orchestration to that owner, never a second registry, scheduler or upload engine.
   **Why:** one authority must own creation, cancellation, retirement and partial failure.
   **Where:** Boundary and Manager Ownership in
   `openspec/changes/operation-grammar/design.md`; Manager Ownership and Thin Service
   Methods in `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   OperationManager owns operation discovery and invocations; StreamManager owns
   publication/demand; StorageManager owns persistence/production; GrpcManager owns
   listeners/access; ServiceManager owns resident services/status and host clock;
   StateManager owns state/replay; RecordManager owns record registration/lifecycle;
   the internal plugin manager owns live activation, placement execution, routing
   and settlement. Products own installed entries, persisted settings and management
   policy (locked amendment, 2026-11-16, Plugin Boundary).
   Existing owners live in `crates/harmos-runtime/src/operations/manager/mod.rs`,
   `crates/harmos-runtime/src/streams/manager.rs`,
   `crates/harmos-runtime/src/storage/manager.rs`,
   `crates/harmos-runtime/src/grpc/manager.rs`, `crates/harmos-runtime/src/services/mod.rs`,
   `crates/harmos-runtime/src/state/manager.rs`, `crates/harmos-runtime/src/records/manager.rs`
   and `crates/harmos-runtime/src/plugin/manager/mod.rs`.
   **Proof:** `client_manages_remote_plugins`, `client_manages_bundled_plugins`,
   `host_restores_plugin_entries` and `client_refuses_invalid_plugin_bundles` guard
   persistent entries and activation ownership. `client_pairs_and_rotates_access`
   and `credential_store_unavailable` guard listener/credential atomicity.
   `client_reads_status`, `client_pages_and_watches_events` and
   `client_reads_diagnostics_ranges` guard contributions, histories and archives.
   **Status:** existing owners landed; the plugin runtime/policy split is locked
   direction. The cited entry tests describe behavior to transfer, not proof of it.

9. **Library Runtime and Product Policy.** Harmos owns hosting, access, status,
   invocations, time and live plugin machinery. Products own plugin contracts,
   registry, installation, bundles, distribution, persisted settings, admission
   policy and management operations/projections, plus schemas, indexes/projections,
   objects, authorization, secrets, UI, archives and credential integration.
   Host operations become in-process only under `operation-boundary`; products
   declare wrappers for generic operations their clients need. Plugin management
   is product policy, not a wrapper around library installation services.
   **Why:** one runtime owner and one policy owner preserve atomic behavior.
   **Where:** Plugin Boundary above and the two amended boundary/lifecycle changes.
   **Proof:** settings, replacement and trusted caller journeys under
   `crates/harmos-runtime/tests/plugin_grpc/manager`; application upload and discovery
   under `examples/schematic/app/tests/storage`.
   **Status:** locked direction, amended by the owner on 2026-11-16; the cited
   stories prove current behavior to preserve during the ownership transfer.

10. **Resource Operation Grammar.** Services name resources, methods use the closed
    ordinary verb set Get/List/Watch/Set/Create/Delete/Append; only declared domain
    actions are exceptions. Nested resources use parent sub-packages; wrappers are
    `<Service><Method>Request` and `<Service><Method>Response`; Set replaces the whole
    writable value under its expected revision, while Append adds ordered immutable data.
    **Why:** predictable generated clients must preserve conflict and immutability
    semantics; mixed-resource Storage retains distinguishing resource nouns.
    **Where:** Predictable Public Method Grammar, Nested Wire Packages and Wrappers
    in `openspec/changes/operation-grammar/specs/operation-contract/spec.md`;
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** client-root fixtures in all four SDKs compose library Plugins and
    plugins.Settings with product Jobs and jobs.Progress, preserving unary and Watch
    cardinality and qualified message identity. The complete storage mutation surface
    remains with its declaration owner; invariant 13 guards existing atomic behavior.
    **Status:** locked direction (`operation-grammar`).

11. **Position-Resolved Reads.** One read path takes optional `at`: absent selects
    latest durable data without requesting a flush or waiting for newer durability;
    every reply reports `resolved_at`. Continue only with issued positions/cuts and
    bound page tokens. Durable lag is accepted as measured, amended by the owner on
    2026-11-13: small about 99 ms; large median 391 ms (178–570); mixed median
    346 ms (173–493) at p95. No accepted-data snapshot is built; Nightly reports
    p50, p95 and maximum per load without a failing threshold.
    **Why:** repeatable reads and read-your-write receipts must not hide a persistence
    barrier. Lag remains measurement, never an assertion against real time in a test.
    **Where:** `openspec/changes/operation-grammar/specs/storage-visibility/spec.md`.
    **Proof:** Issued-page, native-list and cut-lifetime stories in
    `crates/harmos-runtime/tests/storage/` cover durable selection and continuation.
    `watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss` covers
    socket/child durable observation, opening races, filtered progress and resync;
    `durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps`
    in the native storage manager tests covers committed-prefix gating.
    `filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations`
    covers metadata observations, pinned continuation, eviction and restart refusal.
    The owner accepted the repeated benchmark measurements on 2026-11-13.
    **Status:** landed; native Linux/hosted measurements remain pending.

12. **Stable Receipts After Lost Replies.** The caller resends; the same request
    identifier and normalized payload return the original receipt, while a changed
    payload conflicts and expired identities refuse. Admitted completion remains
    owned independently of a retry window; accepted and durable receipts stay distinct.
    **Why:** a lost reply must not duplicate effects, lose an admitted result or
    falsely acknowledge persistence; this does not promise an unlimited replay ledger.
    **Where:** Client-Owned Resending After Failed Calls in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`;
    `openspec/specs/storage-mutations/spec.md`;
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** Storage's unary-transfer, Append-record/sample/object, definition-
    receipt and schema-mirror journeys in `crates/harmos-runtime/tests/storage/`
    cover original receipts, changed requests, admitted work after caller loss and
    retained windows. Native schema/index/projection restart journeys cover atomic
    outcomes and `admitted_create_completes` covers completion through retry expiry.
    **Status:** landed for the migrated storage surface; the complete service census
    and non-storage mutation joins remain under `operation-grammar`.

13. **Distinct Storage Guarantees.** Saved reads pin durable data; follows may join
    accepted data and must not label it saved. Bounded physical scans, ordered index
    seeks and exact-revision/frontier projections are distinct from bulk ingestion
    and atomic conditional object replacement; preserve backend durability guarantees.
    **Why:** one generic read/write label would erase consistency and conflict behavior.
    **Where:** `crates/harmos-storage/src/snapshot.rs`,
    `crates/harmos-storage/src/backend.rs`, `crates/harmos-storage/src/memory/`,
    `docs/storage.md`; requirement: Distinct Storage Guarantees in
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** `saved_windows_match_fallback_while_live_writes_and_cancellation_advance`
    in `crates/harmos-storage/tests/integration/columnar_snapshot.rs`;
    `seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart` in
    `crates/harmos-storage/tests/integration/index_seek.rs`;
    `projection_frontiers_survive_interrupted_publication_rebuild_and_restart` in
    `crates/harmos-storage/tests/integration/projection_replay.rs`;
    `conditional_objects_keep_conflicts_indexes_and_receipts_atomic_across_restart`
    in `crates/harmos-storage/tests/integration/conditional_objects.rs`.
    The memory adapter saves within its instance lifetime; a product backend owns
    durable media. Changing flush cadence never weakens a registered backend's
    guarantees. New durable Watch semantics remain invariant 11's direction.
    **Status:** landed.

14. **Refuse Incompatible Roots.** Incompatible host state definitions or corrupt
    recovery data refuse opening and preserve history; never reset silently.
    Back up before deliberately choosing/resetting a root; the application owns that decision.
    **Why:** an API/schema migration must not become silent data loss.
    **Where:** `crates/harmos-domain/src/state/transaction.rs`,
    `crates/harmos-runtime/src/storage/attachments.rs`, `docs/storage.md`;
    `openspec/specs/plugin-state-history/spec.md` and Safe Root Opening in
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** `changed_record_shape_refuses_root_and_preserves_history` in
    `crates/harmos-runtime/tests/storage/state/shape_versions.rs`;
    `corrupt_snapshot_blocks` in
    `crates/harmos-runtime/tests/storage/state/snapshots/corrupt_snapshot_blocks.rs`.
    **Status:** landed; backup/reset guidance is maintained in `docs/storage.md`.

15. **Callee-Owned Policy and Consumer Budgets.** Deadlines, delays and retries come
    from owning operation policies/configuration; callers override one call only.
    Never tune a budget or limit to make proof pass; consumers own storage budgets,
    admission, concurrency and response sizing, while the library owns correctness constants.
    **Why:** operational policy must have one owner and failures must stay visible.
    **Where:** `openspec/specs/operation-invocation/spec.md`,
    `openspec/specs/operation-authoring/spec.md`, `openspec/specs/canonical-build/spec.md`;
    Policy Ownership and Honest Budgets in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** `configured_policy_owns_io_retries_and_retirement` in
    `crates/harmos-storage/tests/integration/manager/policy.rs` proves configured
    I/O/recovery, retries, production capacity/lifetime and receipt expiry with
    controlled time. `a_child_deadline_override_does_not_change_the_next_storage_call`
    in `crates/harmos-runtime/tests/storage/callee_policy.rs` proves a plugin child
    override expires only its call. Existing operation policy stories preserve
    replay-safety admission. `StorageSettings.policy` owns operational defaults;
    request settings own production/receipt capacities and existing budgets.
    **Status:** landed for storage and invocation policy (task A.8).

16. **Crate Dependency Direction.** Runtime composes operations, storage, domain and
    protocol; operations depends on storage/query/domain/protocol, storage on query/domain/protocol,
    domain on protocol; the standalone Rust SDK depends on protocol/derive, never
    runtime, and lower crates and SDKs must not depend on runtime or a consuming product.
    **Why:** contracts and plugin/client authoring must work without host machinery;
    only runtime owns sandbox authority. Derive/build/dev edges do not invert production ownership.
    **Where:** `crates/harmos-runtime/Cargo.toml`, `crates/harmos-operations/Cargo.toml`,
    `crates/harmos-storage/Cargo.toml`, `crates/harmos-domain/Cargo.toml`,
    `crates/harmos-protocol/Cargo.toml`, `sdk/rust/Cargo.toml`;
    Library Dependency Direction in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** `test_production_edges_follow_domain_ownership` inspects normal and
    target production dependencies across all six libraries, excluding derive support.
    **Status:** landed.

17. **Carrier Helpers and Product Author SDKs.** Locked amendment, 2026-11-16:
    Harmos retains thin Rust, Python, C# and C++ carrier helpers, shared generation,
    supported guest ABI and generic conformance. Products own their contracts and
    the only author-facing SDKs and starters; Harmos types, attributes, namespaces,
    exceptions and escape hatches are invisible to authors. No import aliases.
    **Why:** several products reuse cancellation, windows and transport without
    inheriting another product's policy or copying carrier engines.
    **Where:** `openspec/changes/plugin-boundary/`, `operation-boundary` D7 and
    `plugin-lifecycle`'s amended registration contract.
    **Status:** locked direction; the following package inventory describes the
    landed implementation that will be reduced to helpers, not the target API.

    One SDK package per language, amended by the owner on
    2026-11-13: Python `harmos-sdk` (import `harmos`), Rust `harmos`, NuGet `Harmos`
    and CMake `Harmos` (`Harmos::Cpp`) include authoring, client roots and host
    protocol needs. Audience-specific builds and stub guards are removed. The host
    enforces faces and visibility at runtime, regardless of available bindings.
    The current four SDKs live in this repository, with
    the same concepts named consistently using each language's normal casing;
    product SDKs currently compose them. Library contracts, conformance and guides use generic
    vocabulary without product objects or private identities.
    **Why:** independent hosts must reuse the same library, not inherit another
    application's model. Common naming does not claim unresolved semantic parity.
    **Where:** `sdk/rust/`, `sdk/python/`, `sdk/csharp/`, `sdk/cpp/`,
    `tools/conformance/`; Self-Sufficient Library SDK Ownership in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** the independent reference host runs the shared conformance suite in
    each SDK. Four-language client-root fixtures compose library and product descriptors.
    Native plugin semantic parity is agreed; generic managed application clients remain
    Rust-only, while other languages use their generated application gRPC clients.
    C++ installs `Harmos::Cpp` with config-package dependencies; its conformance
    implementation belongs to example support, outside the installed library. Conformance
    protocol input lives under `sdk/tests/fixtures/conformance/proto/`; generated fixture
    messages and implementations belong only to test and conformance targets in every SDK.
    C# builds every project through `sdk/csharp/Harmos.slnx`; central properties and
    package versions own the .NET build. Committed client roots live in `src/Harmos/Generated`
    and `tests/Harmos.Tests/Generated`. All build outputs live under `artifacts`; executable
    manifests from MSBuild are the path authority for test and fixture consumers. NuGet `Harmos`
    takes its version from the Cargo workspace and includes SourceLink and repository metadata.

    **Status:** landed for these SDK contracts; Wasm is Rust-only.

18. **Deterministic Stories.** Correctness tests follow journeys using controlled
    clocks/events, never assertions on real elapsed time, machine load or sampled
    memory; measurements belong to benchmarks. Never test authored prose or one
    repository constant, and never add tautological assertions.
    **Why:** proof must fail on broken behavior, independently of the workstation.
    **Where:** `openspec/changes/test-stories/specs/test-stories/spec.md`;
    Deterministic Behavioral Proof in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** No proof yet for repository-wide compliance; controlled-clock examples
    such as invariant 23 do not establish it for every suite.
    **Status:** locked direction (`operation-grammar`, retaining the agreed `test-stories` rule).

19. **Declaration Integrity Before Activation.** Validate declarations digest, descriptors,
    schema fingerprints and requirements before admitting operations; refuse
    ambiguous providers instead of choosing by arrival order.
    **Why:** opaque bytes are safe to route only under the admitted contract, and
    a renamed operation must not silently satisfy an old requirement.
    **Where:** `crates/harmos-protocol/src/schema/mod.rs`,
    `crates/harmos-runtime/src/plugin/grpc/catalog.rs`,
    `crates/harmos-runtime/src/operations/manager/admission.rs`;
    Catalog Admission Integrity in
    `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
    **Proof:** `host_refuses_tampered_catalogs_before_activation` in
    `crates/harmos-runtime/tests/plugin_grpc/catalog_admission.rs`;
    `duplicate_operation_names_both_owners` in
    `crates/harmos-runtime/tests/plugin_grpc/duplicate_operation.rs`.
    **Status:** landed for gRPC plugin admission.

20. **Committed Stream Revisions.** Derive revisions from canonical row schema and
    window; treat hashes as opaque, never choose current by numeric order. Activation
    commits current selection atomically; failed staging preserves it and readers
    retain their own revision's descriptor.
    **Why:** reload must not decode old rows with a new schema or invalidate retained readers.
    **Where:** `crates/harmos-domain/src/schema/stream_revision.rs`,
    `crates/harmos-runtime/src/streams/manager.rs`;
    `openspec/specs/stream-revisions/spec.md`.
    **Proof:** `changed_row_commits_a_new_current_revision`,
    `failed_reload_keeps_old_revision_current` and
    `restart_on_one_root_reads_each_revision_with_its_own_schema` in
    `crates/harmos-runtime/tests/streams/revisions.rs`.
    **Status:** landed.

21. **Retained Plugin History.** Read plugin-owned history with its written
    definitions after shape changes or removal; incompatible typed current reads
    refuse rather than converting retained history to current state.
    **Why:** removing/replacing executable code must not erase historical evidence;
    host recovery compatibility remains a separate constraint (invariant 14).
    **Where:** `crates/harmos-domain/src/state/envelope.rs`,
    `crates/harmos-runtime/src/storage/attachments.rs`;
    `openspec/specs/plugin-state-history/spec.md`.
    **Proof:** `plugin_history_survives_shape_change_and_absence_on_one_root` in
    `crates/harmos-runtime/tests/storage/state/shape_versions.rs`.
    **Status:** landed.

22. **Fenced File Production.** Production and publication are distinct: only the
    current writer may append/publish; resumption fences the previous writer and
    abandonment is explicit. Reconnection must not silently publish or delete work.
    **Why:** interrupted producers need recoverable progress without competing writers
    or treating partially written bytes as a finished file.
    **Where:** `crates/harmos-storage/src/staging/mutations.rs`,
    `crates/harmos-storage/src/staging/session.rs`,
    `crates/harmos-storage/src/file_production.rs`,
    `crates/harmos-storage/src/backend.rs`;
    `openspec/changes/operation-grammar/specs/storage-transfers/spec.md`.
    **Proof:** `sessions_survive_reopen` in
    `crates/harmos-storage/tests/integration/file_sessions.rs` and
    `mutation_receipts_survive_replays_lost_responses_and_cancellation` in
    `crates/harmos-runtime/tests/storage/manager/receipts.rs`.
    `resume_fence_publish_and_range_across_faces` guards unary client/plugin transfer
    sessions, replay, fencing and bounded immutable reads. Bundled manager stories
    upload through the same client session operations. Diagnostics and file snapshots
    share `harmos_storage::ContentSource` and `ContentRead`; the service manager owns archive retention.
    **Status:** landed for current production sessions.

23. **Authoritative Host Time.** Plugin waits and interval publication use the host
    clock; active work prevents virtual time from advancing past unsettled work.
    Device-driven publication remains distinct from host interval windows.
    **Why:** deterministic simulation must see plugin demand and work ownership,
    rather than race a plugin's independent wall clock.
    **Where:** `crates/harmos-runtime/src/clock.rs`,
    `crates/harmos-runtime/src/plugin/grpc/invoke.rs`,
    `crates/harmos-runtime/src/plugin/grpc/publish.rs`;
    Authoritative Host Time in
    `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
    **Proof:** `an_in_flight_plugin_call_blocks_virtual_time_until_settlement` in
    `crates/harmos-runtime/tests/plugin_grpc/active_call.rs`;
    `host_windows_are_stored_in_order_with_host_timestamps` in
    `crates/harmos-runtime/tests/plugin_grpc/interval_stream.rs`,
    `windows_cover_exact_sample_ranges_on_a_controlled_clock` in
    `crates/harmos-runtime/tests/plugin_grpc/interval_windows.rs` and
    `device_samples_need_no_window_and_keep_source_time_separate` in
    `crates/harmos-runtime/tests/plugin_grpc/device_stream.rs`.
    **Status:** landed.

24. **Atomic Recovery Snapshots.** Preserve state, undo history, receipts and tail
    in verified ordered bounded segments; publish only complete durable snapshots
    and remove covered journal entries only after publication.
    **Why:** interrupted checkpoint work must leave recoverable state equivalent
    to journal replay, without silently dropping undo or deduplication history.
    **Where:** `crates/harmos-runtime/src/state/snapshot_codec.rs`,
    `crates/harmos-storage/src/state/segments.rs`;
    `openspec/specs/segmented-state-snapshots/spec.md`.
    **Proof:** `segmented_snapshot_matches_full_replay` in
    `crates/harmos-runtime/tests/storage/state/snapshots/segmented_snapshot_matches_full_replay.rs`;
    `snapshot_failure_retryable` in
    `crates/harmos-runtime/tests/storage/state/snapshots/snapshot_failure_retryable.rs`.
    **Status:** landed.

## Glossary

- **Face / Visibility:** wire audience/serving boundary versus operation admission level; neither replaces the other.
- **Operation / Invocation / Attempt:** declared contract / one admitted execution / one try under its retry policy; an attempt is not a new logical request.
- **Position / Cut / Page Token / Snapshot:** progress in one history / a consistent selected boundary (possibly composite) / opaque continuation bound to a selection / retained read image or recovery image identified by context; none is a content digest or interchangeable cursor.
- **Accepted / Durable:** admitted into owned work versus acknowledged persisted under backend guarantees; accepted is not crash-safe acknowledgement.
- **Instance / Plugins Service:** one running plugin activation versus host management of plugin entries; the management service is not the plugin protocol endpoint.
- **Manager / Service:** owner of live orchestration and lifecycle versus an API service or resident lifecycle participant, as qualified; a service name is not a new manager.
- **Scope / Grant:** opaque inherited caller context versus checked permission to an operation/resource; scope text is never a token of authority.

## Decisions and Their Reasons

| Decision | Reason | Recording Change |
| --- | --- | --- |
| Three visibility levels; explicit choice; decided built-in mapping (2026-11-12) | Local privacy, host-only control and exposed peer calls need separate admission meanings. | `openspec/changes/operation-grammar/` |
| Executor layout and mandatory callers (2026-11-14) | One operation home; caller admission is independent of executor and visibility. | `openspec/changes/operation-grammar/` |
| Runtime composition, manager inventory and product-owned settings/secrets (2026-11-12) | Reuse generic machinery under one lifecycle authority while retaining application policy. | `openspec/changes/operation-grammar/` |
| Resource grammar, nested packages and shared primitives (2026-11-12) | Generate predictable SDKs without flattening storage guarantees. | `openspec/changes/operation-grammar/` |
| Latest durable reads, measured lag and caller resending (2026-11-12) | Repeatable reads and lost replies must preserve durable boundaries and original outcomes. | `openspec/changes/operation-grammar/` |
| No automatic twins; distinct Stop and Cancel (2026-11-12) | Keep one method per promise and preserve cooperative final results. | `openspec/changes/operation-grammar/` |
| Host-only control uses Protected | Keep host controls on the instance carrier and refuse peer/client calls. | `openspec/changes/operation-grammar/` |
| Retained plugin history and derived revisions (2026-11-07) | Written schema identity must outlive plugin replacement or removal. | `openspec/changes/archive/2026-11-07-plugin-state-survives-shape-changes/`, `openspec/changes/archive/2026-11-07-stream-schema-revisions/` |
| Segmented recovery snapshots (2026-11-08) | Bounded checkpoint pieces must preserve complete recovery and atomic publication. | `openspec/changes/archive/2026-11-08-snapshot-payload-limit/` |
| Deterministic stories (agreed 2026-11-10) | Correctness must not depend on workstation speed or measurement noise. | `openspec/changes/test-stories/` |

## Protocol Boundary Definition and Consumer Migration

This section records the landed protocol and its earlier consumer migration.
Its direct service composition, plugin administration and author-SDK assignments
are superseded by Plugin Boundary and `operation-boundary`. Message maps remain
source evidence, not the future carrier or product contract inventory.

### Executor Layout and Definition Status

The 2026-11-14 owner decision replaces audience packages with executor packages.
Protocol declarations, runtime admission, SDKs, conformance and example consumers
use this layout. Former names
in the mapping are migration references only. There are no compatibility packages.

```text
proto/harmos/model/v1/    harmos.model.v1     exchanged values; no services
proto/harmos/host/v1/     harmos.host.v1      operations executed on the host
proto/harmos/plugin/v1/   harmos.plugin.v1    operations executed on a plugin
```

Before moving files, the duplicate inventory was: Operations List/Get and Time
Get/Sleep are identical pairs; Invocations joins client Invoke with shared Get,
Cancel and Watch, renaming Invoke to Create; Settings joins nested client List/Set
with plugin-host List, keeping the richer response and one optional name selector.
Settings.List was the only same-name clash with different field layouts: the new
shape retains the client's name=1, at=2 and declarations/value/revision/resolved_at
response tags 1–4. All other operations keep their names and field numbers.

The tree changes from 33 proto files, 19 services and 45 operations to 33 proto
files, 15 services and 37 operations (host 14/33; plugin 1/4; model zero services).
Storage operations stay in-process and are outside both censuses. The runtime
always includes its storage manager and default in-memory adapter; this pass
makes no storage/query source changes.

### Call Mapping

Every row names the former identity, its single replacement, its replacement
request/response and the declared callers. Repeated target identities are merged
operations, not duplicate declarations. Resource segments follow the version:
`harmos.host.v1.plugins.Settings` is `host/v1/plugins/settings.proto`.

| Former Identity | New Identity | New Request / Response | Callers |
| --- | --- | --- | --- |
| `harmos.client.v1.access.Certificates/Create` | `harmos.host.v1.access.Certificates/Create` | `harmos.host.v1.access.CertificatesCreateRequest` / `harmos.host.v1.access.CertificatesCreateResponse` | CLIENTS |
| `harmos.client.v1.access.Listener/Get` | `harmos.host.v1.access.Listener/Get` | `harmos.host.v1.access.ListenerGetRequest` / `harmos.host.v1.access.ListenerGetResponse` | CLIENTS |
| `harmos.client.v1.access.Listener/Set` | `harmos.host.v1.access.Listener/Set` | `harmos.host.v1.access.ListenerSetRequest` / `harmos.host.v1.access.ListenerSetResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/Create` | `harmos.host.v1.access.Pairings/Create` | `harmos.host.v1.access.PairingsCreateRequest` / `harmos.host.v1.access.PairingsCreateResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/List` | `harmos.host.v1.access.Pairings/List` | `harmos.host.v1.access.PairingsListRequest` / `harmos.host.v1.access.PairingsListResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/Delete` | `harmos.host.v1.access.Pairings/Delete` | `harmos.host.v1.access.PairingsDeleteRequest` / `harmos.host.v1.access.PairingsDeleteResponse` | CLIENTS |
| `harmos.client.v1.Invocations/Invoke` | `harmos.host.v1.Invocations/Create` | `harmos.host.v1.InvocationsCreateRequest` / `harmos.host.v1.InvocationsCreateResponse` | CLIENTS |
| `harmos.client.v1.Invocations/Get` | `harmos.host.v1.Invocations/Get` | `harmos.host.v1.InvocationsGetRequest` / `harmos.host.v1.InvocationsGetResponse` | ALL |
| `harmos.client.v1.Invocations/Cancel` | `harmos.host.v1.Invocations/Cancel` | `harmos.host.v1.InvocationsCancelRequest` / `harmos.host.v1.InvocationsCancelResponse` | ALL |
| `harmos.client.v1.Invocations/Watch` | `harmos.host.v1.Invocations/Watch` | `harmos.host.v1.InvocationsWatchRequest` / `harmos.host.v1.InvocationsWatchResponse` | ALL |
| `harmos.client.v1.Operations/List` | `harmos.host.v1.Operations/List` | `harmos.host.v1.OperationsListRequest` / `harmos.host.v1.OperationsListResponse` | ALL |
| `harmos.client.v1.Operations/Get` | `harmos.host.v1.Operations/Get` | `harmos.host.v1.OperationsGetRequest` / `harmos.host.v1.OperationsGetResponse` | ALL |
| `harmos.client.v1.Plugins/Get` | `harmos.host.v1.Plugins/Get` | `harmos.host.v1.PluginsGetRequest` / `harmos.host.v1.PluginsGetResponse` | CLIENTS |
| `harmos.client.v1.Plugins/List` | `harmos.host.v1.Plugins/List` | `harmos.host.v1.PluginsListRequest` / `harmos.host.v1.PluginsListResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Watch` | `harmos.host.v1.Plugins/Watch` | `harmos.host.v1.PluginsWatchRequest` / `harmos.host.v1.PluginsWatchResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Create` | `harmos.host.v1.Plugins/Create` | `harmos.host.v1.PluginsCreateRequest` / `harmos.host.v1.PluginsCreateResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Set` | `harmos.host.v1.Plugins/Set` | `harmos.host.v1.PluginsSetRequest` / `harmos.host.v1.PluginsSetResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Delete` | `harmos.host.v1.Plugins/Delete` | `harmos.host.v1.PluginsDeleteRequest` / `harmos.host.v1.PluginsDeleteResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Restart` | `harmos.host.v1.Plugins/Restart` | `harmos.host.v1.PluginsRestartRequest` / `harmos.host.v1.PluginsRestartResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Replace` | `harmos.host.v1.Plugins/Replace` | `harmos.host.v1.PluginsReplaceRequest` / `harmos.host.v1.PluginsReplaceResponse` | CLIENTS |
| `harmos.client.v1.plugins.Settings/List` | `harmos.host.v1.plugins.Settings/List` | `harmos.host.v1.plugins.SettingsListRequest` / `harmos.host.v1.plugins.SettingsListResponse` | ALL |
| `harmos.client.v1.plugins.Settings/Set` | `harmos.host.v1.plugins.Settings/Set` | `harmos.host.v1.plugins.SettingsSetRequest` / `harmos.host.v1.plugins.SettingsSetResponse` | CLIENTS |
| `harmos.client.v1.Schemas/List` | `harmos.host.v1.Schemas/List` | `harmos.host.v1.SchemasListRequest` / `harmos.host.v1.SchemasListResponse` | CLIENTS |
| `harmos.client.v1.Status/Get` | `harmos.host.v1.Status/Get` | `harmos.host.v1.StatusGetRequest` / `harmos.host.v1.StatusGetResponse` | CLIENTS |
| `harmos.client.v1.Status/Watch` | `harmos.host.v1.Status/Watch` | `harmos.host.v1.StatusWatchRequest` / `harmos.host.v1.StatusWatchResponse` | CLIENTS |
| `harmos.client.v1.status.Diagnostics/Get` | `harmos.host.v1.status.Diagnostics/Get` | `harmos.host.v1.status.DiagnosticsGetRequest` / `harmos.host.v1.status.DiagnosticsGetResponse` | CLIENTS |
| `harmos.client.v1.status.Logs/List` | `harmos.host.v1.status.Logs/List` | `harmos.host.v1.status.LogsListRequest` / `harmos.host.v1.status.LogsListResponse` | CLIENTS |
| `harmos.client.v1.status.Logs/Watch` | `harmos.host.v1.status.Logs/Watch` | `harmos.host.v1.status.LogsWatchRequest` / `harmos.host.v1.status.LogsWatchResponse` | CLIENTS |
| `harmos.client.v1.status.TraceEvents/List` | `harmos.host.v1.status.TraceEvents/List` | `harmos.host.v1.status.TraceEventsListRequest` / `harmos.host.v1.status.TraceEventsListResponse` | CLIENTS |
| `harmos.client.v1.status.TraceEvents/Watch` | `harmos.host.v1.status.TraceEvents/Watch` | `harmos.host.v1.status.TraceEventsWatchRequest` / `harmos.host.v1.status.TraceEventsWatchResponse` | CLIENTS |
| `harmos.client.v1.Streams/List` | `harmos.host.v1.Streams/List` | `harmos.host.v1.StreamsListRequest` / `harmos.host.v1.StreamsListResponse` | CLIENTS |
| `harmos.client.v1.Time/Get` | `harmos.host.v1.Time/Get` | `harmos.host.v1.TimeGetRequest` / `harmos.host.v1.TimeGetResponse` | ALL |
| `harmos.client.v1.Time/Sleep` | `harmos.host.v1.Time/Sleep` | `harmos.host.v1.TimeSleepRequest` / `harmos.host.v1.TimeSleepResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Get` | `harmos.host.v1.Invocations/Get` | `harmos.host.v1.InvocationsGetRequest` / `harmos.host.v1.InvocationsGetResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Cancel` | `harmos.host.v1.Invocations/Cancel` | `harmos.host.v1.InvocationsCancelRequest` / `harmos.host.v1.InvocationsCancelResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Watch` | `harmos.host.v1.Invocations/Watch` | `harmos.host.v1.InvocationsWatchRequest` / `harmos.host.v1.InvocationsWatchResponse` | ALL |
| `harmos.plugin.host.v1.Operations/List` | `harmos.host.v1.Operations/List` | `harmos.host.v1.OperationsListRequest` / `harmos.host.v1.OperationsListResponse` | ALL |
| `harmos.plugin.host.v1.Operations/Get` | `harmos.host.v1.Operations/Get` | `harmos.host.v1.OperationsGetRequest` / `harmos.host.v1.OperationsGetResponse` | ALL |
| `harmos.plugin.host.v1.Settings/List` | `harmos.host.v1.plugins.Settings/List` | `harmos.host.v1.plugins.SettingsListRequest` / `harmos.host.v1.plugins.SettingsListResponse` | ALL |
| `harmos.plugin.host.v1.Time/Get` | `harmos.host.v1.Time/Get` | `harmos.host.v1.TimeGetRequest` / `harmos.host.v1.TimeGetResponse` | ALL |
| `harmos.plugin.host.v1.Time/Sleep` | `harmos.host.v1.Time/Sleep` | `harmos.host.v1.TimeSleepRequest` / `harmos.host.v1.TimeSleepResponse` | ALL |
| `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.InstanceGetRequest` / `harmos.plugin.v1.InstanceGetResponse` | Host |
| `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.InstanceInvokeRequest` / `harmos.plugin.v1.InstanceInvokeResponse` | Host |
| `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.InstanceSamplesRequest` / `harmos.plugin.v1.InstanceSamplesResponse` | Host |
| `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.InstanceStopRequest` / `harmos.plugin.v1.InstanceStopResponse` | Host |

### Message and Enum Mapping

Every top-level message and enum from the former tree is listed, including the
unchanged Instance wrappers so carrier moves can be applied mechanically. Nested
messages, oneofs and enum identities follow their enclosing type's replacement
prefix, with their names, numbers and order unchanged. The method-option extension
is new: `harmos.model.v1.callers` in `model/v1/callers.proto`.

| Former Identity | New Identity | New Source |
| --- | --- | --- |
| `harmos.client.v1.access.CertificatesCreateRequest` | `harmos.host.v1.access.CertificatesCreateRequest` | `harmos/host/v1/access/certificates.proto` |
| `harmos.client.v1.access.CertificatesCreateResponse` | `harmos.host.v1.access.CertificatesCreateResponse` | `harmos/host/v1/access/certificates.proto` |
| `harmos.client.v1.access.ListenerGetRequest` | `harmos.host.v1.access.ListenerGetRequest` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerGetResponse` | `harmos.host.v1.access.ListenerGetResponse` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerSetRequest` | `harmos.host.v1.access.ListenerSetRequest` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerSetResponse` | `harmos.host.v1.access.ListenerSetResponse` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerValue` | `harmos.host.v1.access.ListenerValue` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.PairingsCreateRequest` | `harmos.host.v1.access.PairingsCreateRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsCreateResponse` | `harmos.host.v1.access.PairingsCreateResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsListRequest` | `harmos.host.v1.access.PairingsListRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsListResponse` | `harmos.host.v1.access.PairingsListResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsDeleteRequest` | `harmos.host.v1.access.PairingsDeleteRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsDeleteResponse` | `harmos.host.v1.access.PairingsDeleteResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.Pairing` | `harmos.host.v1.access.Pairing` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.InvocationsInvokeRequest` | `harmos.host.v1.InvocationsCreateRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsInvokeResponse` | `harmos.host.v1.InvocationsCreateResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsGetRequest` | `harmos.host.v1.InvocationsGetRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsGetResponse` | `harmos.host.v1.InvocationsGetResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsCancelRequest` | `harmos.host.v1.InvocationsCancelRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsCancelResponse` | `harmos.host.v1.InvocationsCancelResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsWatchRequest` | `harmos.host.v1.InvocationsWatchRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsWatchResponse` | `harmos.host.v1.InvocationsWatchResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.OperationsListRequest` | `harmos.host.v1.OperationsListRequest` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsListResponse` | `harmos.host.v1.OperationsListResponse` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsGetRequest` | `harmos.host.v1.OperationsGetRequest` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsGetResponse` | `harmos.host.v1.OperationsGetResponse` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.PluginsGetRequest` | `harmos.host.v1.PluginsGetRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsGetResponse` | `harmos.host.v1.PluginsGetResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsListRequest` | `harmos.host.v1.PluginsListRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsListResponse` | `harmos.host.v1.PluginsListResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsWatchRequest` | `harmos.host.v1.PluginsWatchRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsWatchResponse` | `harmos.host.v1.PluginsWatchResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsCreateRequest` | `harmos.host.v1.PluginsCreateRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsCreateResponse` | `harmos.host.v1.PluginsCreateResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsSetRequest` | `harmos.host.v1.PluginsSetRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsSetResponse` | `harmos.host.v1.PluginsSetResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsDeleteRequest` | `harmos.host.v1.PluginsDeleteRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsDeleteResponse` | `harmos.host.v1.PluginsDeleteResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsRestartRequest` | `harmos.host.v1.PluginsRestartRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsRestartResponse` | `harmos.host.v1.PluginsRestartResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsReplaceRequest` | `harmos.host.v1.PluginsReplaceRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsReplaceResponse` | `harmos.host.v1.PluginsReplaceResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginValue` | `harmos.host.v1.PluginValue` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginDescription` | `harmos.host.v1.PluginDescription` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginRemote` | `harmos.host.v1.PluginRemote` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginManaged` | `harmos.host.v1.PluginManaged` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginGuest` | `harmos.host.v1.PluginGuest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginPlacement` | `harmos.host.v1.PluginPlacement` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginProcess` | `harmos.host.v1.PluginProcess` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginEntry` | `harmos.host.v1.PluginEntry` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginSnapshot` | `harmos.host.v1.PluginSnapshot` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginChange` | `harmos.host.v1.PluginChange` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginState` | `harmos.host.v1.PluginState` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.plugins.SettingsListRequest` | `harmos.host.v1.plugins.SettingsListRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsListResponse` | `harmos.host.v1.plugins.SettingsListResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsSetRequest` | `harmos.host.v1.plugins.SettingsSetRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsSetResponse` | `harmos.host.v1.plugins.SettingsSetResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.SchemasListRequest` | `harmos.host.v1.SchemasListRequest` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.SchemasListResponse` | `harmos.host.v1.SchemasListResponse` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.SchemaDescription` | `harmos.host.v1.SchemaDescription` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.StatusGetRequest` | `harmos.host.v1.StatusGetRequest` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusGetResponse` | `harmos.host.v1.StatusGetResponse` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusWatchRequest` | `harmos.host.v1.StatusWatchRequest` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusWatchResponse` | `harmos.host.v1.StatusWatchResponse` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HostIdentity` | `harmos.host.v1.HostIdentity` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HealthComponent` | `harmos.host.v1.HealthComponent` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HostStatus` | `harmos.host.v1.HostStatus` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.ComponentState` | `harmos.host.v1.ComponentState` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.status.DiagnosticsGetRequest` | `harmos.host.v1.status.DiagnosticsGetRequest` | `harmos/host/v1/status/diagnostics.proto` |
| `harmos.client.v1.status.DiagnosticsGetResponse` | `harmos.host.v1.status.DiagnosticsGetResponse` | `harmos/host/v1/status/diagnostics.proto` |
| `harmos.client.v1.status.LogsListRequest` | `harmos.host.v1.status.LogsListRequest` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsListResponse` | `harmos.host.v1.status.LogsListResponse` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsWatchRequest` | `harmos.host.v1.status.LogsWatchRequest` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsWatchResponse` | `harmos.host.v1.status.LogsWatchResponse` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.Event` | `harmos.host.v1.status.Event` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.TraceEventsListRequest` | `harmos.host.v1.status.TraceEventsListRequest` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsListResponse` | `harmos.host.v1.status.TraceEventsListResponse` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsWatchRequest` | `harmos.host.v1.status.TraceEventsWatchRequest` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsWatchResponse` | `harmos.host.v1.status.TraceEventsWatchResponse` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.StreamsListRequest` | `harmos.host.v1.StreamsListRequest` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.StreamsListResponse` | `harmos.host.v1.StreamsListResponse` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.StreamIdentity` | `harmos.host.v1.StreamIdentity` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.TimeGetRequest` | `harmos.host.v1.TimeGetRequest` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeGetResponse` | `harmos.host.v1.TimeGetResponse` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeSleepRequest` | `harmos.host.v1.TimeSleepRequest` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeSleepResponse` | `harmos.host.v1.TimeSleepResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.InvocationsGetRequest` | `harmos.host.v1.InvocationsGetRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsGetResponse` | `harmos.host.v1.InvocationsGetResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsCancelRequest` | `harmos.host.v1.InvocationsCancelRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsCancelResponse` | `harmos.host.v1.InvocationsCancelResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsWatchRequest` | `harmos.host.v1.InvocationsWatchRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsWatchResponse` | `harmos.host.v1.InvocationsWatchResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.OperationsListRequest` | `harmos.host.v1.OperationsListRequest` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsListResponse` | `harmos.host.v1.OperationsListResponse` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsGetRequest` | `harmos.host.v1.OperationsGetRequest` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsGetResponse` | `harmos.host.v1.OperationsGetResponse` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.SettingsListRequest` | `harmos.host.v1.plugins.SettingsListRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.plugin.host.v1.SettingsListResponse` | `harmos.host.v1.plugins.SettingsListResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.plugin.host.v1.TimeGetRequest` | `harmos.host.v1.TimeGetRequest` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeGetResponse` | `harmos.host.v1.TimeGetResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeSleepRequest` | `harmos.host.v1.TimeSleepRequest` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeSleepResponse` | `harmos.host.v1.TimeSleepResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.v1.InstanceGetRequest` | `harmos.plugin.v1.InstanceGetRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceGetResponse` | `harmos.plugin.v1.InstanceGetResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceInvokeRequest` | `harmos.plugin.v1.InstanceInvokeRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceInvokeResponse` | `harmos.plugin.v1.InstanceInvokeResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceSamplesRequest` | `harmos.plugin.v1.InstanceSamplesRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceSamplesResponse` | `harmos.plugin.v1.InstanceSamplesResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceStopRequest` | `harmos.plugin.v1.InstanceStopRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceStopResponse` | `harmos.plugin.v1.InstanceStopResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.v1.Declarations` | `harmos.model.v1.Declarations` | `harmos/model/v1/description.proto` |
| `harmos.v1.Section` | `harmos.model.v1.Section` | `harmos/model/v1/description.proto` |
| `harmos.v1.Failure` | `harmos.model.v1.Failure` | `harmos/model/v1/failure.proto` |
| `harmos.v1.Classification` | `harmos.model.v1.Classification` | `harmos/model/v1/failure.proto` |
| `harmos.v1.FileMetadata` | `harmos.model.v1.FileMetadata` | `harmos/model/v1/file.proto` |
| `harmos.v1.FileReference` | `harmos.model.v1.FileReference` | `harmos/model/v1/file.proto` |
| `harmos.v1.File` | `harmos.model.v1.File` | `harmos/model/v1/file.proto` |
| `harmos.v1.Upload` | `harmos.model.v1.Upload` | `harmos/model/v1/file.proto` |
| `harmos.v1.UploadState` | `harmos.model.v1.UploadState` | `harmos/model/v1/file.proto` |
| `harmos.v1.GuestRegistration` | `harmos.plugin.v1.GuestRegistration` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestRequirement` | `harmos.plugin.v1.GuestRequirement` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestCommand` | `harmos.plugin.v1.GuestCommand` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestPrepare` | `harmos.plugin.v1.GuestPrepare` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestInvocation` | `harmos.plugin.v1.GuestInvocation` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestValue` | `harmos.plugin.v1.GuestValue` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestInitialize` | `harmos.plugin.v1.GuestInitialize` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestShutdown` | `harmos.plugin.v1.GuestShutdown` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChild` | `harmos.plugin.v1.GuestChild` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildRequest` | `harmos.plugin.v1.GuestChildRequest` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildControl` | `harmos.plugin.v1.GuestChildControl` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestSubmitted` | `harmos.plugin.v1.GuestSubmitted` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildAction` | `harmos.plugin.v1.GuestChildAction` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.Request` | `harmos.plugin.v1.Request` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.Cancel` | `harmos.plugin.v1.Cancel` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.Response` | `harmos.plugin.v1.Response` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildRequest` | `harmos.plugin.v1.ChildRequest` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildDemand` | `harmos.plugin.v1.ChildDemand` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildCancel` | `harmos.plugin.v1.ChildCancel` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildResponse` | `harmos.plugin.v1.ChildResponse` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.InstanceState` | `harmos.model.v1.InstanceState` | `harmos/model/v1/invocation.proto` |
| `harmos.v1.OperationContract` | `harmos.model.v1.OperationContract` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Requirement` | `harmos.model.v1.Requirement` | `harmos/model/v1/operation.proto` |
| `harmos.v1.AssignedOperation` | `harmos.plugin.v1.AssignedOperation` | `harmos/plugin/v1/operation.proto` |
| `harmos.v1.OperationAssignments` | `harmos.plugin.v1.OperationAssignments` | `harmos/plugin/v1/operation.proto` |
| `harmos.v1.Protobuf` | `harmos.model.v1.Protobuf` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Grpc` | `harmos.model.v1.Grpc` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Grant` | `harmos.model.v1.Grant` | `harmos/model/v1/operation.proto` |
| `harmos.v1.DiscoveredContract` | `harmos.model.v1.DiscoveredContract` | `harmos/model/v1/operation.proto` |
| `harmos.v1.DiscoveredOperation` | `harmos.model.v1.DiscoveredOperation` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Visibility` | `harmos.model.v1.Visibility` | `harmos/model/v1/operation.proto` |
| `harmos.v1.PageRequest` | `harmos.model.v1.PageRequest` | `harmos/model/v1/page.proto` |
| `harmos.v1.OperationPolicy` | `harmos.model.v1.OperationPolicy` | `harmos/model/v1/policy.proto` |
| `harmos.v1.RetryPolicy` | `harmos.model.v1.RetryPolicy` | `harmos/model/v1/policy.proto` |
| `harmos.v1.History` | `harmos.model.v1.History` | `harmos/model/v1/position.proto` |
| `harmos.v1.Position` | `harmos.model.v1.Position` | `harmos/model/v1/position.proto` |
| `harmos.v1.ResourceRevision` | `harmos.model.v1.ResourceRevision` | `harmos/model/v1/position.proto` |
| `harmos.v1.ReadCut` | `harmos.model.v1.ReadCut` | `harmos/model/v1/position.proto` |
| `harmos.v1.Receipt` | `harmos.model.v1.Receipt` | `harmos/model/v1/receipt.proto` |
| `harmos.v1.SchemaKey` | `harmos.host.v1.SchemaKey` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaField` | `harmos.host.v1.SchemaField` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaDefinition` | `harmos.host.v1.SchemaDefinition` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaRepresentation` | `harmos.host.v1.SchemaRepresentation` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaGraph` | `harmos.host.v1.SchemaGraph` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaInput` | `deleted (unused wire message)` | `—` |
| `harmos.v1.PayloadEncoding` | `harmos.host.v1.PayloadEncoding` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaCardinality` | `harmos.host.v1.SchemaCardinality` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaKind` | `harmos.host.v1.SchemaKind` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaForm` | `harmos.host.v1.SchemaForm` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaOmission` | `harmos.host.v1.SchemaOmission` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SettingsDeclarations` | `harmos.model.v1.SettingsDeclarations` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingDeclaration` | `harmos.model.v1.SettingDeclaration` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingValue` | `harmos.model.v1.SettingValue` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingValues` | `harmos.model.v1.SettingValues` | `harmos/model/v1/setting.proto` |
| `harmos.v1.StreamDeclaration` | `harmos.model.v1.StreamDeclaration` | `harmos/model/v1/stream.proto` |
| `harmos.v1.Tick` (deleted) | `harmos.plugin.v1.Window` and `harmos.plugin.v1.SampleBatch` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.SamplesSelection` | `harmos.plugin.v1.SamplesSelection` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.Sample` | `harmos.plugin.v1.Sample` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.StorageTransferSettings` | `harmos.model.v1.StorageTransferSettings` | `harmos/model/v1/transfer.proto` |
| `harmos.v1.MessageSettings` | `harmos.model.v1.MessageSettings` | `harmos/model/v1/transfer.proto` |
| `harmos.v1.TransferFraming` | `harmos.model.v1.TransferFraming` | `harmos/model/v1/transfer.proto` |

Decision: Instance uses CLIENTS because its host is the connection client; only
that host may connect. No library host operation is currently PLUGINS-only, so
rule 33 uses a fixture operation. SchemaKey is also used by Streams.List; it remains
registry vocabulary beside Schemas, not a model storage interface. Behaviour now
lives at `proto/harmos/plugin/v1/behaviour.md` beside the connection service.

### Caller Options and SDK Projection

```proto
// model/v1/callers.proto
extend google.protobuf.MethodOptions { Callers callers = 51001; }
enum Callers { CALLERS_UNSPECIFIED = 0; CLIENTS = 1; PLUGINS = 2; ALL = 3; }
// host/v1/time.proto
rpc Get(TimeGetRequest) returns (TimeGetResponse) {
  option (harmos.model.v1.callers) = ALL;
}
```

Every RPC under host/ requires a present, recognized nonzero option;
`mise run check -- --repository` compiles descriptors and checks it, alongside Buf
format and lint. Generated descriptors retain custom method options. Runtime consumers must read
those options from the raw descriptor bytes with prost_reflect; decoding and
re-encoding through prost_types discards unknown custom options. A host method's
executor package does not imply its callers. Plugin executor operations are called
by the host by definition and carry no callers option; CLIENTS always means client
programs, never the host. Host operations with CLIENTS or ALL
generate the client surface; PLUGINS or ALL generate the plugin context. Plugin
executor services generate connection stubs, never a host client/context resource.
Reference documentation must use these same projections. Visibility, fingerprints,
trusted identity, inherited scope and resource grants remain independent gates.
The host must refuse a wrong caller before executing any handler or admitting a
managed/encoded child; hiding the method in an SDK is not authorization.

`plugins.Settings.List` clients must supply name. Plugins may omit it or name their
own trusted entry; another name is refused before storage access. Its response
contains declarations, complete values, revision and resolved_at for both callers.
Set is CLIENTS-only and replaces the complete map. No old top-level Settings
service, forwarding adapter or request/response twin survives.

The four public SDK barrels continue to hide wire packages. Shared values use the
root barrel; generated client and plugin resources keep their existing call style:
request content as arguments, owned transport context, and a single response field
returned as the payload. Multi-field replies retain their complete response type.
Python keeps wire modules under `_wire/model`, `_wire/host` and `_wire/plugin`.
Rust protocol modules are `model`, `host` and `plugin`; SDK wire modules remain crate-private. ALL payloads are exported
through the Rust/Python and C++ shared roots, and through
C# `Harmos.Clients` (the context uses that same public payload type).

Repository checks reject wire packages and version namespaces in public C++ headers,
Python modules, stubs and exports, Rust wire module declarations and SDK guides.
`source/protocol.py` owns code surfaces and export inventories; CI documentation
checking in `ci/docs.py` owns SDK guides and local documentation links.
The allow-list is internal generated directories (`detail`, `_generated`, `_wire`),
canonical gRPC method paths, and protocol documentation that maps packages (`proto/**/*.md`
and this architecture guide). Markdown link destinations may name protocol source files.
Generated Python export inventories also reject duplicate public homes.

Reflection remains optional and off by default; `Operations` is supported discovery.
When reflection is enabled, it merges the reachable message graphs from operation
contracts before registering each served service once. Shared descriptor files are
partial graphs, not conflicting complete registrations. Python generators relocate
imports and message module metadata without changing canonical gRPC service identities.

Wrong-caller admission uses one failure constructor in `harmos-operations/src/failure.rs`:
`REFUSED`, code `-32604`, action `Call`, the refused operation, and a message naming both.
Native gRPC, child replies, samples, guest imports and nested invocation failures
preserve that record; the gRPC status is PermissionDenied, including after nesting.

### Product Wrappers and the Native Facade

**Locked direction, proposed implementation (`operation-boundary`, 2026-11-16):**
replaces "The product-owned `protocol/composition.toml` is the single declarative
composition input for both runtime assembly and SDK generation." Its purpose was
matching caller admission and generated resources. Product wrapper descriptors
will own remote exposure when the host-wire cutover lands. The composition reader, caller override map and generator `--composition` input
are removed. Generators merge descriptor artifacts directly and project the declared
audiences. Schematic and the four language fixtures use those same declarations.
Host wire and SDK root removal remain pending in the breaking group.

```rust,ignore
let reply = runtime.for_call(&ctx).storage().append_samples(request).await?;
let plugin = runtime.for_call(&ctx).plugins().load(PluginSource::Process(process)).await?;
```

Every typed facade method dispatches its registered native operation under that
operation's policy. The builder accepts storage backends, object catalog and settings,
owns every manager and can assemble storage alone for bootstrap and offline work.
No manager, manager getter, `Deref`, engine, query or reader is reachable through the
product facade or its operation results. Backend-authoring traits and their full DTO,
catalog and codec closure remain public through one authoring module.

The storage operation result boundary is implemented in
`crates/harmos-operations/src/storage/native/requests.rs`: operation-owned selectors,
definitions, pages, statuses and IPC payloads replace query-engine response types.
Internal seek cursors remain behind opaque continuation tokens. The builder and
typed facade migration remains tracked in the open tasks.

A product exposes remote behavior with its own wrapper operations and declared
CLIENTS, PLUGINS or ALL audience. SDK generators project those descriptors. The four
plugin SDKs, protocol generation, Wasm guests and conformance remain; Harmos host-client
resource roots disappear. Shared host messages remain available as wrapper payloads.
Plugin instance/invocation/window/sample/child frames stay on the wire, with plugin
context facilities for time, own settings, own invocation Get/Cancel/Watch and
admitted discovery. Discovery applies the dispatch admission predicate.

The source still exposes the older managers and host services until the breaking
groups close; those are migration inputs, not the agreed product contract. See
`openspec/changes/operation-boundary/tasks.md` for completed groups and usable pins.

### Streaming Field Expressions

Scale, Offset and Contains are domain query declarations executed in `harmos-query`
through native `storage.list_rows`. Scale and Offset derive Float64 columns; Contains
supports strings and scalar lists. The streaming prefix preserves row order and parent
nulls without spooling the source. Field-path null handling and typed literal matching
have one owner shared with filters, so containment does not define a second comparison
or missing-value policy. Products select output names and query/page bounds.

### Storage-Only Assembly

The builder's `storage_only()` borrows assembly until its storage session is released.
It constructs an independent operation dispatcher and storage worker over the registered
backends, with no application services, listeners or plugin lifecycle. Closing it settles
operation admission, drains accepted storage work and closes both primary and recovery
sessions. Full startup constructs a fresh manager over those same backend handles and
reopens them. There is no worker ownership handoff or retained live-session upgrade. The builder
retains a retryable cleanup receipt after cancellation or failure and refuses another
opening or full startup until it settles.
Primary-backend copy uses a retained source cut and a backend-owned empty offline target.
Snapshot copy holds the storage lifecycle read guard and uses the backend I/O policy.
Disposal is an explicit destructive operation on an offline target. Recovery copy remains
with the recovery-backend owner.

### Native Type Ownership

Native revision hashing retains the protocol domain separators `harmos.stream.v1\0`
for streams and `harmos.record.v1\0` for records. Their remaining inputs and byte
encoding are described in the product guide; public SDK symbols do not carry these versions.

Storage remains Private and in-process. Products own typed file, upload, query,
record and object wire operations over those native calls. Model FileReference,
File, Upload, PageRequest and Receipt remain exchanged values, preserving their
field numbers and encodings. Plugins.Create resolves managed-bundle FileReference
through the existing in-process content owner. No storage wire API, new backend,
feature gate or alternate storage owner is introduced here.

`harmos-operations/src/storage` owns one Private in-process declaration and native
request/result pair per storage capability. Domain storage values remain in
`harmos-domain/src/model/storage`; engine query, column and index values live in
`harmos-query`, while manager projection and backend values live in `harmos-storage`. Native mutation, cursor and revision
encodings preserve persisted data and original outcomes. Neither boundary exposes
storage methods or an object registry. Product handlers call the native operation
through `ExecutionContext::call_native` with their own policy and typed wire values.
Native record and stream callbacks retain their declared message codec; public
encoded calls and SDK lifecycle envelopes use protobuf independently of that codec.

### Application Operations in Discovery

The existing registry composes application and library operations; it needs no
second discovery inventory. Register each declaration once, using its executor
identity and effective callers. Discovery returns admitted contracts with their
fingerprints and projections. Dynamic client calls use Invocations.Create with
operation, fingerprint and payload; application typed clients come from the
application's descriptors and their declared audiences.

Plugin discovery uses the same admission predicate as child dispatch, including
declared requirements, matching fingerprints, provider visibility and audience.
Forwarding List or Get through a product wrapper preserves that plugin identity;
undeclared or unauthorized operations are absent from List and unavailable to Get.

### Consumer Work Lists

**Locked replacement, proposed (`operation-boundary`):** core crates close storage
coverage, consolidate statistics and own native declarations and the typed facade;
all four SDKs move context calls to plugin facilities and delete host-client roots;
generators project product wrappers; conformance owns a wrapper fixture; Schematic,
stories and benchmarks migrate together. The table below records the preceding
executor-package migration, not the new delivery's completion.

The consumer migration is implemented across these owners. Regenerate bindings
from their source definitions; generated files have no independent ownership.

| Owner | Landed Consumer Work |
| --- | --- |
| Core Crates | Model/host/plugin imports, one host declaration and handler per operation, caller admission on direct/encoded/child paths, descriptor-declared audiences, trusted settings selectors and effective callers in discovery. Storage remains Private and in-process. |
| Rust SDK | Public barrels, flattened client roots, shared plugin context resources, product fixture bindings and host method identities. The SDK client example is dynamic; the Schematic client is generated from application descriptors. |
| Python SDK | Typed public payloads, generated client/context resources and internal executor wire modules, migrated example and fixture imports. The packaged finite-schema projector validates the mandatory callers field in canonical contracts. |
| C# SDK | Public payload namespaces, shared client/context reply types, generated roots and fixture bindings, caller-owned call context. |
| C++ SDK | Public payload barrel, generated client/context methods, native protocol inventory, conformance fixture barrel, installed-package consumers. |
| Conformance | Both wrong-caller directions use the real runtime, including direct and managed invocation admission; all 38 rules apply to each SDK fixture. |
| Schematic and Fixtures | Executor packages, explicit host callers, product descriptors, typed application client, in-process storage wrappers, native sidecar and wasm32 guest. Host handlers register their canonical service/method identities. Guest publication declares its own PLUGINS-only host service. Guest admission validates protobuf identities through their checked contracts while native declarations retain their native identity rules. |

The narrowing-only stories in `crates/harmos-runtime/tests/plugin_grpc/application_faces.rs`
and `tools/python/tests/integration/test_protocol_callers.py` are removed. The runtime file
retains the scoped discovery wrapper proof, matching discovery to dispatch. Private host
wire reachability and plugin context facilities remain open in the breaking group.

Invocation selectors retain the submitting caller's ownership. A plugin cannot watch
an invocation submitted by the host or another caller. A watch demand already in
transit when its child ends does not replace the terminal response or refusal;
unknown active watch identities and duplicate outstanding demand remain protocol
errors on both invocation and sample carriers.

### Next Pin for Products That Vendor the Protocol

Replace the entire proto/harmos tree and regenerate all languages. Remove client/
and plugin/host/ folders and old shared v1 files. Apply the complete identity map,
mandatory declared callers before enabling listeners. Products
own storage wrappers and application-specific publication; no library Storage or
Samples.Append service is recreated. Products expose their own wrapper services;
library host-wire and host-client removal remain open in the breaking group.

## Managed Process Ownership

`crates/harmos-runtime/src/plugin/grpc/managed.rs` owns a native child immediately
after spawn, before readiness. Startup has a retained task and runtime stop
participant. The spawn is never raced against cancellation, so cancelling the
caller or a manager worker signals that owner once the child exists, and the owner
awaits termination and exit. Readiness failures, connection failures and staged
replacement rollback settle the same child.

Reaping never waits on a child the host has not signalled. Every ending first
settles its bindings, then releases the plugin in phases that each get their own
`PlacementSettings::stop_grace`, measured when the phase starts:

1. Settle: bindings and publications drain, or are cancelled at the deadline.
2. Stop: the host sends Stop and awaits the requested exit, so the plugin is always
   told to stop before it is killed, even when settling spent its whole grace.
3. Kill: a child still running is killed and reaped within a fresh grace measured
   from the kill, so a slow drain cannot turn into a reap failure.

A disconnected child cannot receive Stop, so it is killed as soon as its bindings
settle; a child that exited is only reported. Teardown therefore ends within
`2 × stop_grace` for a remote plugin and within `3 × stop_grace` for a managed one.

Every managed child is forked from one process-lifetime spawner thread
(`managed/spawner.rs`) on every platform. A request whose caller is gone is dropped
unforked. A child forked for a caller that left meanwhile is handed to its process
owner, which kills it and reaps it under the runtime stop participant. A panic
answers only the request that raised it, with an error naming the start; the thread
keeps serving. Should the thread end anyway, the next spawn starts a new one.

The host's death ends its managed children on Linux: the spawn sets
`PR_SET_PDEATHSIG` to `SIGKILL` and rechecks the parent to close the fork race.
Linux delivers that signal when the forking thread exits, which is why the spawner
thread lives for the whole process rather than on a runtime worker that may retire.
If the spawner thread ended anyway, Linux kills the children it forked and their
attachments report the exit. macOS is not covered. On Windows the hosting product
owns process-tree lifetime, for example with a job object that kills on close.

Manager Delete and installation reuse follow successful settlement. Candidate
bundles are kept out of synchronous temporary-directory cleanup once launch is
possible; rollback explicitly reaps before removing them. An expired exit bound
reports the action, executable and process identifier, retains the installation
and refuses reuse. A retained native reaper keeps ownership after cancellation or
failed settlement; runtime participants propagate the failure through shutdown.
Archive extraction itself launches no process. Upload production publishes bytes
through storage, and installation reads immutable content into a separate extracted
directory. Guests compile owned component bytes in-process; there is no on-disk
guest module cache or native child holding the uploaded module.

## Guest Entries and Reported Declarations

The durable-entry description below is landed behavior to preserve at the product
owner. Under Plugin Boundary the product owns guest entries/settings and explicitly
loads selected modules; Harmos retains guest admission, initialization and fencing.

Every host-registered WebAssembly plugin has a durable `PluginEntry` keyed by its
registered identity with `PluginPlacement { guest: PluginGuest {} }`. Configure and
start the plugin entry manager before `PluginHost.register_guest`. Registration
materializes typed settings before guest initialization; the host
`plugins.Settings.List` uses the admitted guest generation's owner; name is optional
and may only equal that owner when supplied.
Client `Plugins.List/Get` and `Plugins.Settings.List/Set` use that same entry.
Deleting it removes settings in the same registry commit and retires the guest.
The product supplies and re-registers its module after host restart; guest placement
is a marker, not a second module store. `Plugins.Create` refuses guest placement
and directs the host to register the loaded module in-process. `Plugins.Replace`
remains managed-only; native guest reload keeps its existing entry and settings.

`Instance.Get`, `GuestRegistration` and `PluginDescription` report `Declarations`
through `declarations`; `PluginEntry.declarations_digest` identifies those bytes.
There is no Catalog alias. Fields retain their numbers. The four SDK builders use
`declarations()` (`Declarations()` in C#). Internal operation, object, state and
packaging registries retain catalog terminology; they do not name this wire payload.
Consumers must regenerate bindings, rename the reported message/fields and SDK
builder calls, and configure the entry manager before registering guest modules.

## SDK Call Contracts

The manager/getter examples below describe landed source. `operation-boundary`
replaces those public paths with typed facade calls and opaque handles; Plugin
Boundary retains their scope, clock, sampling and settlement guarantees.

### Scoped Plugin Bindings

A product can bind an already admitted gRPC activation to a trusted scope and its
own clock for the lifetime of a scoped unit of work:

```rust,ignore
let activation = plugins.activation("plugins/example").await?;
let binding = runtime.bind_grpc(&activation, "unit/opaque-id", unit_clock).await?;
binding.operations().call_encoded(initialize, payload).await?; // the plugin registers the unit
binding.start_samples(["example.reading"]).await?;
// ... the unit runs; the host grants its streams windows on the unit clock ...
binding.stop_samples().await?;
binding.operations().call_encoded(cleanup, payload).await?; // the plugin ends the unit
binding.close().await?;
```

`Runtime::bind_grpc` returns `plugin::grpc::ScopedBinding`. The supplied activation
comes from `plugin::manager::Manager::activation` for a ready managed entry, or
`Attached::activation` for a directly attached plugin. Manager lookup refuses
unavailable entries and never restores an old activation after disable or restart.
The activation must belong to that runtime and still be admitted; duplicate live bindings for the
same activation and scope refuse. The product owns the scope's meaning, time
advancement and admission policy. Scope text is selection context, never authority.
Binding does not change declarations, endpoints, the plugin process or its single
physical connection, and it opens no Samples demand: calls are bound at once,
sample demand is controlled separately. `start_samples(streams)` opens one Samples
demand for exactly each requested stream with the existing `SamplesSelection.scope`
and retains the activation's admitted stream writers; a stream the plugin does not
declare is refused, and the activation's other streams stay under its unscoped
demand. Opening is one plugin load, bounded by the `plugins.load` operation's deadline on the runtime clock. `stop_samples()` settles only that demand
within `stop_grace`: an in-flight window is cancelled and publishes nothing,
accepted batches are drained and committed, and the demand's Samples responses
end. Calls stay bound. The host grants a scope windows only between start and stop,
so a product starts demand after its plugin has registered the unit and stops it
before the plugin's cleanup ends the unit; no window reaches the plugin outside that
interval. Demand may start again after a stop. `close()` settles any demand still
open together with the unit's calls. A close or attachment settlement that begins
while `stop_samples()` is settling ends that stop with an error naming `close()` and
keeps its deadline, so close still returns within one `stop_grace`. Start and stop
requests that arrive while the binding settles are refused at once, and a start
whose caller stopped waiting does not open demand. The unit's calls are cancelled
on every ending, including an attachment that aborts the binding at its deadline.

The binding's operation handle carries a local clock resource through descendants,
including plugin child calls to Time.Get and Time.Sleep. Portable context cannot
replace it. Callee policy deadlines continue to use the runtime clock. Scoped
invocation activity and each accepted row's ingestion activity keep the unit clock
from advancing through outstanding work. Each demand validates its own wire
sequence; the shared stream writer assigns its publication sequence atomically.
Rows retain the trusted scope in `PublicationRow.context` as the JSON `scope`
property, alongside device source time when present. Consumers use that property
to select the unit's rows and advance only its clock after awaited ingestion.

Clock waits register at admission. An operation that declares the wait its request
performs (`admitted_wait` on `ProtobufOperation` and `NativeOperation`; Time.Sleep
does) registers that timer in `admit_entry` through `NativeInstance::admit`, before
operation middleware observes the call. Every path admits there: encoded, protobuf,
native and typed calls and plugin children. Inside a scoped unit each call receives
its own unit clock, so middleware may announce the sleep and a product may advance
the unit clock immediately; the handler awaits that registered timer. The prepared
call owns it, so refusal, cancellation and binding closure drop it even when
middleware has not let the handler run. A retried attempt registers its own timer.
Unscoped calls retain their runtime-clock behavior.

```rust,ignore
let binding = runtime.bind_grpc(&activation, scope, unit_clock).await?;
// After ingestion and the middleware's Time.Sleep notification:
unit_time.advance_by(sample_interval).await;
binding.close().await?;
```

A scoped sleep settles after the samples it spans. A clock wait of a scoped call
that ends at instant `t` (Time.Sleep or an operation's admitted wait) settles only
after every sample of the same unit due at or before `t` has been granted in a
window, answered and committed, on every clock kind. On reaching `t` the wait asks
each demand of the unit to grant through `t`; a demand whose window end lies later
wakes early and grants the window up to its current reading. Samples sit on a
fixed grid from the demand's origin, so a sleep of N periods spans exactly N
samples on a controlled clock. A plugin that sleeps and then reads its observer
state sees every sample due by the sleep's end. Waits inside a window's own call
tree are exempt, since that window cannot be answered before them. A window whose
handler waits on something the sleeping call holds therefore holds that sleep until
the call's own policy deadline. Each scoped interval demand records its next
ungranted sample and the latest requested cut in the unit (`operations/clock.rs`).
`plugin_grpc::scoped_sleep::a_sleep_spanning_1024_samples_observes_1024` and its
1,023 and 1,025 siblings read the count after one sleep, over 100-sample windows
whose last one the sleep cuts short, with no wait in the plugin.

### Interval Windows

For an interval stream the host grants `harmos.plugin.v1.Window` frames and the
plugin answers each with `SampleBatch` parts whose columns hold one value per
sample ([behaviour rules 10 and 35](../proto/harmos/plugin/v1/behaviour.md)).
Sample `i` of a demand is due at its origin plus `i` periods; a window covers the
samples due since the previous window and ends on the `origin + k * window` grid at
or after the next sample, extended to the clock's reading when the wake is late or
the controlled clock advanced further. A controlled step that reaches a window end
or ends a scoped wait is covered by exactly one window; a shorter step grants nothing
until a later step reaches the next window end. The window length is
`PlacementSettings::interval_window`, or the plugin Duration setting named by the
stream's `window_setting`. Each part becomes one stored row of the derived
`harmos.batch.<row>` message (`<row>` is the row's full name with dots as underscores) (`harmos_protocol::columns::derive`): the first
sample's Unix nanoseconds, the period, the count and the row's fields as repeated
columns. The host validates each part's offset, count, last flag and column lengths,
admits it once and waits for ingestion once per window; it does no per-sample
admission, sequencing, window entry or ingestion wait, and converts the columns to
the stored encoding in bulk.
A part holding more structural values than one stored row may
(`harmos_domain::message::postcard::MAX_VALUES`) is stored as consecutive rows.
Rows with repeated, map or oneof fields cannot be interval rows. While any scope of
an activation samples a stream, the activation's unscoped demand grants that stream
no windows; it resumes from a fresh origin when the last such scope stops, so each
stream has one live schedule per activation and scope. Streams no scope requested
keep their unscoped schedule. The unscoped demand also grants windows only
while the stream's demand count (`PublishedStream::demand()`) is positive, that is
while a `watch()` handle or a plugin listener holds it,
and resumes from a fresh origin when a watcher returns; idle streams cost no
sampling. `plugin_grpc::interval_windows` proves exact
boundaries and counts on a controlled clock, per-stream window lengths, splitting at
the plugin's maximum message size, the paused unscoped schedule of a requested stream
and the running one of a stream the scope did not request.

A demand that cannot keep up fails fast instead of dropping samples or growing its
windows without bound. A grant whose clock reading lies more than
`PlacementSettings::interval_lag` window lengths past the window's end fails with
`Error::Lagging`; a window whose smallest stored footprint exceeds the empty shared
publication pool is refused before the plugin samples it (`Error::WindowExceedsPool`);
a part that finds the pool full fails with `Error::PoolFull`; a window the plugin
fails with a gRPC status fails with `Error::WindowFailed`, carrying the plugin's status
and message. Each names the stream, the limit or cause and the action. An unscoped failure ends only that stream's demand and is
reported on `Attached::status().failure` while the activation stays attached; a scoped
failure is the result of `stop_samples()` or `close()`. `plugin_grpc::window_overload`
proves the lag limit, the full and the too-small pool, the plugin's failed window and
the watched gate. The pool
reclaims a row only after every feed has taken it and every storage consumer has
drained it (`streams/ingest.rs`), so a lagging storage consumer fails the demand
rather than losing rows it never saw.

Concurrent and nested waits inside one operation share its activity without a
lock; the activity parks only while all of its waits park, so joined waits never
serialize or stall virtual time.

Each binding is one state machine the attachment owns from `bind_grpc` until it
settles; no binding task outlives its attachment.

| State | Entered when | Leaves when |
| ----- | ------------ | ----------- |
| Serving | `bind_grpc` admits the scope under the attachment's bindings lock and returns the handle | `start_samples(streams)`, `close()`, the handle is dropped, or the attachment settles |
| Sampling | `start_samples(streams)` opened the requested streams' demands | `stop_samples()` settles them back to Serving, `close()`, the handle is dropped, or the attachment settles |
| Settling | Close was requested | The steps below finish or their deadline passes |
| Settled | The outcome is published | `close()` returns it |

Settling signals close to every demand, then cancels the unit's tracked calls, so
close always wins over a tick it cancels. It then drains each demand: accepted
ingestion and the plugin's end of its Samples response. A drain that misses the
deadline cancels the remaining work and reports `Error::Deadline` naming the
action. The attachment settles itself through the same sequence on every ending,
whether stop, plugin exit, disconnect or runtime shutdown. It closes scoped
admission first, so a later `bind_grpc` is refused and must bind to the next
activation. It then joins its bindings, revokes the activation's stream leases and
withdraws its operations. Only then does it send Stop, kill and reap a managed
process, or start reconnecting a remote one.

The deadline is `PlacementSettings::stop_grace` on the runtime clock, measured
when settling starts. The unit clock never bounds it. Cancelled calls settle
alongside the drain, so each has the whole grace. `ScopedBinding::close` returns
within that bound. On `Error::Deadline { action: "Close Scoped Plugin" }`
the demands were cancelled; rows accepted before the deadline remain delivered.
The plugin did not end its Samples response after the host half-closed it, which
the SDK conformance suite requires.

The bound holds on every clock kind. Teardown parks its runtime-clock activity
while it waits on the plugin, and a demand parks its activity while it awaits the
plugin's end of its Samples response, so a virtual runtime clock reaches the
deadline once its controller advances. Accepted host ingestion holds a
runtime-clock activity until it completes, so virtual time never expires host work
in progress. A controller that advances while a stop, close or disconnect waits on
the plugin may therefore expire that wait even for a plugin that would answer;
advance past `stop_grace` only to exercise the deadline. A cancelled call that is
still running at the deadline remains owned by its operation entry, whose own
policy and settlement govern it.

Connection loss or plugin exit still awaits accepted scoped ingestion before the
binding reports the transport failure. A settlement failure is logged with its
action on every ending, returned by `stop()` after a Stop request and kept in a
remote attachment's disconnected status. Remote attachments continue reconnecting
under the placement policy. Each reconnect attempt, including opening its
publications, is one `plugins.load` under the configured `GrpcPluginSettings::policy.load`, and stop
interrupts every step. An attempt
that cannot select the retained settings, admit its operations or open its
publications closes its connection, reports the failure in the status and retries
after `reconnect_delay`. Leases belong to their activation, so a cleanup that
missed its deadline never affects the next one.
Retained operation handles refuse new work after closure. Closing one binding
leaves the other scopes active; the unscoped publication resumes once no scope of the
activation samples.

The fixture story `plugin_grpc::scoped_publications` starts two concurrent calls
and demands after attachment, holds one ingestion boundary, closes that scope,
then settles only the other scope's sleep by advancing its clock. It checks
scope-stamped rows and proves the original unscoped demand operates again once both scopes have closed.
`plugin_grpc::binding_lifecycle` drives each ending with a live binding: a drain
outliving `stop_grace`, a plugin that ignores half-close on the manual and on a
virtual runtime clock, Stop after a spent drain, plugin exit, managed disconnect,
runtime shutdown, both orders of a bind and stop at the bindings lock, a bind
during reconnect, a remote cleanup that misses its deadline then reconnects, and
sample demand that grants the plugin windows only between `start_samples(streams)` and
`stop_samples()` while the binding stays bound.

Rust client retained admission bounds each generated oneof by its largest variant,
with presence/alignment and boxed recursive edges, and scans the actual wire values
for nested allocations and repeated capacity. Inline nested layouts are counted
once. A representative reply fixture covers a plugin entry, a document, sample
pages and plot snapshots with a 4 MiB consumer budget and refuses an oversized
reply before decode. Python, C# and C++ clients use their native protobuf transport
decoders and do not share this Rust descriptor-layout estimator.

Plugin peer admission compares the configured transfer chunk plus framing with
the peer's advertised bound. The transport message ceiling remains independent.
Encoded Invocations.Create forwarding preserves an explicit native gRPC status,
including its action, details and cause; conformance's Client Caller Refusal rule
also compares direct and encoded Unimplemented failures.

All four typed client generators flatten request fields in declaration order. Required
fields are positional; definition-level optional fields and retry identity are optional.
Python uses keyword-only arguments; C# uses optional parameters. Rust exposes a plain
method and a `_with` method taking one generated options value; C++ defaults that options
value. A missing request identity is generated once per call. Explicit identities are
preserved for retry safety. Request messages belong only to private wire code; generic
operation dispatch remains message-based for application operations.

C++ resource constructors and call parameters avoid member names and local
identifiers. The renderer adds a value suffix until each parameter is unambiguous,
including nested resources whose names already end in underscores, without
introducing reserved double underscores.

Calls create their transport context. Options permit metadata, cancellation and deadline
overrides. No SDK invents a default deadline; operation policy owns it. Plugin handlers
use `Context.host` (`host()` in Rust/C++) for typed Time, Plugins.Settings, Operations and
Invocations calls, declaring their requirements with `HostOperation`.

Rust `CallOptions` implements `Clone` and `Debug`; a clone retains metadata and the
explicit timeout override for forwarding through generated clients. Python passes
metadata and timeout values as keyword arguments, C# uses the copyable gRPC
`CallOptions` struct, and C++ generated options are copyable values sharing their
caller-owned `ClientContext`; the C# and C++ copies retain the same cancellation owner.
Rust client and native protobuf session futures support `Send` for spawned tasks.
Plugin handler futures run on the SDK's local executor and may use non-Send local state.
Encoded settlement owns a cloned instance control handle, so it does not borrow
the invocation's boxed response future across suspension.

A response with exactly one declared field returns that field, including a repeated
collection. Streaming calls project every item by the same rule. Missing singular message
payloads refuse the read. Empty and multiple-field responses retain their response type:

- Client: Access.Listener.Get/Set; Access.Pairings.Create/List/Delete; Plugins.Get/List/
  Watch/Create/Set/Replace; Plugins.Settings.List/Set; Status.Get/Watch; Status.Diagnostics.Get;
  Status.Logs.List/Watch; Status.TraceEvents.List/Watch; Time.Sleep.
- Plugin context: Plugins.Settings.List and Time.Sleep.

Status.Get and Status.Watch retain both status and resolved read-cut metadata. The direct
HostStatus return shown in the requested example requires an owner decision about that
metadata; the single-field rule currently preserves it. Optional read cuts and pages are
marked in the protocol definitions rather than inferred by parameter name. The optional
Plugins.Watch name selects all entries when absent.

Product generators merge descriptors, keep their own protocol modules private and export
response types through a client barrel. The shared descriptor fixture proves a product
Annotations resource beneath library Plugins alongside the sibling Jobs.Progress root.
Python composed roots inherit Client connection ownership and preserve typed resource
properties. No application-side resource grafting is required.
