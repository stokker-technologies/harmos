# Storage Interfaces

`harmos-storage` owns backend contracts, manager admission, saved scans, index and
projection catalogs and atomic conditional objects. `harmos-query` owns Arrow
execution; `harmos-domain` owns selections and query contracts. The library ships
one process-lifetime in-memory backend. Products own durable backends. Consumers choose capacity, concurrency,
physical chunk sizes, work budgets and response limits. Storage does not expose
product keys or a database connection.

## Native Schema Mirrors

`Storage.Info`, `Storage.GetSchema`, `Storage.SetSchema` and `Storage.DeleteFile`
are Protected native hooks with service/method request-response wrappers and no
gRPC projection. GetSchema resolves optional `at` through the same issued-cut
owner and returns the whole schema, existence and an opaque revision. SetSchema
requires that revision or explicit create-if-absent, validates the complete graph,
and compares atomically in the existing schema-write hook. An existing schema
key's graph is immutable; a changed graph requires a new version. Even an unchanged
value with a stale revision conflicts and names SetSchema with get/retry guidance.

The provider commits the original applied/conflict outcome and mutation receipt
in the same synchronous transaction. Ordered admission owns it after caller loss;
replay after later revisions or restart preserves that outcome. Successful
receipts are durable, and stored schema encodings are unchanged. DeleteFile keeps
its existing retained-reader behavior; this migration makes no reclamation change.

## Operational Policy and Admission

`StorageSettings.policy: StoragePolicy` owns backend I/O/recovery deadlines, the
shared `RetryPolicy`, abandoned-production lifetime and mutation-receipt retention.
Its defaults preserve the previous policy: 30-second I/O, 300-second recovery,
four total transient attempts with exponential 10 ms–1 s backoff, 60-second
productions and 300-second receipt retention. A call-local operation override
does not change that configuration or subsequent calls.

Consumers explicitly provide `request.max_file_productions` and
`request.max_mutation_receipts` alongside pending-write, query and response limits.
Retiring a receipt rotates its issuance window; an expired request is refused
rather than admitted again. Admitted work remains owned until settlement.

## Backend Registration

The built-in adapter requires no directory or file:

```rust,ignore
let storage = harmos_runtime::storage::StorageManager::in_memory(catalog, settings)?;
setup.settings().storage(&storage);
```

To register a product backend, implement all required methods in
`harmos_storage::StorageBackend` and `harmos_storage::state::StateBackend` and
compose their shared instances:

```rust,ignore
let backends = StorageBackends { primary: primary.clone(), state: recovery.clone() };
let storage = StorageManager::with_catalog(catalog, backends, settings)?;
setup.settings().storage(&storage);
```

Primary and recovery sessions are separate owners; there is no transaction across
them. The product chooses persistence format, durability, backup, migration and
exclusive root ownership. It must preserve saved cuts, replay identities, atomic
conditional outcomes and the capability contracts below. A backend must refuse
corrupt or incompatible data without resetting it. Register the object catalog
used by both the primary adapter and manager.

`memory::Backend` implements all 24 required primary methods and 24 optional
methods. Its capabilities report conditional objects, projection publication,
file sessions/deletion and mutation receipts. Native Arrow and native indexes
report false; shared bounded execution provides those reads. Immutable record
views and selected-record rank seeks are implemented. The four remaining optional
methods retain their declared defaults: `get_work` returns zero counters,
`advance_projection` returns `None` to select shared execution, and direct native
columnar/index hooks refuse as unsupported. `memory::RecoveryBackend` implements
all eight recovery methods, including snapshot integrity, singleton publication,
bounded replay and saved-frontier flush. Snapshot bytes use `Arc<[u8]>` rather
than a mandatory file.

Keep both adapter instances to reopen their process-lifetime state. Dropping them
loses their data; they provide no process-restart durability. The
`durable_projections` capability names the publication contract within the
backend's lifetime, not a disk guarantee. Budgets, policy and clock remain
caller-owned. Runtime feature isolation is still pending; the locked off-by-default
storage feature is not yet implemented in this tree.

## Saved Reads and Capabilities

```rust,ignore
let capabilities = storage.capabilities();
let snapshot = storage.snapshot(&saved_cursor).await?;
let saved = snapshot.at();
let page = snapshot.list_column_rows(&read, &cancel).await?;
let ordered = snapshot.list_index_entries(&index_read, &cancel).await?;
```

