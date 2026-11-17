# Facility Trim

## Release Readiness

The removals are landed. Task 3.1 retains combined native Linux/Windows query,
stream and state proof; source inspection does not close it.

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

- FFT belongs to its consuming product.
- The public signal namespace and statistical stream-batch API are removed.
- The original kernel-retention choice is superseded by [operation-boundary](../operation-boundary/tasks.md), which owns one
  crate-internal statistics/aggregation home in `harmos-query`, available through storage
  `list_rows` transforms, and removal of duplicate kernels. That consolidation is
  landed under its owning group; native final proof remains open here.

### Simulation

Simulation stays. It was built to accelerate time in tests, and until now nothing could use it
for that, because a plugin in another process has its own clock. The host-owned clock in
`grpc-plugins` removes that obstacle: the simulation clock becomes the host's virtual clock, and
a product runs its stories on it. Its tests of state recovery, replay identity and inverse laws
stay as they are.

## Impact

- Breaking for a consumer that enables the `signal`, `signal-fft` or JSON storage features.
  A product moves FFT in its own change.
- Benchmarks that used the JSON backend are removed with it.

## Storage Removal Contract

The JSON backend removal is part of this combined branch; do not reimplement it in a later
slice. Its persistence benchmarks and JSON-only fixtures are removed with it. Consumer backends
supply persistence. Harmos retains recovery interfaces, the default in-memory adapter and
backend-neutral query kernels; products own disk recovery.

## Replacement Ownership

The former downsampling-retention question belongs to [operation-boundary](../operation-boundary/tasks.md) group 6. This change
retains its unperformed native storage, stream and state proof in task 3.1 and is not archived.
