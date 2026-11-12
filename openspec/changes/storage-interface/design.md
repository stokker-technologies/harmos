# Storage Interface Design

## Boundary and Sequence

```text
protocol <- reduced native storage contracts <- harmos-storage
                                               ^ operations <- runtime
Dokime operations -> dokime-storage -> Harmos storage interface
```

The owner permits parallel work at the landed Harmos host surfaces (see Locked Implementation Decisions). Settle the semantics below before
implementation, implement and prove Harmos, publish a usable revision, then Dokime adopts it in
its separate `storage-interface` change and extracts application persistence. Integration-only
local overrides may support coupled work; final validation uses the published upstream pin.
There is no storage extraction during the gRPC migration.

The interface already separates query snapshots from recovery snapshots, accepted writes from
durability, and independent backend writes from cross-method transactions. Preserve these
distinctions, existing schema/object/record/stream/file capabilities, mutation receipts and
replay-idempotent writes. Do not introduce product execution or cycle identifiers into Harmos.

## Original Interface Proposal

The following original proposed signatures are superseded by the locked shapes below. They are not existing APIs. Supporting types are new. Their
fields, errors, lifetimes and object-safe backend equivalents are design tasks; the capability
set is locked. Plans carry consumer-supplied page/work limits rather than Harmos budgets.

### Native Columnar Scans

```rust,ignore
fn scan_arrow<'a>(
    &'a self,
    snapshot: &'a BackendSnapshot,
    read: ArrowScan,
) -> StorageFuture<'a, ArrowPage>;
```

`ArrowScan` identifies source/schema, columns, predicate/time bounds, position continuation
and page limits. `ArrowPage` holds schema-compatible Arrow batches and continuation, avoiding
payload-to-row-to-Arrow reconstruction. Plan pushdown preserves results and source positions;
unsupported optimizations use an explicit row fallback through the query executor, not a
silent empty page. The backend must declare native support and implement snapshot-bound scans,
column projection, pruning and bounded continuation. Validate schema and allocation limits
without decoding unrequested columns. Retain typed readers for callers that do not need Arrow.

### Indexed Search With Ordered Seek Pages

```rust,ignore
fn scan_index<'a>(
    &'a self,
    snapshot: &'a BackendSnapshot,
    read: IndexRead,
) -> StorageFuture<'a, IndexPage>;
```

`IndexRead` names a registered index, key/prefix bounds, supported predicates, sort direction,
exclusive seek key and page limits. The order includes a stable unique tie-breaker. The backend
implements durable index maintenance consistent with source writes, bounded seek queries and
snapshot-bound tokens. Harmos specifies generic keys and ordering, not product status/history
filters or authenticated continuation tokens. Missing indexes must be distinguishable from an
empty result; settle refusal versus a declared bounded fallback before coding.

### Durable Materialized Projections

```rust,ignore
fn register_projection(
    &self,
    definition: ProjectionDefinition,
) -> StorageFuture<'_, ProjectionId>;

fn read_projection(
    &self,
    read: ProjectionRead,
) -> StorageFuture<'_, ProjectionPage>;
```

A definition identifies its inputs, transform, schema and version. Reads select a projection,
snapshot/cut and page bounds; pages report the readable source frontier and projection revision.
The backend durably stores definitions, outputs, replay progress and invalidation state. It
must not advertise a frontier ahead of durable output. Registration/replay must be idempotent
for an unchanged definition; changed definitions require explicit invalidation/rebuild semantics.
Settle synchronous commit-coupled advancement versus durable asynchronous catch-up in this
design. Either choice must state what a reader sees when its requested frontier is unavailable,
and how old-snapshot reads survive rebuild or fail explicitly.

### Time Buckets With Late-Sample Replacement

```rust,ignore
enum Transform {
    // Existing transforms remain.
    TimeBuckets(TimeBucketPlan),
}
```

This is a query transform, composable with native scans and durable projections, not a product
plot service or a second ingestion engine. Plans define time origin, width, ordering and
selection. Buckets preserve first/last/min/max, moments and contributing source positions.
Late or corrected samples replace affected buckets; readers can observe projection revisions
and changed keys without treating replacements as new appended buckets. Backends implement
the scan/projection hooks needed for persisted acceleration; the generic executor supplies
equivalent transform results where native acceleration is unavailable.

### Conditional Update

```rust,ignore
async fn compare_exchange<T: StoredObject>(
    &self,
    expected: Option<ObjectVersionToken>,
    value: &T,
) -> Result<ObjectReceipt>;
```

The typed object API delegates to an object-safe backend primitive. A proposed `None` means
create-if-absent; `Some(token)` replaces exactly that object version. A conflict reports the
mismatch without modifying data. The backend atomically checks and writes the object and
its revision/index effects. Admission, mutation-receipt replay and durability barriers retain
their separate meanings; a retry of an accepted mutation must not apply it twice.

## Semantics to Settle Before Implementation

The table defines the decisions the design must resolve, with proposed baselines. Tasks must
record the final choices here before interface implementation; consumers cannot guess them.

| Capability | Snapshot Consistency | Replay and Invalidation | Atomicity |
| --- | --- | --- | --- |
| Columnar scan | Bind every page to one retained snapshot, schema and source cut; define expiry errors | Repeating a read at the same cut yields the same order; reject incompatible continuation/schema revisions | Publish complete batches/pages, never partially visible rows |
| Indexed search | Stable total order and exclusive seek within one cut; define snapshot/token lifetime across restart | Rebuild/version changes must not silently reuse a seek token; specify restart persistence | Source changes and corresponding index visibility agree at the published cut |
| Projection | Return a readable frontier and revision; define exact-cut reads and lag behaviour | Define idempotent registration, crash replay checkpointing, rebuild and old-revision reader behaviour | Output and advertised frontier become durable together; settle source-commit coupling |
| Time buckets | Specify half-open boundaries, source-time ordering, ties, empty buckets and non-finite inputs at a cut | Define duplicate positions, corrections, late samples, changed-key retention and revision invalidation | A replaced bucket and its revision become visible together; old snapshots retain their prior result |
| Conditional update | Compare with the current committed version, not an earlier query snapshot | Settle token scope, absent-object semantics, conflict details and mutation-receipt replay after crash | Check, replacement and revision/index effects are one backend atomic action; no assumed cross-method transaction |