A manager snapshot captures a saved admission cut, not pending writes. It retains
its backend token until the existing worker release path processes its drop.
Tokens bind an owner, store history and exact cut; release, close or reopen makes
that token unusable. Later writes do not alter the retained answer. Readers must
release snapshots before expecting retention to reclaim their rows.

`StorageCapabilities` advertises `native_arrow`, `native_index`,
`durable_projections`, `conditional_objects`, `file_sessions`, `file_deletion` and
`mutation_receipts`. `list_column_rows` and `list_index_entries`
choose native execution or the common bounded executor. A backend without native
Arrow scans supplies typed row reads; it still receives the same predicates and
saved-cut semantics. A backend without a native index stores its declarations and
uses bounded source scanning. It refuses insufficient work rather than returning
an incomplete globally sorted answer. Durable projections require an atomic
publication primitive and immutable `get_projection_definition(name, Some(revision))` lookup
for source authorization of retained revisions; conditional objects require an atomic compare/exchange
primitive. Defaults for unavailable primitives return `Unsupported`, never pretend
an empty result or successful write. Independent read-then-write calls cannot
implement compare/exchange safely.

`ArrowScan` selects a `ColumnSource` (objects, records or stream plus `SchemaKey`),
unique dotted leaf column names, optional numeric predicate and exact nanosecond
`TimeBounds`. Bounds are lower-inclusive and upper-exclusive. Its initial `start`
is an exclusive source position. Output is ordered by source position ascending;
objects select the latest retained version at that cut. `limits` bounds returned
rows/array bytes; `work` bounds source rows and physical decoding bytes per call.
A native page may need a whole physical chunk: choose work limits at least as
large as the backend's chunk settings. A single oversized chunk or row refuses.
An empty page with a continuation is valid after pruning or filtering. Continue
until `complete`; never treat an empty page as end-of-source.

`ArrowSeek` binds the saved token, plan digest and last examined source position.
Change the plan by starting a new read. A native backend may fetch only required position, projection and predicate/time
columns, pruning before column loading. The memory adapter uses shared execution. `ArrowPage` reports source rows scanned, encoded column bytes read,
columns fetched and payload rows reconstructed for measurement. These counters
are evidence, not admission policy or timing assertions.

For a record query containing only projections, `StorageBackend::get_record_position`
may locate the exclusive position after the requested number of selected records.
It binds the same snapshot and selection; an exhausted selection returns the saved
cut, and `None` keeps sequential execution. The query executor then decodes only
the requested page. Backends own any ordinal index. Streaming filters and global
reductions retain their complete input semantics. The deterministic record backend
retains shared immutable images and per-schema ranks/encoded sizes, so opening a
snapshot and reading an unfiltered page do not copy or rescan its prior history.

Backends can also implement `read_record_views(snapshot, selection, page)` and
return `Some(Vec<RecordView>)` instead of copying owned record payloads per read.
Construct each `RecordView` once with `RecordView::new(record)?`; clones share the
immutable row, and `encoded_bytes()` retains its exact checked serialized size.
The snapshot reader still validates history, position order, cut, selection and
row/byte ceilings. Returning `None` keeps the existing `list_records` path.
Payload validation and decoding still run on selected rows. No backend-provided
unchecked byte count can bypass the page bound.

`Cancellation` is cooperative across source reads, decoding, sorting and bucket
work. Cancellation does not roll back an already committed write. CPU bucket work
runs on the blocking executor; dropping its awaiting future cancels its child
without cancelling the caller's shared token.

## Unary Filesystem Metadata

`GetFileHash`, `CaptureFile` and `ListDirectoryEntries` have separate client and
context wrappers and shared handlers. Stored hashes validate an exact durable
reference and digest. Local hashes retain their result metadata at the returned
issued cut; repeating that cut returns the original result after the file changes.
`GetFileProduction` similarly retains progress, writer generation and distinct
accepted/durable byte lengths at its resolved cut.

Directory enumeration retains bounded, path-ordered metadata for continuation.
Pages obey complete response-frame limits; changing the selection refuses the
token. This is a retained enumeration, not an atomic filesystem snapshot. An
explicit read lease pins the same image and its continuations through automatic
cache eviction. Releasing it, expiry or restart refuses unavailable observations
and tokens, even when the underlying durable store position is unchanged.
Only metadata is retained; accepted storage payloads are never snapshotted.

