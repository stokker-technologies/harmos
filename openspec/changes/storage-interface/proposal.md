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
- Define backend obligations and a durable reference backend, retaining existing row, file,
  recovery and durability contracts. Prove correctness and measure before/after performance,
  including multi-day data; no performance improvement is assumed.

## Scope and Order

This is a separate change after the last gRPC migration wave, not part of `grpc-plugins`.
Design and implement the generic Harmos interfaces first. Dokime consumes the published Harmos
revision in its own change named `storage-interface`, then extracts `dokime-storage` from
`crates/dokime-runtime/src/storage/` and `src/runtime/storage/`. Its persisted shapes live there;
no Dokime domain crate or reverse dependency on runtime is introduced.

Execution/step-range statistics, history filters and tokens, finalization/seals, report capture,
artifact inventory and fresh-root refusal remain Dokime behaviour. Logs, support bundles,
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
