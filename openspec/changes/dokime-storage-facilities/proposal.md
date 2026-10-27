# Dokime Storage Facilities

## Why

Dokime still owns storage copy/follow loops and cannot admit backend serialization or retain append request buffers through cancellation using the current public contracts. The user authorized coupled Harmos and Dokime implementation so those mechanisms can be removed without losing bounded memory, stable source identity or exact retries.

## What Changes

- Extend existing storage facilities with shared-session following and opened-file, retained-producer capture where required by concrete Dokime consumers.
- Provide native preallocation/lifetime ownership for backend scratch and accepted request payloads.
- Migrate existing native implementations and fixtures with clean replacement semantics; keep policy in consumer-supplied storage configuration.
- Integrate through a local Cargo patch in the paired Dokime worktree before its complete prototype build.

## Impact

The Harmos storage backend, file and request ownership interfaces and their direct consumers may change. Preserve durability, exact retries, cancellation safety and source identity. No publication, full CI, benchmark or test execution is authorized; targeted project-task Cargo checks and necessary prototype builds are authorized.