Across all five, specify cancellation before/after admission, error classification, page memory
accounting, snapshot release, retention and refusal when a required cut has expired. Define
projection dependency ordering and failure recovery without allowing a readable frontier to
claim missing output. Consumer policy supplies retention, batch sizes, concurrency and budgets.

## Backend Responsibilities and Reference Backend

`StorageBackend` extension points live in `crates/harmos-storage/src/backend.rs`; typed
conditional update is in `objects.rs`, and planning/fallback execution in `query/` with
`harmos-domain/src/model/storage/query/` retaining only non-wire native contracts. Existing
backend implementations must migrate together; unsupported capabilities return explicit
capability errors unless a specified equivalent bounded fallback exists.

The reference implementation proposed for this change is a durable SQLite backend under
`crates/harmos-storage`, with Arrow-native batch scan support and durable indexed/projection
tables. It implements all five capabilities, including transactional compare/exchange and
projection-frontier persistence; settle its module/feature and dependency footprint before
implementation. SQLite alone is not proof of native columnar scan efficiency: demonstrate the
batch/pruning path without reconstructing every selected row payload.

Extend `crates/harmos-storage/tests/support/storage.rs` for deterministic fault/ordering proof;
it is not the durable reference. Preserve `src/state/file/` as the production reference recovery
backend; its checksummed checkpoints/JSONL journal do not implement the new query facilities.
The reference is generic, with no Dokime dependency. Dokime keeps its own physical backend
behind the extended interface, preserving required resumable files, receipts and enumeration.

## Dokime Consumer Ownership

Dokime's separate change merges both runtime storage folders into `dokime-storage` and removes
concrete SQLite `State` escapes. Break writer-to-preview/query/runtime edges first: storage
owns persisted preview/index shapes, captured schemas, codecs and storage query inputs;
operations own host validation, parameter expansion and expression evaluation. Runtime owns
transactions/state/cycle transitions, execution manager, effects and plugin entries. Pass
owned inputs and narrow resources instead of `RuntimeHandle` or report callbacks into storage.

Keep execution/step-range resolution and statistics, history selection/token authentication,
evidence/metadata cuts, capture/finalization/seals, artifact inventory and incompatible-root
refusal as product semantics. They compose generic scans, projections, objects and durability
barriers without assuming independent storage methods form a cross-method transaction.
Logs, support bundles, exports/report spools, plugin installation, desktop root lease/backup,
client pairing/download files and CLI/build tooling stay outside application storage ownership.

## Correctness and Benchmark Proof

Qualify each backend against deterministic parity, stable pages, pruning, cancellation,
conflicting conditional updates, crash/restart, replay, rebuild/invalidation and late-sample
replacement. Include snapshot reads during index/projection advancement and old-cut expiry.
Run native Windows and Linux proof during implementation, not during specification authoring.

Capture before/after measurements on the same hardware, revision pair, dataset, query plans,
page sizes and durability settings. Compare existing Harmos storage suites and Dokime
`benches/storage/{query,contention,ingestion}.rs`, `benches/integration/{telemetry,plots,documents}.rs`
and `benches/integration/finalization.rs` where applicable; adapt surviving ingest benchmarks
to the gRPC host path. Existing finalization cases cover only 1/4-hour executions and do not
prove multi-day behaviour.

Add deterministic multi-day datasets with millions of samples, out-of-order/late data,
corrections, restart/replay and saved-cut versus live-tail queries. Record ingestion throughput,
first/next-page latency, allocations/peak memory, bytes read, projection catch-up and durability
cost, cancellation/stop latency and product finalization/report time. Separate warm/cold scans,
selective/indexed reads and contention. Verify result parity outside timing and keep raw
benchmark artifacts with reproducible commands and workload metadata. Settle numerical
regression budgets before accepting measurements; do not infer performance from source size.
Harmos proves the generic facilities before Dokime extraction; Dokime compares its own
before/after extraction with identical multi-day inputs and records unresolved regressions.

## Locked Implementation Decisions

The owner authorizes parallel storage implementation at Harmos revision `a7583836`:
protocol, operations and SDK host surfaces are landed; guest migration is the other writer's
work. The combined `grpc-plugins` marker remains authoritative. The consumer's final gRPC
wave remains pending; publication and consumer extraction are not claimed here.

### Present-Tree Inventory

| Owner | Source and Acceptance |
| --- | --- |
| Storage worker | `crates/harmos-storage/src/{backend.rs,objects.rs,query/}`; new `query/columnar.rs`, `query/index.rs`, `query/projection.rs`, `query/time_buckets.rs`, `sqlite/`; typed admission, bounded fallback and durable reference |
| Storage worker | `crates/harmos-storage/tests/support/storage.rs`, focused storage stories and `benches/multi_day/`; deterministic parity, restart, invalidation, conditional writes and late replacement |
| Storage worker | `tools/python/src/harmos_tools/storage_bench.py`, narrow bench dispatch in `ci/tasks.py`; existing task, focused compilation and native Criterion artifacts |
| Storage worker | This change, `docs/storage.md` and storage links/commands in `docs/product-guide.md`, `docs/tooling.md`; README claims updated |
| Minimal shared manifest edits | Workspace SQLite dependency/lock and the storage package's optional feature; inspect status before editing and stage only owned changes |
| Consumer adoption | `crates/harmos-domain/src/model/storage/query/{transform/,protobuf.rs}`, protocol storage protobuf/generated mappings, operations storage query/object handlers, runtime composition, SDK generated clients and examples; no new wire compatibility implied |
| Preserved owners | `src/state/file/` recovery journal, all existing file/session/receipt contracts; these are not query backends |

