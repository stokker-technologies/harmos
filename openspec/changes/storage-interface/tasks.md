# Tasks

## 1. Interface Design

- [x] 1.1 Owner authorizes parallel work: Harmos host gRPC surfaces at a7583836 are complete; consumer wave pending; lock inventory for storage backend/object/query contracts, protocol mappings, operations, examples, backends, tests, benchmarks and storage guidance.
- [x] 1.2 Settle the proposed scan/index plan and page types, ordering, snapshot lifetime, capability negotiation, bounded fallback, cancellation and error semantics in design.md.
- [x] 1.3 Settle projection registration/versioning, readable frontier, source-commit coupling, crash replay, invalidation/rebuild and old-snapshot behaviour in design.md.
- [x] 1.4 Settle time-bucket boundaries, retained statistics/positions, corrections, late replacement, revision and changed-key retention semantics in design.md.
- [x] 1.5 Settle conditional-update token scope, absent-object semantics, object-safe backend primitive, conflict receipts, durability and atomic index effects in design.md.
- [x] 1.6 Lock the durable SQLite/Arrow reference backend module/feature, dependencies and capability obligations; retain the file recovery backend and bounded test backend as distinct owners.
- [x] 1.7 Before implementation, lock reproducible before/after workloads and numerical regression budgets; capture the baseline with deterministic multi-day data and millions of samples (Windows single-traversal baseline and proposed ceilings recorded in design.md; repeated/native Linux proof remains in 3.x).

## 2. Implementation and Targeted Proof

- [x] 2.1 Add native Arrow scan interfaces and bounded executor fallback; prove column/predicate/time pruning, snapshot/page parity and cancellation without row reconstruction on the native path.
- [x] 2.2 Add index declarations/maintenance and ordered seek reads; prove stable ties, restart/token invalidation, snapshot cuts and explicit missing-index behaviour.
- [x] 2.3 Add durable projection registration/reads and inspectable frontiers; prove crash replay, idempotence, advancement, lag and rebuild/invalidation semantics.
- [x] 2.4 Add Transform::TimeBuckets and projection revision/changed-key support; prove late correction/replacement, saved-cut stability and equivalence with generic execution.
- [x] 2.5 Add typed compare_exchange and atomic backend implementation; prove conflicting writers, create-if-absent, receipt replay and durability outcomes.
- [ ] 2.6 Implement all five capabilities in the durable reference and deterministic test backends; migrate existing implementers and operation/protocol consumers without product-specific APIs.
  - [x] Storage backend portion: all five capabilities in durable/test adapters, public manager/snapshot resources, exact retention gaps and files/receipt recovery proof.
  - [ ] Remaining: protocol, operations, runtime, SDK and example consumer adoption in the design inventory.
- [ ] 2.7 Update affected README/storage guides, examples and backend obligations; verify dependency direction and preservation of existing files, recovery and mutation receipts.

  - [x] Storage documentation portion: native API guide, README claims, tooling commands and retained recovery/file obligations.
  - [ ] Remaining: example consumer migration and linked usage claims after that migration.

  - [x] Prepare an ordered per-file consumer adoption inventory with inspected ownership and dependency blockers under Consumer Adoption; do not overwrite dirty shared protocol generation outputs.

## 3. Benchmark Proof

- [x] 3.4 Profile and repair late bucket replacement in `crates/harmos-storage/**`; persist incremental identity/bucket state, preserve exact saved cuts and replay, and prove affected-bucket work deterministically.
- [x] 3.5 Profile and repair two-million-entry index construction; preserve atomic publication and cancellation, and prove bounded decoding/preparation work.
- [x] 3.6 Profile and repair first/next/deep index pages; preserve ordering ties, reverse seeks and snapshot cuts, and prove seek plus bounded sequential work.
- [x] 3.7 Profile and repair the generic row full/first/next scan; eliminate history rescans/copies and prove bounded page work.
- [x] 3.8 Add true time-window and numeric-predicate cases to the shared storage workload and `tools/python/**` Nightly adapter, reporting source rows and bytes examined; record unchanged-workload before/after measurements in design.md and update `docs/storage.md`.
- [x] 3.9 Run focused storage correctness stories and benchmark producer proof, then the required format, Rust, Python, repository checks and test-artifact build; commit only owned paths locally without pushing.

- [x] 3.1 Measure native columnar, indexed seek, projection advancement/replay, late buckets and conditional-update contention against the baseline on Windows; verify results outside timed sections. Resolve deep-seek prefix scanning without changing budgets; native Linux remains in 3.2.
- [ ] 3.2 Record throughput, page latency, allocations/peak memory, bytes read, durability and cancellation/stop costs on Windows and Linux; preserve raw artifacts and resolve regressions.
  - [x] Windows: record measured times, throughput, Rust allocations/incremental heap, process peak memory, encoded column bytes, WAL/FULL commit costs, replay, cancellation refusal and owner stop/restart; preserve raw samples and unchanged budgets.
  - [ ] Linux: execute the identical native workload and retain its measurements; hosted Nightly execution remains pending.

- [x] 3.3 Wire backend-independent storage case names/units into Nightly benchmarks.json on native Linux and Windows; expose the identical workload as public storage bench support and retain only the boundary-crossing case in ordinary checks. Source and focused producer proof pass; hosted execution/publication awaits integration and authorization.

## 4. Final Validation and Delivery

- [ ] 4.1 After source closure and targeted proof, run the required native platform checks and hosted validation when delivery is authorized; report actual failures and pending proof.
- [ ] 4.2 Publish an authorized usable Harmos revision for Dokime's storage-interface change and verify its pin; leave Dokime extraction and its product benchmarks in that consumer change.

## Workflow Follow-Up

- Dokime's separate `storage-interface` consumes these extensions before extracting `dokime-storage`, retains product semantics and named disk-I/O exceptions, and proves before/after performance with multi-day data, restart, late replacement, finalization and report workloads.
