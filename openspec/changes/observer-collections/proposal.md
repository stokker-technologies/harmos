# Observer Collections

## Why

A product can read one device and obtain rows for several streams, including
streams with no row or several rows at a tick. Independent observers repeat that
read; a product-owned producer loop duplicates the host's scheduling and demand
lifetime. Collections give that read one demand-driven owner.

## What Changes

- Declare one handler for a disjoint set of streams belonging to one plugin.
- Invoke that handler once per due tick while any member stream has demand;
  publish zero or more rows per stream with the tick's host timestamp.
- Acquire resources on the first listener and release them on the last listener,
  including failure, cancellation and attachment settlement.
- Resolve the collection interval from a constant or a named Duration setting
  through the existing host interval resolver.
- Extend the existing Samples window transport, scheduling, child calls and
  settlement; introduce no collection timer or producer loop.
- Adapt ordinary observers into the same internal demand machinery while keeping
  their required dense-column window behavior.
- Generate tracked Rust, Python, C# and C++ bindings. Add non-Rust authoring only
  when a thin wrapper suffices; otherwise report the missing authoring support.

## Capabilities

### New Capabilities

- `observer-collections`: Grouped demand, sparse and repeated rows, common tick
  timestamps, collection failure and first/last listener resource lifetime.

### Modified Capabilities

None. Stream schema revisions and retained-row behavior remain governed by
`stream-revisions`; changing the row representation must still use that contract.

## Impact

Implementation ownership covers `proto/harmos/model/v1/stream.proto`,
`proto/harmos/plugin/v1/stream.proto`, `proto/harmos/plugin/v1/instance.proto`,
their declaration validation and generated outputs, runtime gRPC publications and
window processing, Rust SDK stream/registration/server code and focused fixtures.
The other SDKs receive generated bindings and only thin authoring additions.
Any necessary declaration digest changes belong to this delivery. No dependency
change is planned.

The state worker owns `crates/harmos-runtime/src/state`; this change adds no state
service or state calls. Generated bindings may overlap that worker's outputs and
must be regenerated together during integration.

Documentation remains owner-written. See design.md for exact source findings,
the proposed shapes, implementation inventory and documentation impact.

This proposal is prepared for review. No implementation delivery is active in
this worktree; the orchestrator must agree it and activate it before source or
task-inventory edits.
