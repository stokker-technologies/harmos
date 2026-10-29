# Proposal

## Why

Storage and carrier boundaries repeatedly copy owned payloads, encode disposable intermediate buffers, and serialize independent file I/O. The user authorized measured optimization of Harmos storage, sidecar, guest and external gRPC paths, with baseline and identical post-change benchmarks.

## What Changes

- Establish explicit reference settings and four benchmark categories before optimizing production code.
- Add 30 MiB file, 10,000 record and 1,000,000 sample storage workloads using shipped JsonStorage.
- Measure real sidecar, guest and external gRPC command paths, retaining useful kernel benchmarks separately.
- **BREAKING**: consume owned storage payloads at admission and migrate all in-repository consumers; borrow for inspection and share immutable retry sources.
- **BREAKING**: replace mixed-type atomic backend writes with independent typed durable writes, ordered by the manager with idempotent prefix recovery.
- Ship a thin JsonStorage primary backend composed with the existing JSON state backend; ordinary persistence stories and storage benchmarks use the shipped implementation.
- Remove redundant byte accounting, payload copies and transport encoding buffers without changing wire compatibility.
- Add consumer-configured bounded independent file I/O while preserving file order and cursor/durability barriers.

- **BREAKING**: unify stored/local file reads, hashes, follows and captures; add shared directory listing/watching with ordered selection rules and ordinary shared data ownership. Expose the same operations to gRPC, sidecars and guests.
- Compose filesystem and capture/replay/state settings, migrate all callers, and add applicable filesystem benchmarks to the one final measurement.

## Capabilities

### New Capabilities

- `storage-transfers`: bounded owned storage and carrier transfers with measured workloads.
- `filesystem-operations`: unified file and directory operations across native and remote carriers.

### Modified Capabilities

None.

## Impact

Harmos domain, storage, runtime, SDK, sidecar, guest integration and benchmark fixtures, their tests, manifests, authored build/fixture tasks and maintained examples. Dokime implementation remains owned by its separate chat. No release or remote delivery is authorized by this implementation request.
