# Design

## Locked Decisions

Preserve sequential arithmetic, exact sign-safe interpolation, typed predicates, snapshot cuts, capture membership, checksums, quality counts and bucket order. Keep the logical nine-pass interior-percentile work charge. Consumer policy continues to own deadlines and memory budgets.

## Ownership And Dependency Order

Paired worktrees are harmos.wt/query-batching and dokime.wt/query-batching on perf/query-batching. Harmos owns crates/harmos-storage/src/query/quantiles.rs and its integration tests. Dokime owns runtime/query/{batches,buckets,statistics,selection}, storage/sqlite/data/telemetry/query/batches.rs, registrations and focused tests. Preserve current scoped-document evidence.

Dokime needs the spool to retain compact finite f64 values between exact radix passes, replacing repeated evidence decoding. Existing Accumulator supports individual generic scalar reductions rather than shared requested f64 ranks. The smallest upstream capability is a numeric spool with caller-budgeted payload/scratch and cooperative cancellation. Adopt its merged revision through the existing Harmos facade.

## Production Hardening

Reject invalid finite inputs and percentile ranks. Check cancellation through batch loops and at bounded intervals during rank scans. Account spill buffers and rank scratch within explicit caller allocations. Run numeric selection on a blocking worker with cancellation propagated from a dropped query. Temporary files close on return or error.

## Documentation And Proof

Existing README claims and links remain accurate. Update existing query topic guides. Retain bit-exact numeric parity and proportional batch/spill regressions in one cohesive integration story per new file: Harmos tests/integration/exact_quantile_spill.rs and Dokime tests/integration/numeric_query_parity.rs. Million-row scaling remains owned by existing benchmarks; it adds no distinct correctness contract beyond the retained ordering, predicate, numeric and spill cases. Run the thirteen original focused cases, additional touched-module tests and check:rust. A busy machine skips comparative benchmarking with the reason reported. Final local proof uses the existing all-platform simulation; provider delivery remains authorized and immediate.
