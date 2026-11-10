# Facility Trim

Status: agreed with the owner on 2026-11-10; implementation has not started and waits for the
owner's go. Independent of `grpc-plugins`.

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
| Simulation | 920 lines of source, about 1,040 of tests, support and benches | Stays; see below |

State management stays. It is a Harmos feature that applications build on.

### Signal Processing

- FFT, including the disk-backed variant, moves to Dokime, its only consumer.
- Statistics and downsampling are used by the storage query and by stream summaries. They stay
  as internals of storage, with no public namespace.
- The hand-maintained downsampling kernels (min/max, largest-triangle, decimation) are replaced
  by the `tsdownsample` Rust kernels where those cover the same behaviour.

### Simulation

Simulation stays. It was built to accelerate time in tests, and until now nothing could use it
for that, because a plugin in another process has its own clock. The host-owned clock in
`grpc-plugins` removes that obstacle: the simulation clock becomes the host's virtual clock, and
Dokime runs its stories on it. Its tests of state recovery, replay identity and inverse laws
stay as they are.

## Impact

- Breaking for a consumer that enables the `signal`, `signal-fft` or JSON storage features.
  Dokime moves FFT in its own change.
- Benchmarks that used the JSON backend are removed with it.
