# Test Stories

## Release Readiness

The listed story consolidation is landed. Task 6.1 retains combined native
Linux/Windows proof. The remaining repository-wide deterministic assertion audit
belongs to `operation-grammar` A.11, including the elapsed-time assertion still in
storage file stories; FL.1–FL.3 own the reported flake investigations and proof.
This change does not claim those repairs or native-platform results are complete.

## Why

Repeated setup, fragmented scenarios and large timing workloads made focused
behavioral proof expensive and difficult to read.

## What Changes

- **One convention.** A test at integration level and above follows one user journey, one story
  per file. It reads top to bottom with few or no indentation levels and a short comment naming
  each stage; every line is an action or a check of what follows from it.
- **Shared modules own the mechanics.** Launching and shutting down a runtime, registering a
  catalog, storage fixtures and waiting for an event each exist once. A shared module never
  chooses a scenario or hides an assertion.
- **Fragmented groups become one story** that performs the scenario once and checks each facet
  in order.
- **Large workloads move to benchmarks.** A correctness test uses the smallest input that
  crosses the boundary it proves.
- **No test depends on real time.** A test runs on a paused or controlled clock or waits for an
  event. A bound that only stops a hung test stays, as a guard and not as an assertion.
- **Tests that prove little are removed,** with any useful check moved into a story.

## Impact

- Test trees of `harmos-runtime`, `harmos-storage`, `harmos-packaging`, `harmos-guest` and
  `harmos-operations`. No production behaviour changes.
- Tests of facilities removed by `facility-trim` are deleted there, not rewritten here.