Capture preserves native consistency checks and returns its completed durable
publication receipt. It has unknown replay safety and no caller mutation ID;
that receipt does not add a replay-key promise to capture.

## File and Directory Observation

`WatchFileBytes`, `WatchFileProduction` and `WatchDirectoryEntries` have independent
client/context declarations and shared native handlers. Immutable stored content
must be durable before observation; local-file events retain their live
Beginning/End/At-offset, replacement and truncation semantics. A completed finite
file watch emits its typed byte-count completion. These live external observations
do not label filesystem data as a durable primary-store history.

Production watches read retained prefixes, then emit terminal `FileProgress`.
Cancelling observation neither publishes nor abandons its producer. Directory
watching subscribes before enumeration, preserves Existing/InitialScanComplete
and RescanRequired events, and shares native selection rules. File payload sizing
includes every enclosing protobuf length and offset; all complete response frames
respect the consumer byte ceiling. Delivery retains its callee-owned deadline.

## Durable Row Observation

Client and context `WatchRows` share the `ListRows` source selector for records,
samples and object revisions. Omit `at` for the current durable boundary, or pass
an issued retained cut. The first event is that boundary; historical initial state
belongs to `ListRows`. Every later record, sample or object revision is exclusive
of the opening cut and follows storage-position order. Samples retain their own
sequence independently of the storage position. Accepted writes remain invisible
until their synchronous commit has been acknowledged.

Progress events advance through committed data rejected by the selector. Events
include issued `resolved_at` cuts, so reconnection can resume exclusively while
that cut remains available. Subscription retention starts before the boundary is
sent; a concurrent opening commit is included in the tail. If bounded retention
retires a slow observer, the stream emits `resync_required` and ends. List a fresh
image and reopen from its returned cut; never manufacture an earlier sequence.
Native `follow`/`next` still observes accepted entries separately;
`Follower::next_durable` observes committed entries and filtered progress with the
same bounded retention and gap mechanism. Closing/restarting retires watchers.

## Ordered Indexes

```rust,ignore
storage.set_index_definition(&definition, expected_revision, rebuild, work, &cancel).await?;
```

An `IndexDefinition` owns a name, nonzero semantic version, source and ordered
columns. Set requires the revision returned by Get; `None` explicitly creates an absent definition.
The provider compares the expected revision atomically before accepting even an unchanged value.
Both audience faces require a mutation identity from `Capabilities.mutations` for
Set. The manager owns admitted work through caller cancellation; the provider commits the
complete replacement or refusal and its receipt atomically. Replaying the normalized request
returns the original result even after a later revision or source change. Replacing a source
requires authority over both the old and new source. Native callers use
`set_index_definition_once` or `set_projection_definition_once` for the same guarantee.
Definition receipts name a completed metadata revision; they carry no primary-data position.

Stale revisions conflict with the action “get again and retry”. A changed definition
requires explicit rebuild and creates a new revision. The memory adapter stores
definitions in its index catalog and uses bounded shared execution over retained
source rows. A product can implement native index construction and seek hooks.
`IndexRead` supports inclusive lower/exclusive upper key bounds, byte prefix,
direction and continuation. Its total order is canonical key bytes followed by
source position; reverse reads reverse both components. Ties remain stable.
Canonical scalar tags order null, boolean, signed, unsigned, finite float, text
and datetime. Within each field type natural order is preserved; signed zero is
normalized and text escapes embedded NULs. Use `harmos_query::index::encode_key` rather
than inventing encodings. Missing declarations return `IndexNotFound`. Rebuilding
invalidates old index continuations; changed plans and foreign cuts refuse.

## Durable Projections

```rust,ignore
let revision = storage.set_projection_definition(&definition, expected_revision, rebuild).await?;
snapshot.advance_projection(&definition.name, None, work, &cancel).await?;
let frontier = storage.get_projection_status(&definition.name, revision, None).await?.map(|status| status.frontier);
let status = storage.get_projection_status(&definition.name, revision, Some(snapshot.at())).await?;
let page = storage.list_projection_rows(&read).await?;
```

Definitions select columns or generic grouped time buckets. Registration and
rebuild are explicit. Source commits do not imply projection advancement: catch-up
materializes one exact saved cut under supplied work bounds. Output pages,
changed keys and frontier publish atomically. Interrupted publication leaves the
old frontier intact; retrying a published cut is idempotent. A stale concurrent
candidate conflicts if its expected predecessor changed. Frontiers are monotonic
within a revision; an unpublished older cut cannot be added behind the frontier.