### 1.2 Scans, Ordering and Negotiation

```rust,ignore
struct StorageCapabilities { native_arrow: bool, native_index: bool, durable_projections: bool, conditional_objects: bool }
struct ColumnSource { kind: ColumnKind, schema: SchemaKey }
enum ColumnKind { Objects, Records, Stream }
struct ArrowScan {
    source: ColumnSource,
    columns: Vec<String>, // Dotted leaf paths; unique and nonempty.
    predicate: Option<ColumnPredicate>,
    time: Option<TimeBounds>, // Exact half-open nanosecond bounds.
    start: u64, // Initial exclusive source position.
    after: Option<ArrowSeek>, // Exclusive source position, not output-row offset.
    limits: ReadLimits,
    work: ReadLimits,
}
struct ColumnPredicate { column: String, lower: Option<f64>, upper: Option<f64> }
struct ArrowSeek { snapshot: Vec<u8>, plan: [u8; 32], position: u64 }
struct TimeBounds { column: String, lower_ns: i64, upper_ns: i64 }
struct ArrowPage { batch: RecordBatch, next: Option<ArrowSeek>, complete: bool, scanned: usize, bytes_read: usize, columns_read: Vec<String>, payload_rows: usize }
fn scan_arrow(snapshot: &BackendSnapshot, read: &ArrowScan, cancel: &Cancellation) -> StorageFuture<ArrowPage>;
struct IndexDefinition { name: Identifier, version: u32, source: ColumnSource, columns: Vec<String> }
struct IndexRead { name: Identifier, lower: Option<Vec<u8>>, upper: Option<Vec<u8>>, prefix: Vec<u8>, descending: bool, after: Option<IndexSeek>, limits: ReadLimits, work: ReadLimits }
struct IndexSeek { snapshot: Vec<u8>, plan: [u8; 32], revision: u64, key: Vec<u8>, position: u64 }
struct IndexPage { entries: Vec<IndexEntry>, next: Option<IndexSeek>, complete: bool }
```

Reason: one object-safe `StorageBackend` contract with capability flags and free executor functions
selects native methods or an equivalent explicitly bounded fallback. Arrow/index capability
flags describe acceleration; atomic conditional writes and atomic projection publication are
required correctness primitives of a backend claiming the corresponding facility. A generic
executor computes projection output and delegates its atomic publication to the same backend;
it cannot synthesize atomicity from independent legacy writes. Backends without the primitive
refuse explicitly until migrated, rather than advertise an incorrect fallback. Fallback processes at most
`work.rows/work.bytes` source data per call; selective scans can return an empty, incomplete page
with an advancing source position. Missing indexes are errors; declarations are required even
for generic search. Ordered seek compares canonical typed key bytes and unique source position;
descending reverses both. Bounds are lower-inclusive/upper-exclusive; prefix applies to encoded
key bytes. Predicate bounds are finite numeric lower-inclusive/upper-exclusive; null and non-finite
values do not match. Unsupported field types are errors, never empty matches.

Snapshots retain an immutable cut until explicit release. Tokens are instance-scoped and expire
on close/reopen; reopened callers acquire a new snapshot at the retained durable cursor. A token
is checked with its exact cut on every read. Continuations bind snapshot, plan and definition
revision; changing direction/bounds/columns requires restarting. Old retained source cuts remain
readable; no automatic age-based expiry. Released, foreign, rebuilt-index and restarted tokens
fail explicitly. Source order is ascending admission position. Object sources select the latest revision per
(schema identity, object key) at that cut; record/stream sources retain all admitted samples. Page output and decoded working
sets have separate consumer-supplied byte limits; one oversize row/chunk refuses rather than
skipping it. Cancellation is cooperative, checked before I/O and between bounded chunks; reads
have no admission side effects. Backend errors distinguish unsupported, conflict, expired,
invalid input, exhausted work/memory and corrupt/incompatible durable data.

### 1.3 Projections and Frontiers

```rust,ignore
enum ProjectionTransform { Select(Vec<String>), TimeBuckets(TimeBucketPlan) }
struct ProjectionDefinition { name: Identifier, version: u32, source: ColumnSource, transform: ProjectionTransform }
struct ProjectionRead { name: Identifier, revision: u64, frontier: StorageCursor, after: usize, limits: ReadLimits, work: ReadLimits }
struct ProjectionPage { batch: RecordBatch, frontier: StorageCursor, revision: u64, changed_keys: Vec<Vec<u8>>, next: Option<usize> }
fn register_projection(definition: &ProjectionDefinition, rebuild: bool) -> StorageFuture<u64>;
fn advance_projection(snapshot: &BackendSnapshot, name: &Identifier, work: ReadLimits, cancel: &Cancellation) -> StorageFuture<u64>;
fn read_projection(read: &ProjectionRead) -> StorageFuture<ProjectionPage>;
```

Reason: explicit asynchronous catch-up avoids coupling ingestion to consumer query cost. Each
advance atomically commits output, changed keys, definition revision and exact durable source
frontier. Registration of identical definitions is idempotent; a changed definition requires an
explicit rebuild flag and a new revision. Reads require an exact published frontier and revision;
lag returns `NotReady`, never a newer or partial answer. Output is derived only from raw source
inputs, so dependencies are acyclic by construction. Replay reuses a durable published cut or
recomputes the bounded candidate after rollback; output is not published ahead of its checkpoint.
Prior revisions/cuts remain retained until explicit consumer retirement; registration/rebuild
cannot mutate them. Advance refuses when the complete selected source exceeds the supplied work
budget; it never silently truncates a materialization. Consumers choose catch-up concurrency.

