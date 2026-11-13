# Facility Trim

## Why

Harmos carries facilities that no application uses or that are not its responsibility. Each one
is code and tests to maintain, and some add branches to the runtime.

## What Changes

| Facility | Decision |
| --- | --- |
| `cargo harmos` authoring tool and its embedded templates | Removed. Products create projects with their own tool |
| JSON storage backend | Removed. It existed to produce benchmarks, and benchmark results depend on the consumer's own backend |
| Public `signal` namespace | Removed as a public facility; see below |
| Operation response cache | Removed |
| External series adapter, axial surveys and tracked reads | Removed |
| Statistical stream batches | Removed |
| Simulation | Stays; see below |

State management stays. It is a Harmos feature that applications build on.

### Signal Processing

- FFT, including the disk-backed variant, moves to Dokime, its only consumer.
- Statistics and downsampling are used by the storage query and by stream summaries. They stay
  as internals of storage queries, with no public namespace or statistical stream-batch API.
- The hand-maintained downsampling kernels (min/max, largest-triangle, decimation) stay, private
  to storage. Replacing them with `tsdownsample` was tried and dropped: the crate needs nightly
  Rust (`argminmax/nightly_simd`, error `E0554` on stable). Consumer applications own their
  private FFT, statistics and downsampling outside storage.

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

## Storage Removal Contract

The JSON backend removal is part of this combined branch; do not reimplement it in a later
slice. Its persistence benchmarks and JSON-only fixtures are removed with it. Consumer backends
supply persistence; Harmos retains its state-file journal and backend-neutral query kernels.

## Open Points

### Awaiting Owner Confirmation

- Keep the hand-maintained storage downsampling kernels and drop `tsdownsample` on stable Rust.
