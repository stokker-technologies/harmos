---
name: harmos-architecture
description: Design Harmos application boundaries, operation ownership, resources, and runtime composition.
---
<!-- Generated from .agents/skills/harmos-architecture/SKILL.md; edit .agents/skills/harmos-architecture/SKILL.md and run `stkr project update` -->

# Harmos Architecture

Project contracts take precedence over generic architecture defaults. Read current source for exact signatures; do not load restricted docs implicitly.

## Application And Contract Layout

- `contract/` owns shared message types, operation declarations and portable
  catalogs used by the application and plugins. Depend on `harmos-contract`;
  default declarations need no Tokio, Tonic, Prost or host engine. Enable `grpc`
  for explicit protobuf codecs, `reflection` for schema export, and `streaming`
  only where portable endpoint execution is needed.
- `app/` owns application state, resource-bound implementations and `app.rs`
  composition. CLI and GUI crates are shells. Plugins depend on the contract,
  never on the application runtime crate.
- Operations are the protocol. Declare `grpc(package = "...", service = "...",
  method = "...")` on the operation; do not prescribe separate consumer
  `grpc/` or `protocol/` crates/folders, hand-authored protobuf, or duplicate RPC
  handlers. Harmos's internal `grpc/` module owns transport implementation.
- Start small: `state.rs` for replayable application data, `resources.rs` for
  shared live dependencies, `operations/mod.rs` and vertical operation leaves.
  Introduce `state/` only when several cohesive state types or rules justify it;
  the runtime's internal state domain does not require consumers to copy it.
- Operations perform managed attempts, services own resident work, and observers
  own periodic production. A bounded subscription operation can stay open under
  an explicit policy and lane; it adapts an existing producer rather than owning
  a duplicate producer or scheduler.


## Catalogs And Domain Ownership

- Name domain catalogs by their role: `Operations`, `Observers`, `Services`,
  `Listeners`, `Streams`, `Records`, `Transactions`, and `Lanes`. Generic typed
  catalogs may use the application's own domain vocabulary.
- Assemble each catalog once and compose it directly. Avoid parallel aliases and
  catalog-wrapper functions. Use `()` for an absent generic catalog.
- Place managers inside their domains, such as `services/manager.rs` and
  `storage/manager.rs`, re-exported by the domain's `mod.rs`. There is one
  app-wide owner per domain; plugin scopes do not create independent managers.
- Keep listener delivery within stream/record ownership; no separate
  `ListenerManager`. Keep shared supervision private rather than duplicating
  service and observer lifecycle engines.
- Plugin-specific public types carry the `Plugin` prefix. Manager operations use
  `harmos::Result` and `harmos::Error`, not separate per-manager error facades.


## Simulation Surface

Keep the shared builder at `harmos::Builder`. Supporting simulation names belong
under `harmos::sim`; do not expose internal reboot factories or restore obsolete
runtime aliases in consumer APIs.

For concrete plugin and transport authoring, use `harmos-authoring`.