### 1.4 Replaceable Time Buckets

```rust,ignore
struct TimeBucketPlan { origin_ns: i64, width_ns: NonZeroU64, time: String, value: String, identity: String, group: Vec<String>, max_samples: NonZeroUsize, max_bytes: NonZeroUsize }
// Output: key, count, first, last, min, max, sum, sum_squares, mean, m2,
// first_position, last_position, min_position, max_position, positions: Vec<u64>.
// Projection revision and changed_keys describe replacement, not append semantics.
enum Transform { /* Existing native transforms. */ TimeBuckets(TimeBucketPlan) }
```

Reason: buckets are a generic deterministic query transform. Bucket `k` contains
`[origin + k*width, origin + (k+1)*width)` using Euclidean division, including negative time.
Process corrections by logical identity: greatest source position wins at a cut, including
moving a corrected sample between buckets; duplicate identical positions are idempotent and
conflicting duplicates refuse. Sort contributions by `(source_time_ns, source_position)`;
first/last follow that order and min/max ties choose the earliest contribution. Null/non-finite
values and invalid timestamps refuse. Empty buckets are absent. All contributing source positions are retained in source-time order. Mean and m2 use ordered
Welford accumulation; sum and sum-of-squares use ordered finite floating point accumulation
and refuse overflow. Changed keys are the sorted
union of changed/removed bucket keys compared with the preceding published cut in that revision;
these keys are retained with that cut. Old cuts retain old values. The native domain transform and its bounded storage executor are implemented here; wire
mapping and consumer adoption remain deferred. Native-to-wire conversion refuses this variant.

### 1.5 Conditional Objects

```rust,ignore
enum CompareExchange { Applied(ObjectReceipt), Conflict { current: Option<ObjectVersionToken> } }
struct ObjectExchange { expected: Option<ObjectVersionToken>, schema: StorageSchema, value: ObjectEntry }
fn compare_exchange(exchange: &ObjectExchange) -> StorageFuture<CompareExchange>;
async fn compare_exchange<T: StoredObject>(
    &self, expected: Option<ObjectVersionToken>, value: &T,
) -> Result<CompareExchange>; // StorageManager; validates, admits, then awaits the backend outcome.
```

Reason: the backend performs the check against the current committed object, not a read
snapshot. `None` is create-if-absent; a token is scoped to store history and to the object's
schema identity/key by lookup. A mismatch has no write or index effect and returns the current
token (or absent). A conflict consumes its admitted position with a durable outcome receipt but changes no object
or derived index. An accepted position identifies the immutable request for replay; an identical
retry returns the same receipt even after later writers/restart, while changed bytes/expectation
at that position conflict. Admission position allocation and durability remain distinct; no
cross-method transaction is implied. The SQLite reference atomically writes object, revision,
receipt and derived index effects in one immediate transaction with full synchronous durability.
Cancellation before admission prevents writes; after transaction admission a dropped waiter
cannot retract the committed request and retry must use its original identity.

### 1.6 Reference Backend

```toml
[features]
sqlite = ["dep:rusqlite"]
# Workspace dependency; the storage package inherits this optionally.
[workspace.dependencies]
rusqlite = { version = "=0.40.2", default-features = false, features = ["bundled"] }
# Existing Arrow 60.1 IPC/array/select dependencies are reused.
```

Reason: `harmos_storage::sqlite::SqliteBackend` implements the existing backend plus all five
extensions. SQLite holds typed durable rows, metadata, receipts, index entries, projection
outputs and separately encoded Arrow leaf-column chunks. Scans fetch only selected/predicate/
position columns and use chunk position bounds before decoding; they never reconstruct payload
rows on the native path. Ingestion builds column chunks once from stored schema graphs. No
external service, concrete connection escape or product-specific key is exposed. An explicit
format identity/version is checked before use; incompatible data is refused with instructions
to back up and reset deliberately, never reset automatically. SQLite in-memory mode is the
reference mechanics fixture; the existing deterministic `tests/support/storage.rs` additionally
exercises generic fallback and controlled cancellation. The file recovery backend stays intact.

### 1.7 Locked Workload and Budgets

Before interface implementation: `mise run bench -- --storage --filter baseline` builds only
`integration-multi-day` in Cargo's bench profile and preserves native Criterion artifacts under
`target/benchmarks/storage/criterion/`. Baseline input uses 2,000,000 records over exactly three
days, 24 interleaved plots, deterministic `value=(logical*7919)%65521`, and a late sample every
4096 records (logical position minus 2048). Payload fields are `time_ns:i64`, `plot:u32`,
`value:f64`; record identities and admissions are deterministic. Pages are 16,384 rows/8 MiB;
window output is 4096 rows starting at one third of the source and the immediately following
window. Baseline measures existing projected row scans (full/first/next); new facilities are
measured separately, with result parity checked outside timing. Criterion: 10 samples, 1-second
warmup, 1-second requested measurement; preserve actual extended timing when Criterion needs it.
Raw compiler metadata and native samples are retained. Workload construction is outside timing.

Acceptance budgets are locked before candidate measurements: existing path medians at most
baseline +20%; native bounded-window and projection reads target at most 1 second per million
source samples (2 seconds for this input); seek next-page work must not grow with prior output.
Native full scans, ingest, catch-up, restart/replay and contention are measured, not inferred from
these targets. Consumer finalization/report numbers and native Linux proof remain in tasks 3.x.
No later result may loosen a budget to pass. Local concurrent-writer CPU/I/O contention must be
disclosed with measurements; hosted Nightly remains the authoritative regression environment.


