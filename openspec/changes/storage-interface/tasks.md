# Tasks

## 1. Interface Design

- [x] 1.1 Owner authorizes parallel work: Harmos host gRPC surfaces at a7583836 are complete; consumer wave pending; lock inventory for storage backend/object/query contracts, protocol mappings, operations, examples, backends, tests, benchmarks and storage guidance.
- [x] 1.2 Settle the proposed scan/index plan and page types, ordering, snapshot lifetime, capability negotiation, bounded fallback, cancellation and error semantics in design.md.
- [x] 1.3 Settle projection registration/versioning, readable frontier, source-commit coupling, crash replay, invalidation/rebuild and old-snapshot behaviour in design.md.
- [x] 1.4 Settle time-bucket boundaries, retained statistics/positions, corrections, late replacement, revision and changed-key retention semantics in design.md.
- [x] 1.5 Settle conditional-update token scope, absent-object semantics, object-safe backend primitive, conflict receipts, durability and atomic index effects in design.md.
- [x] 1.6 Lock the durable SQLite/Arrow reference backend module/feature, dependencies and capability obligations; retain the file recovery backend and bounded test backend as distinct owners.
- [ ] 1.7 Before implementation, lock reproducible before/after workloads and numerical regression budgets; capture the baseline with deterministic multi-day data and millions of samples (workload and provisional budgets recorded; baseline blocked by the guest worker's dirty protocol reference, see design.md).

## 2. Implementation and Targeted Proof

- [ ] 2.1 Add native Arrow scan interfaces and bounded executor fallback; prove column/predicate/time pruning, snapshot/page parity and cancellation without row reconstruction on the native path.
- [ ] 2.2 Add index declarations/maintenance and ordered seek reads; prove stable ties, restart/token invalidation, snapshot cuts and explicit missing-index behaviour.
- [ ] 2.3 Add durable projection registration/reads and inspectable frontiers; prove crash replay, idempotence, advancement, lag and rebuild/invalidation semantics.
- [ ] 2.4 Add Transform::TimeBuckets and projection revision/changed-key support; prove late correction/replacement, saved-cut stability and equivalence with generic execution.
- [ ] 2.5 Add typed compare_exchange and atomic backend implementation; prove conflicting writers, create-if-absent, receipt replay and durability outcomes.
- [ ] 2.6 Implement all five capabilities in the durable reference and deterministic test backends; migrate existing implementers and operation/protocol consumers without product-specific APIs.
- [ ] 2.7 Update affected README/storage guides, examples and backend obligations; verify dependency direction and preservation of existing files, recovery and mutation receipts.

## 3. Benchmark Proof

- [ ] 3.1 Measure native columnar, indexed seek, projection advancement/replay, late buckets and conditional-update contention against the baseline; verify results outside timed sections.
- [ ] 3.2 Record throughput, page latency, allocations/peak memory, bytes read, durability and cancellation/stop costs on Windows and Linux; preserve raw artifacts and resolve regressions.

## 4. Final Validation and Delivery

- [ ] 4.1 After source closure and targeted proof, run the required native platform checks and hosted validation when delivery is authorized; report actual failures and pending proof.
- [ ] 4.2 Publish an authorized usable Harmos revision for Dokime's storage-interface change and verify its pin; leave Dokime extraction and its product benchmarks in that consumer change.

## Workflow Follow-Up

- Dokime's separate `storage-interface` consumes these extensions before extracting `dokime-storage`, retains product semantics and named disk-I/O exceptions, and proves before/after performance with multi-day data, restart, late replacement, finalization and report workloads.
