---
name: harmos-architecture
description: Design Harmos application boundaries, operation ownership, resources, and runtime composition.
---
<!-- Generated from .agents/skills/harmos-architecture/SKILL.md; edit .agents/skills/harmos-architecture/SKILL.md and run `stkr project update` -->

# Harmos Architecture

Project contracts take precedence over generic architecture defaults. Read current source for exact signatures.

## Application And Contract Layout

- `contract/` owns shared message types, operation declarations and portable
  catalogs used by the application and plugins. Because guests compile it to
  WebAssembly, it depends on `harmos-contract` rather than the host facade;
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
- Declare each operation on its owning unit struct in a vertical leaf file.
  Catalog entries reference the leaf type; never attach operation metadata to
  enum entries.
- Assemble each catalog once and compose it directly. Avoid parallel aliases and
  catalog-wrapper functions. Use `()` for an absent generic catalog.
- Place managers inside their domains, such as `services/manager.rs` and
  `storage/manager.rs`, re-exported by the domain's `mod.rs`. There is one
  app-wide owner per domain; plugin scopes do not create independent managers.
- Keep listener delivery within stream/record ownership; no separate
  `ListenerManager`. Keep shared supervision private rather than duplicating
  service and observer lifecycle engines.
- Standard embedded, native and guest hosts use `Runtime.plugin` and
  `Handle.plugin`, the same `PluginHost` allocation. Registration is inert;
  explicit initialization activates the dependency graph. Runtime shutdown
  retires plugins while their operation/state infrastructure remains available.
  Rust-only embedded catalogs use `register_embedded` / `reload_embedded`;
  portable embedded catalogs use `register_embedded_wire` /
  `reload_embedded_wire`, projecting original contracts from resource-bound
  handlers through `AuthoredContracts`. Both use that same lifecycle/registry;
  neither requires a separate SDK host or changes original caller authority.
  Keep custom `PluginLifecycle` drivers behind the lower-level `PluginManager`
  extension rather than requiring them in ordinary applications.
- Plugin-specific public types carry the `Plugin` prefix. Manager operations use
  `harmos::Result` and `harmos::Error`, not separate per-manager error facades.


## Features And Imports

The host runtime, managed state, streams, and authoring are unconditional.
Features select optional facilities: `plugin`, `plugin-sidecar`, `plugin-guest`,
`grpc`, `reflection`, `signal`, `signal-fft`, `simulation`, and host-independent
`packaging`. Sidecar and guest hosting imply `plugin`; `grpc` implies
`reflection`; FFT implies `signal`. TLS and Web behavior is configured through
the single gRPC manager's settings.

Consumers depend on `harmos` alone. The facade exposes qualified namespaces, not
flattened aliases: `operations`, `message`, `schema`, `artifacts`, `catalog`,
`listeners`, `config`, `plugin` (host plugin management merged with the contract's
plugin declarations), `sidecar` (`plugin-sidecar`), `guest` (`plugin-guest`),
`bundle` (`plugin` or `packaging`), `builtin`, `signal` (`signal`) and `grpc`
(descriptor projection with `reflection`, the server with `grpc`). Native plugins
author through `harmos::sidecar`; build scripts project descriptors through
`harmos::grpc` with `reflection`. `__private` holds only what generated code
reaches. Crates compiled to WebAssembly (shared declaration crates and guest
components) are the one exception: they must build without the host runtime, so
they depend on `harmos-contract` and `harmos-guest`.

Each configurable crate owns externally supplied settings and their defaults in
root `src/config.rs`. Internal state and live resources remain with their domains.
Composition supplies each manager only its relevant settings; plugin activation
configuration follows the same root ownership. Domain re-exports may name the
same settings type, but never define a second copy or a second default.

`harmos::prelude` is a bounded authoring convenience for application entry points,
common operation/transaction traits, and macros. Domain modules retain canonical
ownership. Do not expand the prelude into an automatic manager/type inventory.

## Simulation Surface

Keep the shared builder at `harmos::Builder`. Supporting simulation names belong
under `harmos::simulation`; do not expose internal reboot factories or restore obsolete
runtime aliases in consumer APIs.

For concrete plugin and transport authoring, use `harmos-authoring`.
