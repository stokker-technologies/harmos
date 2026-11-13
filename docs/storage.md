# Storage Interfaces

`harmos-storage` owns typed persistence, bounded Arrow scans, indexes, durable
projections and atomic conditional objects. Consumers choose capacity, concurrency,
physical chunk sizes, work budgets and response limits. Storage does not expose
product keys or a database connection.

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

## Reference Backend

```toml
[dependencies]
harmos-storage = { version = "0.1.14", features = ["sqlite"] }
```

```rust,ignore
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
let primary = SqliteBackend::new(path, history, SqliteSettings {
    chunk_rows,
    chunk_bytes,
})?;
```

`sqlite` is optional and uses bundled SQLite. The backend owns one native file
lease for its instance lifetime; share that instance between callers. Durable
commits use SQLite WAL with FULL synchronization. Typed rows, column chunks,
indexes, projection publications and conditional outcomes live in that database.
The independent state/recovery backend remains a separate `StorageBackends::state`
owner. There is no transaction across these owners or unrelated backend methods.

An unrecognized format or different store history fails with an instruction to
back up and deliberately reset the storage root. Opening never silently resets
incompatible data. Application owners choose backup and migration policy.

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
Change the plan by starting a new read. Native SQLite reads fetch only required
position, projection and predicate/time columns, with metadata pruning before
column loading. `ArrowPage` reports source rows scanned, encoded column bytes read,
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
requires explicit rebuild and creates a new revision. SQLite builds history under
one immediate transaction and updates entries in the source write transaction.
Construction decodes each physical source chunk once, orders encoded entries and
inserts them through one prepared statement in that transaction. Record and stream
pages read the covering key/position index directly; object pages additionally
resolve the latest object revision at the saved cut.
`IndexRead` supports inclusive lower/exclusive upper key bounds, byte prefix,
direction and continuation. Its total order is canonical key bytes followed by
source position; reverse reads reverse both components. Ties remain stable.
Canonical scalar tags order null, boolean, signed, unsigned, finite float, text
and datetime. Within each field type natural order is preserved; signed zero is
normalized and text escapes embedded NULs. Use `query::index::encode_key` rather
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

For record and stream projections, SQLite checkpoints the latest sample for each
logical identity and indexes its bucket membership. Advancement reads source rows
after that checkpoint and recomputes the old and new buckets of changed identities.
It recomputes moments in the canonical `(time, position)` order, avoiding subtraction
of floating-point aggregates. Bucket versions, removed-key tombstones, membership,
changed keys and the frontier commit together. Saved cuts share unchanged bucket
versions; retirement retains private anchors needed by later cuts. Object-source
projections use the common executor's latest-object selection semantics.

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
content. Optional resumable sessions and deletion remain separate advertised
capabilities; they are not implied by `sqlite`. The existing file state/recovery
backend and its journal/checkpoint format are retained.

The native storage interface and the two face-local Storage services expose these
capabilities. Register `builtins::storage_catalog()` and supply a storage manager with
`setup.settings().storage(&storage)`. The runtime supplies the retained snapshot registry,
bounded by the consumer's `StorageRequestSettings.max_active_queries`; backend construction
and every row/byte/work allowance remain consumer-owned. Embedders composing resources
directly also supply `harmos_operations::storage::SavedSnapshots::new(capacity)`.

Ordinary native queries retain their bounded row fallback. Public `ListRows` uses
bounded unary pages. `AppendRecords`, `AppendSamples` and `AppendObjectRevisions` take
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

## Generated Storage Clients

Shared storage payloads live under `harmos.storage.v1`. The Rust client wrappers are
in `harmos_protocol::v1`, and context wrappers in `harmos_protocol::context`. Each face's
generated methods use its own descriptors and fingerprints. Python's client service is
`harmos.v1.storage_pb2_grpc.StorageStub`; the context declarations are under
`harmos.context.v1`. SDK root generation and audience-specific stubs follow the protocol
owner's descriptor table.

Client storage uses `harmos.v1.Storage`; invocation context uses
`harmos.context.v1.Storage`. Each face owns its service-qualified method wrappers
and grant identity. Both faces delegate to the same storage handlers.

