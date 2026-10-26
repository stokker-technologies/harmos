# Authoring Streamline

## Why

Operation authoring exposes internal trait machinery, manually maintained versions and mixed request/resident policy. Message facilities and invocation controls need a coherent public surface.

## What Changes

- Introduce concern-organized consumer -operations crates; preserve pure -domain ownership, acyclic facade/runtime dependencies and aligned skills/scaffolding.
- Move operation policy models into the model tree and curate facade/prelude exports.
- Place operation on one flat, feature-gated executable struct; author request/response messages separately, preserve ordinary Rust derives/custom codecs, and improve diagnostics.
- Expose fingerprint, digest, encoded_size, memory_size, encode, decode and allocation-reusing encode_into with explicit stable semantics.
- Replace manual operation versions with complete contract fingerprints.
- Use explicit Operation implementations constructed by initialize(context) and introduce operation(request).retry(policy).run()/start() with per-invocation policy.
- Separate request retry policy from ServicePolicy; restart service errors according to policy without classification.
- Unify runtime/manager register and unregister, individual identity removal, owner-qualified plugin IDs and dynamic contributions.
- Use closed runtime lane enums with typed invocation overrides and host-owned default placement; retain safe contribution withdrawal.

These targets are resolved by design.md and the disjoint inventory in tasks.md; implementation has not started. Development compatibility permits a clean source/protocol break without legacy adapters; persisted application data and payload encodings remain intact. Automatic reload/readiness/rollback, a new carrier protocol architecture, and external company skill edits are deferred for tonight; explicit unregister/register and existing transport machinery cover consumers.

## Capabilities

### New Capabilities

- `operation-authoring`: model ownership, single-macro declarations, automatic contract identity and invocation lifecycle.
- `runtime-registration`: unified registration, qualified contribution identity, typed lanes and service policy.
- `executable-operation`: flat feature-gated authoring, local construction and shared portable definitions.
- `consumer-crate-layout`: concern-based operation crates, pure domain boundaries and coordinated skills/scaffolding.
- `message-authoring`: consistent message identity, encoding, sizing and optional standard traits.

### Modified Capabilities

None.

## Impact

Domain schemas and policy, macros, SDK/runtime handlers and invocation facilities, services/observers, carrier adapters, packaging/bindings, consumers, fixtures, templates and affected README/guidance. Preserve execution, memory, replay and lifecycle guarantees. Audit stored/wire version compatibility before source editing. Update repository Harmos architecture, authoring and testing skills, references and generated copies. Canonical company architecture/Rust architecture owners and their external generated copies remain a deferred follow-up because this assignment is restricted to this worktree. Coordinate overlaps with consumer-skills without activating it or editing installed caches as source. No other open change is activated.
