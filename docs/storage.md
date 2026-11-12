# Storage Interfaces

`harmos-storage` owns typed persistence, bounded Arrow scans, indexes, durable
projections and atomic conditional objects. Consumers choose capacity, concurrency,
physical chunk sizes, work budgets and response limits. Storage does not expose
product keys or a database connection.

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
let page = snapshot.scan_arrow(&read, &cancel).await?;
let ordered = snapshot.scan_index(&index_read, &cancel).await?;
```

A manager snapshot captures a saved admission cut, not pending writes. It retains
its backend token until the existing worker release path processes its drop.
Tokens bind an owner, store history and exact cut; release, close or reopen makes
that token unusable. Later writes do not alter the retained answer. Readers must
release snapshots before expecting retention to reclaim their rows.

`StorageCapabilities` advertises `native_arrow`, `native_index`,
`durable_projections` and `conditional_objects`. `scan_arrow` and `scan_index`
choose native execution or the common bounded executor. A backend without native
Arrow scans supplies typed row reads; it still receives the same predicates and
saved-cut semantics. A backend without a native index stores its declarations and
uses bounded source scanning. It refuses insufficient work rather than returning
an incomplete globally sorted answer. Durable projections require an atomic
publication primitive; conditional objects require an atomic compare/exchange
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

`Cancellation` is cooperative across source reads, decoding, sorting and bucket
work. Cancellation does not roll back an already committed write. CPU bucket work
runs on the blocking executor; dropping its awaiting future cancels its child
without cancelling the caller's shared token.

## Ordered Indexes

```rust,ignore
storage.register_index(&definition, rebuild, work, &cancel).await?;
```

An `IndexDefinition` owns a name, nonzero semantic version, source and ordered
columns. Registration is idempotent for the same definition. A changed definition
requires explicit rebuild and creates a new revision. SQLite builds history under
one immediate transaction and updates entries in the source write transaction.
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
let revision = storage.register_projection(&definition, rebuild).await?;
snapshot.advance_projection(&definition.name, work, &cancel).await?;
let frontier = storage.projection_frontier(&definition.name, revision).await?;
let status = storage.projection_status(&definition.name, revision, snapshot.at()).await?;
let page = storage.read_projection(&read).await?;
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
the same bounded reduction. The current protobuf transform mapping explicitly
refuses this variant pending the separate consumer adoption pass.

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

Accepted positions replay their original success or conflict after later writes
and restart. Consumers explicitly retire receipts with
`retire_object_receipts(through)`; retrying a retired outcome reports `Gap` rather
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

Native Rust storage entry points are available now. Protocol messages/mappings,
built-in operation handlers, runtime composition, generated SDK client surfaces
and example composition still need to adopt them. The locked tree's two query-backend
implementers are complete; example journal/stream callbacks are separate consumer owners. In particular,
the operation-level object exchange must use the atomic primitive. See the locked
[next-pass inventory](../openspec/changes/storage-interface/design.md#consumer-adoption).

The focused Windows workload contains 2,000,000 samples across three days and 24
plots. Run `mise run bench -- --storage --filter baseline` or `--filter native`;
raw output and Criterion artifacts remain under `target/benchmarks/storage/`.
Correctness checks stay outside timing. Local measurements and proposed budgets
are recorded in the change; Linux and consumer performance proof remain open.
