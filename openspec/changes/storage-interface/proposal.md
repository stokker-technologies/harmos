# Storage Interface

## Why

The storage interface returns row payloads for Arrow reconstruction and lacks indexed seek
reads, durable projection frontiers, replaceable time buckets and expected-revision writes.
Applications need these generic facilities without exposing concrete backend APIs to operations.

## What Changes

- Add native columnar scans with projection, pruning and bounded continuation.
- Add indexed search with deterministic ordered seek pages.
- Add durable materialized projections with readable frontiers and revision invalidation.
- Add `Transform::TimeBuckets` with late-sample replacement and changed-key visibility.
- Add atomic conditional object updates.
- Define backend obligations, a memory reference backend and conformance kit, retaining existing row, file,
  recovery and durability contracts. Prove correctness and measure before/after performance,
  including multi-day data; no performance improvement is assumed.

## Scope and Order

The generic interfaces and memory implementation are landed. A product owns SQLite,
persistence extraction and durable backend proof. It reaches storage only through
Harmos operations; `operation-boundary` owns the remaining public facade migration.
Harmos retains backend authoring and the importable conformance kit.

Execution/step-range statistics, history filters and tokens, finalization/seals, report capture,
artifact inventory and fresh-root refusal remain a product behaviour. Logs, support bundles,
exports/report spools, plugin installation, desktop root lease/backup, client pairing/download
files, command-line and build tooling remain with their owners. This change adds no general
all-disk ownership API, storage migration policy or cross-method transaction assumption.

## Capabilities

### New Capabilities

- `storage-extensions`: the five generic capabilities, backend semantics and benchmark proof.

## Impact

Interface owners are `crates/harmos-storage/src/{backend.rs,objects.rs,query/}` and non-wire
storage query contracts in `crates/harmos-domain/src/model/storage/`. Update affected built-in
operations, reference/test backends, examples, storage guidance and conformance together.
`harmos-protocol` owns any new wire shapes. Caller budgets, concurrency and page sizes remain
consumer policy. The SDK remains independent of host storage implementation.