`ProjectionRead` names the definition revision and exact published frontier,
returned page limits, decoding work limits and output ordinal `after`. Old cuts
and revisions remain readable after rebuild until explicit `retire_projection`.
Lag returns `NotReady`; retired cuts return `Gap`. Retirement cannot exceed the
saved cursor. Changed-key and output-row buffers share an ordinal; removed bucket
keys can produce key-only tail pages. Read until `next` is absent.

## Grouped Time Buckets

`TimeBucketPlan` supplies origin, nonzero width, time/value/identity column paths,
optional grouping columns, maximum samples and state bytes. Bucket ordinal uses
Euclidean division `(time-origin)/width`, including negative time. Intervals are
half-open. Time is Int64 or nanosecond timestamp; values must be finite Float64,
including superseded input versions. Identity is non-null and unique across the
selected source, including groups. The greatest source position replaces earlier
versions of that identity, even when it moves groups or buckets. Identical repeated
positions deduplicate; conflicting duplicates refuse.

Each bucket retains count, first/last/min/max, sum, sum of squares, mean and M2,
the selected sample positions and all contributing positions in `(time, position)`
order. First/last and extrema ties use that stable order. Canonical binary group
values plus signed bucket ordinal identify a bucket; SHA-256 revisions identify
its retained content. Catch-up emits sorted changed and removed keys relative
to the preceding published cut. Late input replaces affected buckets; old saved
outputs remain readable. `Transform::TimeBuckets` and projection execution use
the same bounded reduction. The protobuf mapping carries the same bounded time-bucket transform.

The in-memory adapter uses the common executor's bounded source reduction and
atomic projection catalog publication. Native checkpoint and incremental physical
bucket implementations belong to a product backend; they must preserve the same
canonical aggregate order, replacement and retained-frontier semantics.

## Conditional Objects

```rust,ignore
let outcome = storage.compare_exchange(expected, &value).await?;
let replayable = storage.compare_exchange_once(&mutation, expected, &value).await?;
```

`None` means create only if absent. A present token names that exact object's
current revision, key and store history; another object's token refuses. The
atomic backend request owns both the prepared schema and object value at
consecutive admission positions. Success publishes schema, object, index effects
and receipt together. Conflict publishes its original outcome and durable
frontier, with no schema or object change. Manager results are durable outcomes;
a caller cancelled after admission may have committed and should replay its
mutation ID. Reusing an ID with a changed fingerprint refuses.

The client and context `GetObject` methods return the complete value, opaque revision,
existence flag and resolved cut. Omitted `at` selects the latest durable cut immediately;
an issued retained `at` reads that exact image. `SetObject` takes the complete candidate,
its schema, a mutation identity and either that revision or explicit create-if-absent.
Stale candidates return a typed conflict with the current revision and the action
“SetObject: get again and retry”. The
successful reply carries the original value, revision and durable receipt. Client and
context handlers share the native atomic primitive.

Accepted positions replay their original success or conflict after later writes
and restart. Consumers explicitly retire receipts with
`delete_object_receipts(through)`; retrying a retired outcome reports `Gap` rather
than applying again. Object pruning preserves rows protected by live snapshots
and explicit catalog retention. Exact or revision reads crossing reclaimed rows
report `Gap` rather than silently truncating history.

## Files, Recovery and Adoption

The reference backend preserves ordered staged file chunks, identical replay,
length/hash verification, immutable published references, bounded range reads and
mutation receipt recovery. Unpublished file bytes cannot be read as published
content. The memory adapter retains resumable sessions, writer generations and checkpoints;
reopening trims an uncheckpointed tail to its durable prefix. Abandonment preserves
retained content reads. File deletion remains a separate advertised capability. The existing file state/recovery
backend and its journal/checkpoint format are retained.

Private in-process storage operations expose these capabilities; a product owns
any wire wrapper and its policy. Register `builtins::storage_catalog()` and supply a storage manager with
`setup.settings().storage(&storage)`. The runtime supplies the retained snapshot registry,
bounded by the consumer's `StorageRequestSettings.max_active_queries`; backend construction
and every row/byte/work allowance remain consumer-owned. Embedders composing resources
directly also supply `harmos_operations::storage::SavedSnapshots::new(capacity)`.