The original repeated baseline attempt exposed whole-snapshot cloning on every test-backend
page and was interrupted before measurements. The baseline selection therefore records one
timed traversal per case in raw stdout (`target/benchmarks/storage/baseline.txt`); these are
single measurements, not medians. Repeated Criterion sampling remains available without the
baseline selector. Budgets are unchanged and remain provisional until hosted repeated proof.


## Windows Baseline Measurement

Captured before interface implementation on Windows x86_64, AMD Ryzen 9 5950X (32 logical
processors), 85,817,380,864 bytes physical RAM, Rust 1.99.1. Storage source is unchanged from
`a7583836`; benchmark and task source are the local `bb0e4b29` checkpoint, with the concurrent
guest worker's generated protocol reference correction needed for compilation. Other work in
the checkout ran concurrently, so these are local single traversals, not authoritative medians.

| Existing Generic Row Query | Rows Returned | Measured Seconds | Proposed +20% Ceiling (Seconds) |
| --- | ---: | ---: | ---: |
| Full Projection | 2,000,000 | 215.787614 | 258.945137 |
| First Window | 4,096 | 79.456927 | 95.348312 |
| Next Window | 4,096 | 82.026233 | 98.431480 |

Command: `mise run bench -- --storage --filter baseline` (successful). Raw output:
`target/benchmarks/storage/baseline-locked.txt`; compiler metadata: its sibling `compiler-baseline.json`.
All three row counts and sums agree with the deterministic generator outside timing. These
measurements include retained-snapshot copying in the existing test backend and are not a
previous durable-backend benchmark. The proposed native bounded-window/projection ceiling is
2 seconds for this 2-million-sample workload. It is independent of the slow existing-path
ceilings and will not be loosened to accept a candidate. These budgets are proposals from this
baseline; cross-platform repeated and consumer-specific measurements remain tasks 3.x.

Native plans additionally carry an initial exclusive source position (`start: u64`) and an
optional exact integer/timestamp window, separate from seek continuation:

```rust,ignore
struct TimeBounds { column: String, lower_ns: i64, upper_ns: i64 }
// ArrowScan: start: u64, time: Option<TimeBounds>, after: Option<ArrowSeek>.
// start and the time window participate in the plan identity; bounds are half-open.
```

Reason: initial position seeks avoid scanning a known prefix, while integer time bounds avoid
rounding nanoseconds through floating point predicates. The fallback applies identical bounds.

README and `docs/storage.md` describe the native capabilities and generated consumer contracts. The focused benchmark commands are documented in `docs/tooling.md`.

### Consumer Adoption

The guest consolidation frees this inventory under the combined `grpc-plugins` implementing
marker. Source follows this dependency order; targeted proof is tracked separately in tasks.md.

1. `proto/harmos/v1/storage.proto` defines read/work limits, capabilities, schema sources,
   numeric/time bounds, Arrow plans/pages, ordered indexes, projections, grouped buckets,
   saved-cut handles, atomic object outcomes and the corresponding Storage RPCs. Existing
   payload encodings, files, receipts and ordinary query contracts remain supported.
2. The protocol generator refreshes `storage.rs`, reflection, the complete descriptor graph,
   the projected guest graph and Schematic descriptors together. `storage_helpers.rs` exposes
   the shared descriptor graph and reads structured storage failure metadata. Generated
   request decoding accounts for new buffers through the existing descriptor-driven allocator.
3. Domain query protobuf conversion maps `TimeBuckets` in both directions with checked
   nonzero limits and integer conversions; the superseded explicit refusal is deleted.
   Native plan conversions and bounded Arrow IPC validation live in storage's `query/protobuf.rs`.
4. Operations retain ordinary `query.rs` behavior and add saved snapshot acquisition/release,
   bounded Arrow/index pages, projection registration, advancement, exact-frontier inspection
   and reads. Saved reads check grants, bind the exact cut and release acquired leases on
   cancellation/failure. Successful pages keep the saved handle until explicit release.
5. `exchange.rs` and `write_objects.rs` retain streamed batch admission and accepted/durable
   ordering. There was no read-then-write precondition to remove. The separate conditional
   handler uses decoded, catalog-validated atomic manager admission and durable receipt replay.
6. `storage/{mod.rs,registration.rs}` export all operations, source-scoped grants and stable
   failure kinds. Expected version mismatches are typed successful conflict outcomes.
7. Runtime retains its storage re-exports and manager `Deref`. `Builder::storage` composes
   the saved-cut registry using the consumer's active-query capacity. Backend construction
   remains consumer-owned; no automatic reference-backend selection or second facade exists.
8. Rust SDK storage contracts use only protocol and the existing managed client/context.
   Python's existing generator includes storage modules; C# and C++ build-generated clients
   use the same proto. The client codec and plugin-registration implementation are unchanged.
9. Schematic's `services::queries` composes optional SQLite queries beside the existing JSONL
   stream/recovery adapters. The saved-preview journey uses generated SDK requests for
   saved scans, applied/conflict outcomes and release. `services/guest.rs` and
   `cli/walkthrough.rs` have no superseded storage interface calls; their existing generated
   guest and checkpoint flows need no fabricated migration. Application JSONL ownership stays
   in `examples/schematic/storage`, with no new trait implementation there.
10. README and `docs/{storage,product-guide,tooling,schematic}.md` describe generated clients,
    required bounds, source grants, exact frontiers, failure kinds and the example composition.

The query-backend implementers remain the SQLite reference and deterministic storage fixture.
Consumer adoption adds no external backend or product-specific core API. Completion of this
slice does not claim combined source closure, Linux proof, publication or consumer extraction.

The deterministic test backend now retains an immutable `Arc<Snapshot>` per token and checks
its exact cut; page reads no longer clone the full saved image. This preserves snapshot contents
and release semantics while removing fixture work that dominated the baseline. SQLite snapshots
use fresh 256-bit opaque tokens and an instance-owned live registry; token creation performs no
database write. Reopening and independent owners cannot accidentally accept an old token. Native column chunks carry exact integer time bounds
and finite float bounds; pruning advances source continuation without fetching payload bytes.


