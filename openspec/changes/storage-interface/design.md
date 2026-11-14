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
these keys are retained with that cut. Old cuts retain old values.

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

## Proposed Nightly Budget Scheme

Owner decision, 2026-11-13: record the following scheme as proposed. The first hosted
run confirms or adjusts the proposed numbers. Private-repository hosted runners have
2 vCPUs and 8 GB of RAM. Report every proposed absolute ceiling on every Nightly;
none is a wall-clock assertion in a correctness test.

| Workload | Proposed Absolute Ceiling |
| --- | --- |
| Bounded Page or Projection Page | Proposed: 20–30 ms |
| Full Scan of 2,000,000 Rows | Proposed: 2 s |
| Small Unary gRPC Call | Proposed: p95 2 ms |
| Rust or C++ Plugin Ready, Linux | Proposed: 100 ms |
| Rust or C++ Plugin Ready, Windows | Proposed: 300 ms |
| Python Plugin Ready | Proposed: 1–2 s |

Relative regression detection builds base and head and runs both in one job on the
same runner, comparing medians. Proposed margins: +10% for serial CPU, +15% for warm
reads and +30% for cold and fsync-bound cases. Pure CPU kernels such as reduction and
decoding additionally report instruction counts, flagged at a proposed +5% increase.
Hosted peak throughput and cold-disk latency are tracked only, never gated.
Nightly publication remains ungated by benchmarks.

Keep an append-only history beside existing results in `target/benchmarks/`, with
CPU model, runner image version and base/head revisions. Use a rolling baseline of
twenty Nightly runs for trend warnings; retain paired base/head comparison for
regression detection. Recording this proposal does not claim reporting automation
or hosted proof is complete.

## Baseline Regression Budgets

| Existing Generic Row Query | Proposed +20% Ceiling (Seconds) |
| --- | ---: |
| Full Projection | 258.945137 |
| First Window | 95.348312 |
| Next Window | 98.431480 |

## Consumer Adoption

The query-backend implementers remain the SQLite reference and deterministic storage fixture.
Consumer adoption adds no external backend or product-specific core API.

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

## Adoption Call-Site Additions

Consumer adoption adds `query/protobuf.rs`, decoded `StorageManager::compare_exchange_value_once`
in `objects.rs`, and `query/manager.rs` accessors for query capacity and index/projection definitions.
Source-scoped authorization of retained projection revisions requires `projection_definition_at`
on `StorageBackend`, `ProjectionCatalog`, the SQLite adapter and deterministic fixture; reads check
the definition at the requested revision, not the active revision after a source-changing rebuild.
Saved snapshot handles use the consumer's active-query capacity and explicit release. Rust SDK
contracts use existing managed invocations; generated service stubs expose the same unary pages.

Snapshot page leases reserve capacity before backend I/O and release that capacity after retained
reads settle. Revision-checked advancement adds `Snapshot::advance_projection_at` in `snapshot.rs`
and the corresponding executor in `query/projection.rs`; named index reads verify the unchanged
definition revision before returning data. These prevent source-changing rebuild races across
the operation authorization boundary.

The integrated native fast path uses the additive default backend hook
`advance_projection_native_at(snapshot, name, revision, work, cancel)`. SQLite checks the
expected revision while holding the same state lock as bucket publication; unsupported backends
retain the revision-checked shared executor. The unversioned native hook remains unchanged.
The bucket replacement journey proves refusal after rebuild without publishing a new frontier,
then advances the matching revision through both native and fallback paths.

Native protobuf factories now resolve by their declared ID through the same authorization,
preparation, grant and settlement path; owner-qualified routing remains available. This is
required for SDK storage requests called through the existing example `services::call` helper.

| Storage Source Path | Adoption Addition |
| --- | --- |
| `src/backend.rs` | `projection_definition_at` revision lookup and default revision-checked native advancement hook |
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

## Performance Repair Contracts

Immutable bucket output
versions preserve saved cuts. Checkpoint output cardinality survives retirement
of every readable cut, and later generic publication invalidates membership
before native catch-up resumes.

Composite key/position bounds
perform one seek followed by sequential entries, including reverse continuation
and ordering ties. Record/stream probes use the covering index; objects retain
latest-at-cut lookup.

Required numeric projection compiles the selected field once,
uses the canonical Postcard reader for every input value, validates unselected
fields and rejects trailing data.

True predicate cases use `start=1` over the entire saved history, with an exact
half-open `payload.time_ns` interval from generated rows 666,666 to 670,762.
`storage.window.time.2m` has only that time predicate;
`storage.window.numeric.2m` additionally requires `10000 <= payload.value < 20000`.

Existing public signatures are unchanged. Additive default backend hooks are
`work()`, `advance_projection_native(snapshot, name, work, cancel)`,
`seek_records(snapshot, selection, offset)` and
`read_record_views(snapshot, selection, page)`; new public values are `StorageWork`
and immutable `RecordView`. Existing implementers need no call-site changes.
The view constructor owns exact size checking; private fields prevent forged
budget metadata. Readers still check history, order, cut, selection and page
limits. Typed owned-row reads remain the fallback.
