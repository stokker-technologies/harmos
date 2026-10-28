# Single Resource Store And Shared Lifecycle

## Why

HX3 removes duplicated resource channels and startup registration paths. The owner's final
shared Lifecycle design on 2026-10-28 replaces every earlier builder/table and resource-stop
refinement. It is an agreed breaking cleanup delivered in this branch for one dev-drop PR.

## What Changes

- Introduce one Lifecycle and Setup contract for applications, sidecars and guests.
- Application launch runs prepare with Setup, aggregate validation, initialize and serve.
  Shutdown drains supervised work, then calls the retained owner's stop and cleanup.
- Setup owns resource insertion, and for applications lane settings and operation placement.
  A::Lanes declares itself. Tests use launch_with to replace prepared resources by type.
- Plugin CATALOGS constants own both inert inventory and execution registration. Remove
  provides and bind_operations; retain requires and streams metadata.
- Add symmetric harmos sidecar/guest features, modules and attributes; generated paths use
  the facade without caller-path guessing or silently dropped resources/requirements.
- Delete Application::Resources, Dependencies, public builder and consumer RegistrationBatch
  APIs, every registration(deps) path and resource stop callbacks. Preserve durable Store.
- Migrate all implementation, consumers, fixtures, benches, packaging inspection and guidance.

## Capabilities

### New Capabilities

- resource-store: shared setup, frozen resources, aggregate validation and lifecycle ownership.

### Modified Capabilities

Existing catalog atomicity, supervised cleanup and inert inspection guarantees remain.

## Impact

Breaking Rust API across SDK, runtime, domain inventory, macros, carriers, facade and consumers.
No aliases, deprecations or compatibility paths. PR #51 and other open OpenSpec changes stay
outside scope. Focused proof only; push refactor/single-resource-store after proof. No version,
release, full lane or ci:simulate work; the original PR handoff restriction remains.
