# Operation Visibility

## Why

`Shared` is unclear next to `Private`, a plugin cannot call its own shared operations, and a private operation can still declare a gRPC binding that exposes it outside the plugin.

## What Changes

- **BREAKING** Rename `Visibility::Shared` to `Visibility::Public` (attribute short form, domain enum, wire/serialized form, catalogs, fingerprints, docs and skills). Clean break; no alias.
- Keep `Private` (the declaring plugin only) and `Host` (the host application and its clients only; never plugins).
- A plugin may call every operation it declares, Private or Public; Host stays refused to plugin callers.
- A `grpc(...)` binding on a Private operation is a compile error, with the equivalent activation check for runtime-registered operations.
- Host callers are identified by a typed caller kind, never by comparing an owner name to `"host"`.

## Capabilities

### New Capabilities

- `operation-visibility`: who may call an operation and where it may be exposed.

## Impact

Harmos domain, macros, runtime admission and dispatch, authoring guidance and product guide; consumers (Dokime, HCL) rename `Shared` to `Public` when they adopt this release.