Index registration takes the definition, an explicit rebuild decision, consumer work limits and
cancellation. SQLite builds existing keys under one immediate transaction and maintains future
keys with the source write. Canonical scalar bytes plus source position form the total order;
null sorts first, then boolean, signed, unsigned, finite float, text and datetime type tags.
Within a declared field type its natural order is preserved; floating signed zero is normalized.
Text escapes NUL before its terminator. Missing declarations return `Error::IndexNotFound`.
The bounded fallback scans at most the supplied source work and retains only one output page
plus its lookahead; it refuses rather than return an incomplete globally ordered answer.


Projection reads carry both `limits: ReadLimits` for returned arrays/changed-key buffers and
`work: ReadLimits` for decoding one durable chunk. Frontiers advance monotonically per revision;
an already published cut is idempotent, but a missing older cut cannot be published behind a
newer frontier. Old published cuts and revisions remain readable until explicit retirement.
The reference database has one backend owner enforced by a native file lease for the instance
lifetime; concurrent callers share that owner. This makes snapshot retention authoritative
across pruning and prevents independent backend instances from expiring each other's cuts.


Time buckets use optional generic grouping fields so independent series cannot be combined
accidentally. Logical identity is unique within the selected source (consumers scope it across
groups); a correction may move that identity between groups/buckets. Changed keys are canonical
binary `(group scalar values..., signed bucket ordinal)` keys, not unscoped integer ordinals.
Each bucket carries a SHA-256 content revision over its statistics and contributing positions;
the projection's definition revision plus source frontier identifies the atomic materialization.
Consumer-supplied `max_samples` and `max_bytes` bound generic transform state. This is the clean
replacement for the provisional ungrouped key shape; no consumer has adopted that shape.
Native domain contracts gain `TimeBucketPlan` and `Transform::TimeBuckets`. Their native
to-wire conversion is fallible and maps the generated variant with checked nonzero limits;
no transform is silently omitted.


Projection publication additionally compares its expected preceding frontier atomically. Two
concurrent catch-up attempts cannot publish deltas relative to different predecessors silently;
the stale candidate conflicts and can be recomputed. Binary changed-key pages use a shared
ordinal over output rows and changed keys, including key-only tail pages for removed buckets.
Every supplied time-bucket input row must have finite values, including superseded versions;
invalid input refuses advancement and preserves the last published cut.


Conditional writes use one `ObjectExchange { expected, schema, value }` backend request. The
schema descriptor and value own consecutive admission positions; application commits both and
all object index effects atomically, while a conflict commits only its outcome and frontier.
This prevents schema rows escaping from failed conditional writes. Durable receipts retain a
canonical SHA-256 request fingerprint and outcome; an identical accepted position replays the
original outcome after later writes/restart. `retire_object_receipts(through)` is explicit
consumer retention: retired retries fail with a gap instead of being reapplied. Consumers retire
only outcomes they no longer need; a retention call does not create a cross-method transaction.
The typed manager supplies ordinary and mutation-id `compare_exchange_once` entry points and
waits for the durable outcome; cancellation after admission cannot retract queued work.


`StorageManager::snapshot` now returns the public owned `Snapshot`, with `at`, `scan_arrow`,
`scan_index` and `advance_projection`. Manager methods own index/projection registration,
frontier inspection, reads and retirement. Operations can use these generic resources without
access to a concrete backend or connection. Snapshot drop retains the existing worker-owned
release path. Bucket computation runs on the blocking executor with a child cancellation token;
dropping its awaiting future cancels the child without cancelling a caller's shared parent.

Object retention records exact reclaimed positions and returns `Gap` for exact/revision reads
that would cross them. Fully dead object chunks are reclaimed after live snapshots release.
Future-frontier retirement is refused. Native snapshot tokens are random 256-bit owner tokens;
the deterministic fixture encodes its instance scope and counter. Incompatible roots are
validated before journal-mode changes, preserving their bytes when refused.

Storage-side 2.6 covers both backends, the public manager/snapshot facade, retained files and
mutation receipts, token ownership and bounded generic execution. Storage-side 2.7 covers
`docs/storage.md`, README capabilities and tooling commands. The remaining adoption above
keeps the combined task boxes open; this is not combined SOURCE COMPLETE or delivery.

## Native Windows Measurements

Command: `mise run bench -- --storage --filter native` (successful), same Windows machine,
compiler and deterministic two-million-sample input as the locked baseline. Other writer work
continued concurrently; SQLite setup and result verification are outside timing. These are
local measurements, not hosted or consumer end-to-end claims. The full scan selects one value
column; native windows start at the identical exclusive source positions used by baseline offsets.
The reference physical layout is 16,384 rows/8 MiB. Selected result pages contain at most 4096
rows. Projection verification checks the independent generator's count/sum; index verification
sorts independently generated `(value, position)` pairs outside timing.

| Native Case | Median Seconds (10 Criterion Samples) | Mean Seconds |
| --- | ---: | ---: |
| Full Projection, 2,000,000 Rows | 0.249443 | 0.253001 |
| First Window, 4096 Rows | 0.000537 | 0.000545 |
| Next Window, 4096 Rows | 0.001180 | 0.001184 |
| Durable Projection Window, 4096 Rows | 0.000353 | 0.000360 |
| Published Projection Replay | 0.000193 | 0.000196 |
| Indexed Window, 4096 Entries | 0.093912 | 0.095857 |

