# Test Stories

Status: agreed with the owner on 2026-11-10 and implementing under the combined `grpc-plugins`
marker. Source and focused proof are tracked separately in `tasks.md`; tooling ownership covers
the relocated binding-consumer task.

## Why

The runtime has more lines of test than of source: 716 test functions with a median length of 24
lines. Two audits found little duplicated coverage. The volume comes from many short tests that
each rebuild the same setup to check one facet, from workloads sized like benchmarks, and from
tests that depend on real time.

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

`design.md` lists the concrete groups and locations the audits found.

## Impact

- Test trees of `harmos-runtime`, `harmos-storage`, `harmos-packaging`, `harmos-guest` and
  `harmos-operations`. No production behaviour changes.
- Tests of facilities removed by `facility-trim` are deleted there, not rewritten here.
