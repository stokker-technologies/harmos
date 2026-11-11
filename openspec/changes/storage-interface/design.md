# Storage Interface Design

## Boundary and Sequence

```text
protocol <- reduced native storage contracts <- harmos-storage
                                               ^ operations <- runtime
Dokime operations -> dokime-storage -> Harmos storage interface
```

Begin after the last `grpc-plugins` wave in Harmos and Dokime. Settle the semantics below before
implementation, implement and prove Harmos, publish a usable revision, then Dokime adopts it in
its separate `storage-interface` change and extracts application persistence. Integration-only
local overrides may support coupled work; final validation uses the published upstream pin.
There is no storage extraction during the gRPC migration.

The interface already separates query snapshots from recovery snapshots, accepted writes from
durability, and independent backend writes from cross-method transactions. Preserve these
distinctions, existing schema/object/record/stream/file capabilities, mutation receipts and
replay-idempotent writes. Do not introduce product execution or cycle identifiers into Harmos.

## Proposed Rust Interface

The following are proposed signatures, not existing APIs. Supporting types are new. Their
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