Single traversals: ingest 55.001403 seconds; initial projection catch-up 0.233137 seconds;
index construction 47.170998 seconds. First full scan 0.276647 seconds and 128,359,376 fetched
column bytes across its pages; first/next windows 262,992/525,984 bytes respectively. Each native
scan reconstructed zero payload rows. The next window spans a physical chunk boundary and
therefore fetches two chunks. These byte counts are measured encoded reads, not unique disk
I/O or allocations. TempDir removes the owned database after close. The prior stdout/compiler capture was
replaced by the continuation below; native Criterion samples/estimates remain under
`criterion/integration_multi_day_native/`.
The baseline compiler capture is preserved separately as `compiler-baseline.json`.

The previously proposed 2-second native read ceiling remains unchanged and these reads meet it.
The baseline +20% ceilings also remain unchanged; no existing-path median regression conclusion
is claimed from native-path measurements. The continuation below adds bucket catch-up/contention, memory/allocation, restart and
cancellation refusal costs on Windows. Native Linux and consumer workloads remain open.

Minimal forced external edits, inspected clean before editing: domain
`crates/harmos-domain/src/model/storage/query/transform/mod.rs` and `query/protobuf.rs`;
`crates/harmos-runtime/src/error.rs` maps missing indexes to `NotRegistered`; runtime storage
stories `generated_query_batches.rs`, `operation_plugins.rs` and `operation_schemas.rs` use
fallible query conversion for existing supported transforms. These are compilation migrations,
not adoption of the new wire/storage capabilities. No other worker's dirty paths were edited.

## Windows Benchmark Continuation

Measured on the same Windows machine and locked source shape with storage source checkpoint
`8bb72660`, using `mise run bench -- --storage --filter baseline` followed by
`mise run bench -- --storage --filter native`. Both succeed. Guest changes and other workstation
CPU/I/O work continued concurrently, including output cleanup. These local observations do not
replace native Linux or authoritative hosted repeated proof. The public publication harness
uses ten direct read/replay observations after one-second warmup, with the middle two averaged;
mutating/build/first cases run once and contention reports the median of 64 eight-writer rounds.
It preserves the locked input, queries, page/work sizes and numerical budgets. These direct
samples are not the earlier Criterion estimates. Rust allocation instrumentation is active.

| Existing Row Path | Locked Baseline (s) | Current Observation (s) | Unchanged +20% Ceiling (s) |
| --- | ---: | ---: | ---: |
| Full | 215.787614 | 10.496275 | 258.945137 |
| First Window | 79.456927 | 4.576124 | 95.348312 |
| Next Window | 82.026233 | 6.511515 | 98.431480 |

All existing-path observations are below their ceilings. The native window and projection
read ceiling remains 2 seconds; every measured bounded native read below meets it. Existing
row-path captures are single traversals, so no existing-path median regression claim is made.

| Stable Case | Seconds | Rust Allocation Calls | Total Allocated Bytes | Incremental Peak Rust Heap Bytes |
| --- | ---: | ---: | ---: | ---: |
| `storage.bucket.advance.2m` | 7.810964400 | 14500885 | 1031565782 | 321520855 |
| `storage.bucket.replace.2m` | 38.974153600 | 15058913 | 2150948022 | 321829859 |
| `storage.close.2m` | 0.000037000 | 2 | 392 | 392 |
| `storage.index.build.2m` | 79.244174900 | 6053642 | 580467960 | 803576 |
| `storage.index.deep.2m` | 0.120473650 | 4152 | 958504 | 432517 |
| `storage.index.next.2m` | 0.119563550 | 4152 | 958504 | 432517 |
| `storage.index.read.2m` | 0.109404200 | 4146 | 957664 | 432445 |
| `storage.ingest.2m` | 89.927627300 | 20114105 | 4394365279 | 6882275 |
| `storage.object.contend.8` | 0.004551950 | 931 | 76435.5 | 24504 |
| `storage.object.replay.512` | 0.000037000 | 18 | 1694 | 1694 |
| `storage.object.restart_replay.512` | 0.000034300 | 18 | 1694 | 1694 |
| `storage.projection.advance.2m` | 0.453780800 | 58891 | 181177609 | 16658768 |
| `storage.projection.cancel.2m` | 0.000000300 | 4 | 49 | 49 |
| `storage.projection.read.2m` | 0.000481550 | 129 | 1548199 | 264868 |
| `storage.projection.replay.2m` | 0.000172800 | 49 | 3991 | 1957 |
| `storage.projection.restart_replay.2m` | 0.000261600 | 54 | 4072 | 2038 |
| `storage.restart.2m` | 0.004202000 | 15 | 1025 | 436 |
| `storage.scan.cancel.2m` | 0.000000350 | 3 | 43 | 43 |
| `storage.scan.read.2m` | 0.494258600 | 214667 | 338019683 | 411560 |
| `storage.scan.read.2m.first` | 0.712549900 | 214667 | 338019683 | 411560 |
| `storage.stop.2m` | 0.023742600 | 0 | 0 | 0 |
| `storage.window.next.2m` | 0.001858600 | 875 | 1269755 | 411560 |
| `storage.window.next.2m.first` | 0.001216700 | 875 | 1269755 | 411560 |
| `storage.window.read.2m` | 0.000574400 | 438 | 692035 | 411496 |
| `storage.window.read.2m.first` | 0.001320900 | 438 | 692035 | 411496 |

Measured throughput: ingest 22,240 rows/s; full scan 4,046,465 rows/s; select projection
advancement 4,407,414 source rows/s; initial buckets 256,050 source rows/s; contended
conditional requests 1,757 attempts/s. The contention rate is eight divided by the median
round duration, including seven durable conflicts, rather than eight successful writes.

