# Dokime Storage Facilities

## Why

Dokime still owns storage copy/follow loops and needs bounded file copying and retained append request buffers through cancellation using the current public contracts. The user authorized coupled Harmos and Dokime implementation so those mechanisms can be removed without losing bounded memory, stable source identity or exact retries.

## What Changes

- Extend existing storage facilities with manager-owned following and opened-file, retained-producer capture where required by concrete Dokime consumers.
- Retain backend and accepted request data through their actual lifetimes. The locked prototype-streamline Slice H supersedes the original read-session and backend scratch-admission design; consumers own retained-result and scratch budgets.
- Migrate existing native implementations and fixtures with clean replacement semantics; keep policy in consumer-supplied storage configuration.
- Integrate through a local Cargo patch in the paired Dokime worktree before its complete prototype build.

## Impact

The Harmos storage backend, file and request ownership interfaces and their direct consumers may change. Preserve durability, exact retries, cancellation safety and source identity. The read-path follow-up authorizes focused executed tests and opt-in timing diagnostics through project tasks, plus local commits. Publication, pushing and full CI remain unauthorized.