| gRPC Method | Result |
| --- | --- |
| Capabilities | Native acceleration and correctness capabilities |
| CreateSnapshot / DeleteSnapshot | Explicit retained durable lease and release |
| ListRows | Bounded unary query page, issued cut and continuation token |
| WatchRows | Durable boundary, exclusive changes, filtered progress and explicit resynchronization |
| ListColumnRows | Arrow IPC and source-position continuation |
| ListIndexEntries | Ordered key/position seek page |
| GetIndexDefinition / SetIndexDefinition | Complete definition and atomic revision guard |
| GetProjectionDefinition / SetProjectionDefinition | Complete definition and atomic revision guard |
| AdvanceProjection | Checked materialization at a retained cut |
| GetProjectionStatus | Selected revision and newest readable or issued exact frontier |
| ListProjectionRows | Bounded exact-frontier page and composite cut |
| GetObject / SetObject | Complete object, opaque revision and atomic applied/conflict outcome |
| AppendRecords / AppendSamples / AppendObjectRevisions | Bounded immutable ingestion and accepted/durable receipts |

A missing `at` selects the current durable position immediately. Projection status
without `at` selects its newest readable materialization; an explicit `at`
must match a retained issued cut and the selected revision. Reads never flush
or wait for accepted writes. A supplied cut must equal a retained cut this owner
has returned; arbitrary older positions, changed composites and foreign histories
refuse. Automatic cuts use the existing saved-read capacity and evict the oldest
idle cut when needed. `CreateSnapshot` pins its lease until `DeleteSnapshot` or
storage retirement. Active reads retain their image through completion. A failed
or cancelled page does not release an explicit lease.

Return the whole `SavedSnapshot` unchanged for explicit lease release or projection
advancement. Native list methods take a selector, optional issued `at`, and the common
`PageRequest` (`page_size`, `max_bytes`, `page_token`). Their replies carry `resolved_at`
and `next_page_token`. Keep the returned cut or omit `at` when continuing the signed token.
Column/index/projection selectors contain source/ordering/work selection; response sizing
and continuation belong to PageRequest. Index cuts bind the selected definition revision,
which the provider checks within its scan; a rebuild refuses old revision reads explicitly.
Projection cuts bind the exact retained revision/frontier. Definition getters also honor
issued participating revisions; unavailable index revisions refuse rather than substituting
current metadata. The native page mechanisms retain their own seek/materialization behavior.

The complete encoded reply respects both `max_bytes` and the configured response ceiling.
Column/projection packing counts IPC metadata as well as native buffers; an indivisible
row or response metadata that cannot fit refuses explicitly. A short provider page is
progress, so a filtered empty column page can still return a continuation.


Signed native-list tokens bind the retained image, selector and native continuation. ListRows tokens
bind the retained image, selector, transforms, ordering and query progress with an
owner-local HMAC; page size may change within consumer bounds. The most recent continuation request/result remains replayable with the same bounds;
consuming its next token expires older requests. Each call advances one bounded source
or output step, so an empty page can carry a continuation without claiming end-of-source.
Idle query plans share the existing retained-read capacity and can expire when another
read needs it. Paused plans release active execution slots and reacquire them per call. Row-window skipping and stopping belong to the
caller across bounded pages. Responses include their complete encoded envelope in
the byte bound. An indivisible row or metadata that cannot fit refuses explicitly.

Read-lease issuance and page-token keys are not persisted. A retained mutation receipt
can reissue its exact position after the saved frontier proves it durable, including
a recovered durable receipt. Pending receipt positions refuse without flushing;
expired receipts or unavailable provider images refuse explicitly. Released, evicted, foreign and
restarted leases/tokens refuse explicitly; restart the read with no `at`. A durable
position may be returned again as a newly issued cut if it is still the current
readable image, but that does not restore an old page token. Composite cuts retain
the exact returned storage position and provider revisions; callers cannot invent
or combine a different tuple. These leases pin existing durable backend snapshots;
no accepted-data snapshot is constructed.

Both output `limits` and source `work` are required, nonzero row/byte bounds.
Wire page bounds include IPC schema metadata, changed-key buffers and continuation metadata. Column and index
requests name a schema source; named index/projection requests also carry that source so the
host can check it against the registered definition. Grants use `objects:schema:<id>`,
`records:schema:<id>` or `streams:schema:<id>` with the corresponding operation ID. Reading
an old projection revision checks that revision's source even after a source-changing rebuild.
Time-bucket transforms now map in both ordinary queries and durable projections; grouping,
nonzero widths and sample/byte bounds preserve the native contract.