The initial deep seek took 0.473931 seconds versus 0.189066 for the next page. The optional
seek predicate allowed scanning from the prefix lower bound. Merely making the seek predicate
explicit was insufficient: SQLite still chose the independent prefix bound. The final query
passes one merged ascending lower bound (maximum of request, prefix and continuation key),
plus the exclusive key/position tie-breaker. Descending reads retain an explicit upper seek
bound. No budget was changed. In the final run, next/deep seeks have identical 4152 Rust
allocations, 958504 allocated bytes and 432517 incremental heap bytes; time differs by less
than 1%, rather than growing with the skipped million entries. Forward/reverse saved-cut
journeys and independently sorted full-workload expectations both pass.

Encoded bytes fetched: full scan 128,359,376; first window 262,992; next window 525,984.
Native payload rows reconstructed are zero in all three cases. These are column read counters,
not unique physical device I/O. Whole-process resident high water is 681,463,808 bytes,
including input preparation and SQLite native allocations. Per-region heap counters cover Rust
allocations only; they do not count SQLite's C heap. No memory ceiling was invented or tuned.

SQLite uses WAL with FULL synchronization. Pre-generated ingest, projection/bucket publication
and all conditional attempts include their real commit/durability cost. The native stop case
measures owner/connection disposal separately from logical close; restart creates a new owner.
Every contention round has exactly one applied outcome, and the final winning object and
original replay outcome are verified before and after restart. Independent grouped count/sum
checks verify initial, corrected, saved and reopened bucket outputs outside timing. Projection
frontier/replay and indexed source positions are likewise verified outside timing. Cancellation
numbers measure already-signalled refusal, not active non-cooperative I/O cancellation latency.
No wall-clock-dependent correctness test is added.

Raw source/compiler evidence and samples are under `target/benchmarks/storage/`:
`baseline-locked.txt`/`compiler-baseline.json` preserve the original baseline;
`baseline-current.txt`/`compiler-existing-path.json` preserve the current row path;
`candidate.txt` includes every native sample and result counter, with `compiler.json` for its
executable. `target/benchmarks/benchmarks-storage-windows.json` contains the 25 timing cases
and their 112 measurements, including units, backend, platform suite and source revision.
Earlier unsuccessful seek-performance candidates remain named separately; their numbers are
not substituted for the final run. Temporary databases are removed after closing their owners.

Nightly now runs the reference on Linux and Windows, uploads `results-bench_storage_*`, and
its existing report collector places these events in the release `benchmarks.json` and report
archive. Focused producer tests exercise that same staging path with both platform suites.
No push or hosted publication occurred here. Public `bench_support::run_with_reopen` owns all
case names, generation, plans, sample counts and correctness checks; a consumer supplies only
its backend opener and installs the tracking allocator. `docs/storage.md` documents the units
and JSON adapter. The large executable is excluded from ordinary benchmark execution; the
16,385-row native/fallback boundary journey remains an ordinary integration test.

## Targeted Proof and Remaining Gates

The continuation adds one native/fallback boundary journey and extends the existing seek
journey with reverse continuation/tie checks. Both pass on Windows. The three focused
benchmark-producer tests pass, including mixed-platform aggregation into the release inventory,
non-finite refusal, retention after runner failure and exclusion of the large executable from
ordinary integration benchmark execution. No ordinary test contains wall-clock assertions.

Consumer source and focused proof are tracked in tasks.md. Protocol drift, malformed-plan
refusal, runtime native-capability adoption, retained streamed writes/ordinary queries,
Schematic SDK adoption and all four SDK conformance suites have focused Windows proof.
Full native Linux and hosted validation remain separate obligations. No push is authorized.
This slice does not declare combined SOURCE COMPLETE for the other branch changes.

### Adoption Call-Site Additions

Consumer adoption adds `query/protobuf.rs`, decoded `StorageManager::compare_exchange_value_once`
in `objects.rs`, and `query/manager.rs` accessors for query capacity and index/projection definitions.
Source-scoped authorization of retained projection revisions requires `projection_definition_at`
on `StorageBackend`, `ProjectionCatalog`, the SQLite adapter and deterministic fixture; reads check
the definition at the requested revision, not the active revision after a source-changing rebuild.
These are the adoption edits in storage source; performance internals remain with their owner.
Saved snapshot handles use the consumer's active-query capacity and explicit release. Rust SDK
contracts use existing managed invocations; generated service stubs expose the same unary pages.

Snapshot page leases reserve capacity before backend I/O and release that capacity after retained
reads settle. Revision-checked advancement adds `Snapshot::advance_projection_at` in `snapshot.rs`
and the corresponding executor in `query/projection.rs`; named index reads verify the unchanged
definition revision before returning data. These prevent source-changing rebuild races across
the operation authorization boundary.

The example SDK journey found that `operations/wire.rs` dispatched direct IDs only for plugins.
Native protobuf factories now resolve by their declared ID through the same authorization,
preparation, grant and settlement path; owner-qualified routing remains available. This is
required for SDK storage requests called through the existing example `services::call` helper.

| Storage Source Path | Adoption Addition |
| --- | --- |
| `src/backend.rs` | `projection_definition_at` revision lookup primitive |
| `src/objects.rs` | `compare_exchange_value_once` and shared decoded conditional admission |
| `src/query.rs` | Export native protobuf conversions |
| `src/query/protobuf.rs` | Native plans/pages, checked conversions and bounded IPC validation |
| `src/query/manager.rs` | `query_capacity`, `index_definition`, `projection_definition`, `projection_definition_at` |
| `src/query/projection.rs` | `ProjectionCatalog::definition_at` and revision-checked advancement |
| `src/snapshot.rs` | `Snapshot::advance_projection_at` |
| `src/sqlite/mod.rs` | Implement the revision-definition lookup primitive |
| `src/sqlite/projection.rs` | Read an immutable definition by name and revision |

All paths in this table are relative to `crates/harmos-storage`; the fixture implements the
same lookup in `tests/support/storage.rs`. Index construction/pages, the row scan path and
bucket-replacement performance internals are unchanged by adoption.