Ordinary native queries retain their bounded row fallback. Native `ListRows` uses
bounded pages; a product chooses its wire query shape. `AppendRecords`, `AppendSamples` and `AppendObjectRevisions` take
one bounded immutable batch and a mutation identity. Their replies report the original
accepted position immediately. Resend the same normalized request to observe completion:
`durable_at` appears only after the data and its replay receipt are acknowledged durable.
The storage worker owns both stages after caller loss; latest reads never request them.
The identity and accepted position stay fixed as completion advances. Native consumers can
wait passively with `wait_mutation_durable`; client resending stays client-owned.
Changing data under the identity conflicts. Receipt capacity eviction retires unknown IDs
from that retry window; retained IDs still replay, while new work obtains a current window
from Capabilities. Conditional updates use the separate
`Storage.SetObject` operation, which validates the declared object catalog,
schema graph, payload and object key before one atomic admission. Independent calls do not
form a cross-method transaction. File sessions, recovery and receipt retention remain intact.

## Private Native Operations

Storage has one in-process operation set in `harmos_operations::storage`. Register
`storage_catalog()` with the runtime alongside the product's registered backend.
Every declaration has Private visibility and no gRPC projection. The library
exposes no storage service or storage object-definition listing on either boundary.

Products declare their own client and plugin-host operations with their own scope,
attribution, authorization, typed results and policy. An admitted host handler calls
the private native operation through `ExecutionContext::call_native` or
`FunctionContext::call_native`; the call keeps
its context, parent cancellation and scope. Native host code uses
`runtime.operations.call_native` or `submit_native` with typed endpoints. This helper
performs ordinary operation admission and applies the callee's policy.

```rust
use harmos_operations::storage::{GetFileBytes, GetFileBytesRequest};

let result = context.call_native::<GetFileBytes>(GetFileBytesRequest {
    file: Some(reference),
    offset,
    max_bytes,
    at,
}).await?;
```

Requests and results are native Rust values; schema graphs, mutations, positions,
file sessions, queries, conditional outcomes and Arrow pages retain their native
owners. Shared `harmos.v1.File`, `Upload`, `FileReference`, positions, cuts and
receipts are vocabulary a product can reuse at its serialization boundary.
`Plugins.Create` and `Plugins.Replace` still accept an immutable bundle reference.

ListRows and native column/index/projection operations retain bounded pages and
issued cuts. Return `resolved_at` unchanged and continue with `next_page_token`.
Tokens bind the retained image, selector, transforms, ordering and progress; foreign,
changed, released or expired selections refuse. The current page remains replayable
with the same bounds until its next continuation is consumed. An empty page with a
continuation represents progress. Paused queries release execution slots.

Snapshots pin durable images. Lease issuance and continuation signing keys are
process-local and expire across restart. Durable mutation receipts can reissue a
position once its saved frontier is available, without forcing a flush. Accepted
work is never substituted for a durable image. Index and projection cuts retain the
exact definition revision and frontier rather than silently selecting current data.

Consumer response and work limits still bound native allocations, IPC metadata,
changed-key buffers and continuation metadata. A product owns its separate wire
message and authorization limits. Storage failures retain structured operation
classification and `details["storage.kind"]`. Conditional object conflicts are native
outcomes with the current version token. Schema revision bytes preserve the existing
position encoding. Retrying an admitted mutation reuses its original identity and
original outcome, including after cancellation or restart.

## Shared Benchmark Workload