Failures use the existing structured operation failure with `details["storage.kind"]`:
`STORAGE_FAILURE_UNSUPPORTED`, `CONFLICT`, `EXPIRED`, `NOT_READY`, `GAP`, `EXHAUSTED`,
`INVALID` or `MISSING_INDEX` (each has the `STORAGE_FAILURE_` prefix). Rust consumers can use
`StorageFailure::from_operation`. An expected version mismatch is a successful
`CompareExchangeOutcome.conflict`, including the optional current token. Replaying an admitted
mutation returns the original durable applied/conflict result. Cancellation after admission
can leave committed work; retry the same mutation ID, never a new read-then-write sequence.

The Schematic [query composition](../examples/schematic/app/src/services/queries.rs) builds
SQLite query storage alongside its application-owned JSONL adapter. Its
[saved preview journey](../examples/schematic/app/tests/storage/main.rs) retains a cut, applies
an atomic update, refuses a stale writer and reads the unchanged saved preview. It does not
replace the document journal, checkpoints or JSONL stream adapter.

The focused Windows workload contains 2,000,000 samples across three days and 24
plots. Run `mise run bench -- --storage --filter baseline` or `--filter native`;
raw output and Criterion artifacts remain under `target/benchmarks/storage/`.
Correctness checks stay outside timing. Local measurements and proposed budgets
are recorded in the change; Linux and consumer performance proof remain open.

## Shared Benchmark Workload

Enable `harmos-storage`'s `testing` feature to use `bench_support`. In a dedicated
benchmark executable, install `bench_support::TrackingAllocator` as `#[global_allocator]`,
construct `bench_support::workload::Workload::new(workload::ROWS)`, and call
`bench_support::run_with_reopen(&runtime, &workload, || Box::new(open_backend()))`.
The opener first creates an empty backend, then reopens that same store after disposal.
This runs identical data, plans, corrections, checks and case names against any backend;
no concrete database or host engine is required. For JSON, consume its `STORAGE_MEASUREMENT`
and `STORAGE_COUNTER`/`STORAGE_WORK` lines; the repository producer's `storage_bench.measurements(lines,
revision, system, backend="reference")` emits the release schema for another backend.

`StorageBackend::get_work()` optionally returns cumulative `StorageWork` counters.
Differences around an isolated call report decoded source rows/bytes/chunks,
bucket members and buckets recomputed, index entries visited, SQLite VM steps
and prepared index statements. Counters belong to an owner and include repeated
physical decoding; encoded bytes are not unique device I/O. Unsupported counters
remain zero. `advance_projection` is an optional backend hook; returning
`None` selects the shared bounded executor without changing existing implementers.
`Snapshot::advance_projection` accepts an optional expected revision and forwards it
to that same hook. A native implementation must check the expected
active revision atomically with publication; SQLite holds its state lock across both.
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
region; SQLite's C allocations are outside those counters. `storage.process.peak.bytes`
(`bytes`) includes native memory and untimed preparation. Scans emit `.read.bytes` (`bytes`)
and `.payload.rows` (`rows`); encoded fetched bytes are not physical disk I/O. Ingest,
full scan, projection/bucket advancement emit `.throughput` (`rows/s`); contention uses
`attempts/s`. Verification and fixture generation are outside every timed region.

`mise run bench -- --storage --filter native` runs the SQLite reference with WAL/FULL
synchronization; commit, publication and contention costs include durability. Nightly
runs it on Linux and Windows and adds platform/backend-qualified suites to the published
`benchmarks.json` and raw report archive. The two-million-row executable is excluded from
ordinary benchmark lanes. Ordinary tests retain one 16,385-row journey crossing the
physical chunk boundary on both native and fallback backends. Consumer reports should
record their own backend/durability mode while retaining these names, units and inputs.

## Durable-Lag Benchmark

Run `mise run bench -- --storage --filter durable_lag` for the Nightly
`integration/durable_lag/{small_high_rate,multi_megabyte,mixed}` cases. Each reports
acceptance-to-durability p50, p95 and maximum, pending bytes/positions, admission wait,
and synchronous SQLite commit count/time. Compare each p95 independently with 250 ms;
a successful benchmark process does not assert that latency bound. Windows measurements
are recorded in the operation-grammar design; native Linux and hosted evidence remain pending.

Consumers enable `testing`, persist `bench_support::durable_lag::Payload`'s schema,
and call `bench_support::durable_lag::run(&manager, &schema, load)` on their started
manager. Supply the ordinary backend, worker owner, settings and durability mode.
The observer only subscribes to saved progress; it never requests a flush.

The worker starts flushing on its cadence or when queued bytes reach the existing
size threshold. Typed provider writes split at entry boundaries around
`StorageSettings::flush_batch_bytes()`. A single indivisible entry may use the larger
`max_batch_bytes()` ceiling; admission refuses entries beyond that ceiling. Each
successful piece publishes its saved frontier independently, and retry skips its
already durable prefix. Splitting changes no stored encoding, durability setting or
atomic conditional-object replacement.

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

Clients call `harmos.v1.Storage`; plugins make child calls to
`harmos.context.v1.Storage`. Each face owns its request and response wrappers;
`harmos_operations::storage` and its `context` module declare them, sharing the
StorageManager production owner and handlers. Every action requires its own
operation/file grant. Providers without `file_sessions` and `mutation_receipts`
refuse creation explicitly.

```rust
use harmos_operations::storage::{CreateFileProduction, AppendFileBytes,
    GetFileProduction, CreateFileWriter, PublishFileProduction,
    DeleteFileProduction, GetFileBytes};
