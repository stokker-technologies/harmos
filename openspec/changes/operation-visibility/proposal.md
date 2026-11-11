# Operation Visibility

## Why

`Shared` is unclear next to `Private`, a plugin cannot call its own shared operations, and a private operation can still declare a gRPC binding that exposes it outside the plugin.

## What Changes

- **BREAKING** Rename `Visibility::Shared` to `Visibility::Public` (attribute short form, domain enum, wire/serialized form, catalogs, fingerprints, docs and skills). Clean break; no alias.
- Keep only `Private` (the declaring plugin and host) and `Public` (everyone connected); remove `Host` without an alias.
- A plugin may call every operation it declares without a self-requirement; other plugins and external clients may call only Public operations.
- A `grpc(...)` binding on a Private operation is a compile error, with the equivalent activation check for runtime-registered operations.
- Host callers are identified by a typed caller kind, never by comparing an owner name to `"host"`.

## Capabilities

### New Capabilities

- `operation-visibility`: who may call an operation and where it may be exposed.

## Impact

Harmos domain, macros, runtime admission and dispatch, authoring guidance and product guide; consumers rename `Shared` to `Public` when they adopt this release.

## Built-In Mapping

Observer control, storage file mutation operations, `follow_objects`, `follow_stream`,
`storage_status` and `write_schema` are Private. Existing public file read, hash, capture,
follow and directory operations remain Public. The host may drive Private operations;
external clients do not inherit host authority. This is the two-level contract shared with
`grpc-plugins` and replaces the three-level contract.

## Observer Consumer Contract

A released observer may retain a stopped snapshot until scope retirement. Consumers await zero
leases and `Snapshot.lifecycle.phase() == OperationPhase::Stopped` within their operation policy,
rather than waiting for resident history keys to disappear. Cancellation propagates through the
existing invocation and activation interruption; retained stopped history does not mean work is running.

## Open Points

### Awaiting Owner Confirmation

- The built-in operations listed in Built-In Mapping use Private visibility.