Enable `harmos-storage`'s `testing` feature to use `bench_support`. In a dedicated
benchmark executable, install `bench_support::TrackingAllocator` as `#[global_allocator]`,
construct `bench_support::workload::Workload::new(workload::ROWS)`, and call
`bench_support::run_with_reopen(&runtime, &workload, || backend.clone())`.
The factory returns the same `Arc<dyn StorageBackend>` owner for close/reopen.
The library harness uses the memory adapter; a product can supply its own owner.
This runs identical data, plans, corrections, checks and case names against any backend;
no concrete database or host engine is required. For JSON, consume its `STORAGE_MEASUREMENT`
and `STORAGE_COUNTER`/`STORAGE_WORK` lines; the repository producer's `storage_bench.measurements(lines,
revision, system, backend="reference")` emits the release schema for another backend.

`StorageBackend::get_work()` optionally returns cumulative `StorageWork` counters.
Differences around an isolated call report decoded source rows/bytes/chunks,
bucket members and buckets recomputed, index entries visited, backend-specific index steps
and prepared index statements. Counters belong to an owner and include repeated
physical decoding; encoded bytes are not unique device I/O. Unsupported counters
remain zero. `advance_projection` is an optional backend hook; returning
`None` selects the shared bounded executor without changing existing implementers.
`Snapshot::advance_projection` accepts an optional expected revision and forwards it
to that same hook. A native implementation must check the expected
active revision atomically with publication.
Returning `None` retains the shared executor's revision and publication checks.

`storage.window.time.2m` searches the whole history with exact time bounds covering
the same interval as the first positional window. `storage.window.numeric.2m`
adds the half-open value predicate `[10000, 20000)` to that time window. Both start
at the beginning of the source and report source rows and bytes decoded, proving
time pruning independently of a positional seek. Expected rows and sums are
generated independently outside timing. First observations use the `.first` suffix;
warm timings keep the shared ten-sample naming used by Nightly `benchmarks.json`.

The fixed input is 2,000,000 `(time_ns:i64, plot:u32, value:f64)` records over three days
and 24 plots. There is no RNG: logical index is admission index except every 4096th row
arrives 2048 positions late; value is `(logical*7919)%65521`. Chunks contain 16,384 rows,
windows 4096 rows, output/physical chunk limit 8 MiB and work limit 1 GiB. Corrected input
replaces the first 4096 identities, shifts their time back one minute and adds one to each
value. Bucket width is one minute, grouped by plot. Conditional contention is 64 rounds
of eight requests sharing one expected version; exactly one applies in every round.

| Stable Case Name | Measurement (Unit: `s`) |
| --- | --- |
| `storage.ingest.2m` | Commit the pre-generated source batches |
| `storage.scan.read.2m` | Traverse all values |
| `storage.window.read.2m`, `storage.window.next.2m` | Read at one-third of the source and the next window |
| `storage.index.build.2m` | Register/build the value index |
| `storage.index.read.2m`, `storage.index.next.2m`, `storage.index.deep.2m` | First, next and middle-of-source ordered seek pages |
| `storage.projection.advance.2m`, `storage.projection.read.2m`, `storage.projection.replay.2m` | Publish, read a window and retry an already published cut |
| `storage.bucket.advance.2m`, `storage.bucket.replace.2m` | Initial grouped buckets and late/corrected replacement |
| `storage.object.contend.8` | One eight-request round, aggregated across 64 rounds |
| `storage.object.replay.512`, `storage.object.restart_replay.512` | Replay the final durable outcome before/after reopening |
| `storage.scan.cancel.2m`, `storage.projection.cancel.2m` | Already-cancelled read/advance refusal |
| `storage.close.2m`, `storage.stop.2m`, `storage.restart.2m` | Logical close, reference-owner disposal and reference-owner reopen |
| `storage.projection.restart_replay.2m` | Replay the retained projection cut after reopening |

Scan/window names also have a `.first` single-observation case; it is a first traversal,
not an OS-cache-cold claim. Repeated read/replay cases use ten samples after a one-second
warmup; mutating/build cases run once. Every timing name also emits `.allocations` (`count`),
`.allocated.bytes` (`bytes`) and `.heap.peak.bytes` (`bytes`). These count Rust allocator
calls, total requested bytes and the increase in live Rust heap high water during the
region. Native allocations outside the Rust allocator are not counted. `storage.process.peak.bytes`
(`bytes`) includes native memory and untimed preparation. Scans emit `.read.bytes` (`bytes`)
and `.payload.rows` (`rows`); encoded fetched bytes are not physical disk I/O. Ingest,
full scan, projection/bucket advancement emit `.throughput` (`rows/s`); contention uses
`attempts/s`. Verification and fixture generation are outside every timed region.

`mise run bench -- --storage --filter interface` runs the two-million-row library
workload against the memory adapter, without file or disk management. Product
reports can retain the same names, units and deterministic input while recording
their backend and durability mode. The million-row temporal merge cases live in
`harmos-query/benches/merge/`; existing transform cases live in the query and
storage query benches.

## Durable-Lag Benchmark

Run `mise run bench -- --storage --filter durable_lag` for the Nightly
`integration/durable_lag/{small_high_rate,multi_megabyte,mixed}` cases. Each reports
acceptance-to-durability p50, p95 and maximum, pending bytes/positions, admission wait,
and optional backend commit count/time. Five runs per load retain median and
spread, compiler-activity samples and stage counters for cadence/ordered waits,
batch construction, provider writing, synchronous commit and frontier publication.
Durable lag was accepted as measured, amended by the owner on 2026-11-13:
small about 99 ms; large median 391 ms (178–570); mixed median 346 ms (173–493)
at p95. Build no accepted-data snapshot. Nightly keeps these measurements without
a failing threshold. Windows evidence is recorded in the operation-grammar design;
native Linux and hosted measurements remain pending.

Consumers enable `testing`, persist `bench_support::durable_lag::Payload`'s schema,
and call `bench_support::durable_lag::run(&manager, &schema, load)` on their started
manager. Supply the ordinary backend, worker owner, settings and durability mode.
The observer only subscribes to saved progress; it never requests a flush.

The worker starts flushing on its cadence or when queued bytes reach the existing
size threshold. `StorageSettings::flush_trigger_bytes()` requests early persistence separately
from commit sizing. Typed provider writes retain the existing
`max_batch_bytes()` ceiling; admission refuses entries beyond that ceiling. Each
successful piece publishes its saved frontier independently, and retry skips its
already durable prefix. The redundant 1 MiB per-commit split was removed after
stage measurements showed synchronous-commit amplification and small-write blocking;
no stored encoding, durability setting or admission ceiling changed. Large
admissions remain separate commit units, so an earlier small prefix cannot join
a later large admission. Checked admission sizes are reused when batching and
persisting. Larger state captures still split at the existing atomic byte ceiling.
Physical chunking and on-disk transaction costs belong to product measurements.

## Native Provider Hooks

`StorageBackend` has 52 hooks and `StateBackend` has eight. Implement the names in
`crates/harmos-storage/src/backend.rs` and `src/state/mod.rs`; there are no old-name
aliases. Storage and recovery sessions have distinct Create/Delete hooks. Appends
retain their assigned-position replay and durable-receipt contracts. Private flush
remains an explicit recovery barrier.

`capabilities()` owns `file_sessions`, `file_deletion` and
`mutation_receipts` alongside query capabilities. Unsupported capabilities refuse
explicitly. `get_projection_definition(name, revision)` returns a definition and
revision, selecting current when revision is absent. `get_projection_status` accepts
an optional exact frontier; absence returns the newest readable durable status for
that revision. `advance_projection` accepts an optional expected revision, preserving
atomic checked native publication and the checked shared fallback.

An append wholly inside an accepted file-production prefix verifies the original
bytes. Identical bytes succeed with a new request identifier without extending or
rehashing the file; different bytes conflict. A batch crossing the accepted end
or leaving a gap is refused. Reusing an existing request identifier still requires
the identical request and returns its original receipt.

## Unary File Transfers

The private native operations own resumable productions, writer generations,
ordered appends, publication, abandonment and bounded content reads. Mutation
identities come from `StorageManager::mutation_window()`. Keep the same identity
and payload when retrying a lost response. Resuming a writer fences its previous
generation. Publishing verifies length and SHA-256, and acknowledges the immutable
reference with its durable receipt.

```rust
use harmos_operations::storage::{CreateFileProduction, CreateFileProductionRequest};

