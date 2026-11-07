# Runtime-Defined Operations

## Why

Dokime Python authoring owns operation schemas and callbacks at runtime. Compiled
`Executable` factories cannot represent those declarations without a second dispatcher.

## What Changes

- Add generation-scoped encoded operation bindings to the existing plugin staging path.
- Expose per-call retry overrides for encoded submissions and retained callbacks.
- Expose plugin-scoped observer staging for embedded providers.

## Capabilities

### New Capabilities

- `runtime-defined-operations`: Owned contracts and encoded handlers use normal admission.

### Modified Capabilities

## Impact

The paired consumer is Dokime `feat/dokime-python` in `dokime.wt/python`.
Authorization, admission, deadlines, cancellation, child metadata and retry safety
remain owned by existing Harmos execution. No new transport or dispatcher is added.