use harmos_protocol::{v1, storage};

let production = operations.call_protobuf::<CreateFileProduction>(
    v1::StorageCreateFileProductionRequest {
        metadata: Some(metadata),
        mutation: Some(mutation),
    },
).await?.production.unwrap();
let appended = operations.call_protobuf::<AppendFileBytes>(
    v1::StorageAppendFileBytesRequest {
        session: production.session,
        offset: 0,
        data: bytes,
        mutation: Some(next_mutation),
    },
).await?;
```

Obtain mutation identities from `StorageManager::mutation_window()`; retain the
window and key when retrying a lost reply. An identical normalized request
returns its original outcome; changing it conflicts, and expired windows refuse.
Chunks are bounded by the configured response batch byte limit and pending write
budget. Appends require contiguous offsets. Replaying an identical retained
prefix under a new key succeeds; different bytes conflict without modifying it.
`GetFileProduction` observes retained length, phase and current writer generation
without acquiring a writer. `accepted_length` includes an admitted in-flight
append; the production length includes completed backend appends.
`durable_length` advances only after the existing production recovery checkpoint
acknowledges its prefix, and is restored from that checkpoint after reopening. `CreateFileWriter` advances the generation and fences the
previous writer. Terminal productions have no live writer (generation zero).

`PublishFileProduction` supplies the exact length and SHA-256. It validates both
and awaits persistence of the immutable reference and its receipt before returning
`file` and `receipt.durable_at`. Prefix/session completion does not invent a global
history position: its receipt has `completed`, while its status carries the retained
prefix length. Delete abandons unpublished production; losing an observer does
neither. Published-file deletion retains its separate contract.

`GetFileBytes` takes the immutable reference, offset, positive `max_bytes` and
optional `at`; it returns `data`, `total_length`, SHA-256 and `resolved_at`.
Absent `at` selects saved progress without requesting a flush. Repeat with the
returned cut and the next offset; verify the complete reconstructed digest.
An offset at the end returns an empty range, an offset beyond it refuses, and
configuration bounds oversized ranges. Native embedders can call
`StorageManager::get_file_bytes(&file, offset, maximum, at)` directly. Streaming
transfers remain for the required throughput comparison before their removal.

## File-Transfer Benchmark

`mise run bench -- --storage --filter file_transfer` builds only the runtime's
`integration-file-transfer` target. The reusable workload transfers a deterministic
30 MiB file with the configured 64 KiB chunks/ranges, checks its length and digest,
and reports three repetitions for each available carrier/mode. Client unary and
streaming calls cross a host socket; plugin unary calls cross the host/plugin
carrier as context child calls. The existing plugin carrier cannot call streaming
host operations, so it reports that comparison unavailable.

The benchmark composes the existing reference memory backend. Its saved snapshots
copy file blobs, which disproportionately costs repeated unary ranges; the figures
are not a claim about durable filesystem throughput. Consumer backends can reuse
`harmos_storage::bench_support::file_transfer::{Carrier, run}`. Nightly retains raw
native output and median throughput with bytes, call counts and configured range
sizes. Streaming remains available for owner review of that evidence.