let created = operations.call_native::<CreateFileProduction>(
    CreateFileProductionRequest {
        metadata: Some(metadata),
        mutation: Some(mutation),
    },
).await?;
```

A product chooses its wire resources. The recommended shape is `File` and `Upload`
with Create, Get, Resume, Append, Publish, Delete, Read and Watch where needed. Each
handler delegates to the corresponding native operation. A plugin that publishes
stored rows or records calls a product-defined operation; interval samples answer
host ticks on `Instance.Samples`.

`StorageSettings.transfer.chunk_bytes` supplies the shared 4 MiB transfer default.
Carriers use the separate 8 MiB message default, validate framing and refuse ceilings
that cannot carry a chunk. Consumers can override their transfer and message settings.
Each bounded append or read retains ordinary cancellation, replay and receipt rules.

## Application-Owned Content

A host can serve immutable bytes it already owns through a product operation over
the private native GetFileBytes operation. Register one `harmos_storage::ContentSource` on the StorageManager. The
application supplies references with a distinct history and an immutable metadata
identifier; the library calls `resolve(reference.metadata.id.as_str(), caller_scope)`.
The scope comes from trusted invocation context, including inherited plugin calls.
It remains separate from storage grants and never comes from the reference payload.

```rust
use harmos_storage::{ContentLease, StorageFuture};
use std::num::NonZeroUsize;

