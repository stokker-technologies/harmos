---
name: harmos-architecture
description: Structure consumer applications using Harmos, including reusable domains and runtimes, self-contained plugins, resource libraries and interface crates.
---
<!-- Rendered by stkr project update from .agents/skills/harmos-architecture/SKILL.md; sha256:13a829df6e0e27beb3bc54438f85dd6b00c1ddfb9d11d36525b4e17c7ab1159b -->

# Harmos Architecture

Use this skill for products consuming Harmos, including Harmos's shipped
application, sidecar and guest examples. It describes consumer ownership,
not the internal organization or CI of the Harmos repository. No company
skills or tooling are prerequisites.

## Product Boundaries

| Owner | Responsibility | Production Dependencies |
| --- | --- | --- |
| `<p>` | Public entry library and curated `prelude.rs`. | Domain by default; optional runtime. |
| `<p>-domain` | Shared portable models, settings, transactions and pure behavior. | Host-free domain libraries. |
| `<p>-operations` | Vertical operation messages, declarations and optional implementations. | `harmos` without default features; optional resource dependencies under `runtime`. |
| `<p>-runtime` | State coordination, live resource construction and execution composition. | Domain, operations with `runtime`, Harmos execution facilities and resource libraries. |
| `<p>-cli`, `<p>-gui`, `<p>-plugin`, `<p>-sidecar`, `<p>-guest` | Product interfaces and carrier entry points. | Product entry library plus interface or carrier dependencies. |
| Resource libraries | One external system, such as desktop automation or an external gRPC client. | Their own protocols; no dependency back on the product runtime. |

A single `-sidecar` or `-guest` crate stays under `crates/`. Several plugins use
`plugins/sidecars/<name>` and `plugins/guests/<name>`; these carrier crates declare `publish = false`.
Place the other product crates under `crates/`. Create only the
owners the product needs. Runtime is the execution composition layer below
the entry library; domain remains independently accessible beside it.
An existing application crate that owns execution fills the runtime role.

Interfaces access product functionality through `<p>`. A contract-only
interface uses its default domain exports; a host enables its `runtime`
feature. Domain and runtime can depend directly on their lower-level owners;
this facade rule applies to interfaces, not every internal dependency.

### Plugin Internal Structure

Target shape; it requires link-time registration (`#[harmos::catalog(register)]` and
`#[harmos::resource(register)]`, gathered by the plugin macro or builder), a Harmos follow-up feature.

```text
src/
├── lib.rs                  # The plugin; registered catalogs and backends are gathered
├── operations/
│   ├── mod.rs
│   ├── catalog.rs          # `Common`: every operation available on all platforms
│   └── <family>/<operation>.rs
└── platform/
    ├── mod.rs              # Only `#[cfg(...)] mod <os>;` lines
    ├── windows/
    │   ├── backend.rs      # The registered backend resource behind the product trait
    │   ├── catalog.rs      # Registered `Platform` catalog of Windows-only operations
    │   └── <operation>.rs
    └── linux/
        └── ...
```

Gate only declarations, one gate per declaration (`mod` lines, including `#[cfg(test)] mod tests` and
`#[cfg(feature = "runtime")] mod runtime`, and `pub use` lines), none on items: never on functions, impls, structs,
fields, statements or catalog variants. A gated-out module contributes nothing; there are no `unsupported` stubs and no
`pub use` selection. `Common` is identical on every platform, so its fingerprints match everywhere; a `Common` operation
whose backend is not registered on a platform fails at activation with the declared-dependency error.

## Operation Placement

In a reusable product, use one file per operation for its request struct, response and runtime-gated `Operation` implementation in
`<p>-operations/src/<concern>/<operation>.rs`. Its catalog supplies both
`catalog() -> Vec<OperationDefinition>` and, with `runtime`,
`registration(Dependencies) -> RegistrationBatch`. There is one operation tree.
Shared models remain in the domain; resource crates remain optional lower-level
dependencies. Runtime composition supplies dependencies and lane placement.

A self-contained crate can use the same vertical layout under `operations/`.
Create a separate operations crate when independent contract consumers justify
it. The operation struct is its request message. Its Definition Module and runtime-gated Runtime Module each use a complete skeleton,
including their own Tests section. Runtime resources are declared with `resources` and obtained through `Context`; they are not request fields.


## Resources And Configuration

Portable product settings and their validation belong in the domain.
A root `config.rs` composes subsystem settings; each subsystem owns its
settings and validation. Runtime constructs configured clients, devices and
workers and retains their lifecycle owners. Interface settings decode or
adapt host-specific input without duplicating product policy.

Keep resource implementations beside `operations/`, not beneath it merely
because handlers use them. A cohesive external-system library remains a valid
crate; small resources can be runtime modules. External gRPC clients are distinct
from exposing product operations through Harmos gRPC.

Crates owning fallible behavior use `error.rs` with `thiserror`, preserving
source errors and classifying failures at the owning boundary. Pure re-export
facades, thin binaries, procedural macros and foreign adapters need no artificial
error wrapper. No universal `boot.rs` or empty module hierarchy is required.

## Hosting And Reuse

A composed native plugin hosts the product runtime in its sidecar process.
The plugin retains that owner, shares its resources and registers the shared operation factories. Do not add another process without an isolation requirement.
A separately hosted provider instead advertises the contracts it implements;
other interfaces require those original contracts.

A Wasm guest is a different execution boundary. Reuse only dependencies,
handlers and contexts supported by that target; a native runtime is not
automatically guest-compatible. Registration declares contributions; activation
owns execution. Harmos's local provider registration is not remote-runtime
attachment.

Read [Workspace](references/workspace.md) for the full layout and facade
examples. Use the accompanying authoring skill for operation and carrier
mechanics; this skill owns the product structure.
