# Facility Trim

Status: agreed with the owner on 2026-11-10 except where marked proposed; implementation has not
started and waits for the owner's go. Independent of `grpc-plugins`.

## Why

Harmos carries facilities that no application uses or that are not its responsibility. Each one
is code and tests to maintain, and some add branches to the runtime.

## What Changes

| Facility | Size | Decision |
| --- | --- | --- |
| `cargo harmos` authoring tool and its embedded templates | 1,400 lines, plus 28,650 lines of templates in 191 files | Removed. Products create projects with their own tool |
| JSON storage backend | 2,400 lines | Removed. It existed to produce benchmarks, and benchmark results depend on the consumer's own backend |
| Public `signal` namespace | 1,450 lines | Removed as a public facility; see below |
| Operation response cache | About 110 lines of dedicated tests; no caller selects a cache key in either repository | Removed |
| External series adapter, axial surveys and tracked reads | About 210 lines of source and 520 of tests and fixtures | Removed |
| Statistical stream batches | About 300 lines of source and 230 of tests and fixtures | Removed |
| Simulation | 920 lines of source, about 1,040 of tests, support and benches | Proposed for removal; see below |

State management stays. It is a Harmos feature that applications build on.

### Signal Processing

- FFT, including the disk-backed variant, moves to Dokime, its only consumer.
- Statistics and downsampling are used by the storage query and by stream summaries. They stay
  as internals of storage, with no public namespace.
- The hand-maintained downsampling kernels (min/max, largest-triangle, decimation) are replaced
  by the `tsdownsample` Rust kernels where those cover the same behaviour.

### Simulation (Proposed)

No application uses it; Dokime constructs a simulated runtime in one test and drops it. Where
Dokime accelerates time it uses Tokio's paused clock directly. Removing it deletes the feature
branches in the executor, restart entropy and runtime setup.

Its tests are the ones that check state recovery, replay identity and inverse laws for undo.
Those properties stay covered: the tests are rewritten on the paused clock without the
simulation harness before the facility is deleted.

## Impact

- Breaking for a consumer that enables the `simulation`, `signal`, `signal-fft` or JSON storage
  features. Dokime moves FFT in its own change and drops the simulation feature forwarding.
- Benchmarks that used the JSON backend are removed with it.