pub trait ContentSource: Send + Sync + 'static {
    fn resolve<'a>(&'a self, reference: &'a str, scope: Option<&'a str>)
        -> StorageFuture<'a, ContentLease>;
    fn read<'a>(&'a self, lease: &'a ContentLease, offset: u64, maximum: NonZeroUsize)
        -> StorageFuture<'a, Vec<u8>>;
}

storage.register_content_source(std::sync::Arc::new(source))?;
let read = storage.open_content(&reference, Some("caller-scope"), None).await?;
let range = read.get_bytes(offset, maximum).await?;
```

`ContentLease` carries `length`, `sha256`, `expires_at: SystemTime`,
`max_bytes: NonZeroUsize` and application `token: Vec<u8>`. Use the runtime's
captured clock to choose expiry. The library checks expiry before/after reads,
times out pending reads at that expiry, checks metadata against the supplied
reference and enforces both the lease and transfer bounds. Short provider reads
are accumulated in order; empty/oversized reads refuse. A complete single-range
read verifies SHA-256, and callers verify the digest after assembling multiple ranges.

Product wrappers and their callers retain `resolved_at` for every subsequent range. The existing bounded
read-cut owner retains the lease and binds it to the reference and caller scope.
Expired/evicted/foreign continuations fail with GetFileBytes named; start a new
read explicitly rather than silently renewing a lease. No application content is
written into library persistence. Library-history references still select saved
storage and perform the existing reference/provenance checks. Registration refuses
replacement and retires with its storage owner. Bundle installation and diagnostics
use the same content-source/read mechanism.

## File-Transfer Benchmark

`mise run bench -- --storage --filter file_transfer` builds only the runtime's
`integration-file-transfer` target. The reusable workload transfers a deterministic
30 MiB file with the storage-owned 4 MiB default chunks/ranges, checks its length
and digest, and reports three repetitions for client and plugin unary transfers.
Client calls cross a host socket; plugin calls cross the host/plugin carrier as
context child calls. Download timing includes incremental hashing; comparisons
happen after timing. Streaming transfer handlers, declarations and cases are deleted.

The benchmark composes the existing reference memory backend and reuses retained
saved images across unary ranges. The figures are transport/reference-provider
measurements, not a claim about durable filesystem throughput. Consumer backends can reuse
`harmos_storage::bench_support::file_transfer::{Carrier, run}`. Nightly retains raw
native output and median throughput with bytes, call counts and configured range
sizes. The owner accepted the measured unary tradeoff on 2026-11-13.

## Story Migration

Of the original 23 backend-focused integration stories, nine remain over shared
memory owners: saved columnar cuts, index seeks/rebuild, projection replay,
conditional objects, schema exchange, bucket replacement, stream identity,
snapshot singleton publication and runtime checkpoint reopen. These assert
interface behavior, including failures, cancellation and exact retained answers.

Fourteen disk/backend-only stories were removed for product ownership:

| Previous File | Deleted Story |
| --- | --- |
| `benchmark_boundary.rs` | `shared_workload_keeps_boundary_pages_and_replayed_projection_identical` |
| `bounded_native_chunks.rs` | `coalesced_admissions_keep_physical_chunks_bounded_and_commit_atomically` |
| `corrections_and_tied_seeks_do_not_revisit_unaffected_history.rs` | `corrections_and_tied_seeks_do_not_revisit_unaffected_history` |
| `sqlite_files.rs` | `finished_files_and_mutation_receipts_reopen_without_resetting_incompatible_roots` |
| `state/file.rs` | `rollback_requires_reopen`; `interrupted_segments_preserve_checkpoint_and_journal`; `segmented_manifest_rejects_missing_reordered_and_damaged_frames`; `existing_single_payload_checkpoint_remains_readable` |
| `state/recovery_seeks_and_compacts_across_page_boundaries.rs` | `recovery_seeks_and_compacts_across_page_boundaries` |
| `state/index_open_cancellation_preserves_the_durable_journal.rs` | `index_open_cancellation_preserves_the_durable_journal` |
| Runtime `storage/state/files.rs` | `corruption_refused_preserved`; `state_root_exclusive`; `limits_bound_records`; `unsupported_format_refused` |

Implementation-local unit tests leave with the deleted SQLite and journal
implementations. The schematic example's separate JSONL storage crate, format
fixtures and disk-format stories are also removed. Its application storage
stories exercise the public memory adapter; manual document Save/Open remains
application-owned file serialization. Library tests do not claim disk durability.
