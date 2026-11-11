# Harmos Plugin Protocol

[`harmos/v1/plugin.proto`](harmos/v1/plugin.proto) defines the plain gRPC wire
interface. [`harmos/v1/behaviour.md`](harmos/v1/behaviour.md) is its numbered
behaviour contract and fixed cross-language conformance fixture catalog.

[`harmos/v1/host.proto`](harmos/v1/host.proto) defines the shared protobuf assignment
catalog under `harmos.host.v1`. Assignments retain a qualified operation name, a
32-byte contract fingerprint and a nonzero activation generation. A receiver
validates identities, refuses duplicate local names, and freezes the first accepted catalog.

Rust bindings live in `crates/harmos-protocol`; the independent reference host
and hand-written Rust fixture server live in `crates/harmos-conformance`.
The behaviour contract includes build and invocation examples for every SDK.

Within `harmos.v1`, evolve additively: never reuse field numbers or enum values,
reserve removed names and numbers, and preserve existing meanings. Peers advertise
their highest revision in `Describe`; the effective revision is their minimum.
Unknown protobuf fields are ignored. An incompatible contract needs a new package.
