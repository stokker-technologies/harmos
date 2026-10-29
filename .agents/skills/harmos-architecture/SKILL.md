---
name: harmos-architecture
description: Structure consumer applications using Harmos, including reusable domains and runtimes, self-contained plugins, resource libraries and interface crates.
---

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

Each operation has one definition in `operations/<op>/mod.rs`: its request,
response, identity, policy and contract tests. Native platform implementations
live beside it in `windows.rs` and `unix.rs`; gate only their declarations:

```rust
#[cfg(all(feature = "runtime", target_os = "windows"))]
mod windows;
#[cfg(all(feature = "runtime", unix))]
mod unix;
```

```text
src/
├── lib.rs                # Lifecycle/Setup resources and Plugin::CATALOGS
└── operations/
    ├── mod.rs
    ├── catalog.rs        # The plugin's one catalog
    └── <op>/
        ├── mod.rs        # The one portable definition and gated mod declarations
        ├── windows.rs    # Native Operation implementation and its tests
        └── unix.rs
```

Request and response types exist once and remain available without runtime.
A platform-independent implementation can use a runtime-gated inline `mod runtime`
in the same operation's `mod.rs`. Keep one catalog: no platform catalogs, backend
traits, platform selection blocks, `#[path]` switches or unsupported stubs.
Every catalog operation ships on each supported platform.

`Lifecycle::prepare(config, setup)` acquires shared platform handles and inserts
them once with `setup.resource::<T>(value)`. Plugin `const CATALOGS` names the
catalog's `CATALOG`; the SDK binds it after validating setup. A WLAN client or
D-Bus connection is an ordinary resource retained by its lifecycle owner.

For this portable plugin layout, gate only declarations: `mod` lines (including
test and runtime modules) and `pub use` lines. Keep its catalog variants ungated
so the same operations ship on every supported platform. General application
catalogs also support platform-only operations by gating both the owning `mod`
and its catalog variant; see the authoring reference. Never gate functions,
impls, structs, fields or statements to select a plugin backend.
Spell native predicates `target_os = "windows"` and `unix`;
use `target_os = "linux"` only for Linux-specific functionality.

## Operation Placement

In a reusable product, use one file per operation for its request struct, response and runtime-gated `Operation` implementation in
`<p>-operations/src/<concern>/<operation>.rs`. Its catalog supplies both
`catalog() -> Vec<OperationDefinition>` and, with `runtime`, implements
`operations::OperationCatalog` with an executable `CATALOG` descriptor. There is one
operation tree. Rust's orphan rule prevents a runtime crate from implementing
Harmos's `Operation` trait for operation types owned by a domain crate. When
contracts and execution need separate dependency selections, put the operation
types and their implementations in the same operations crate and gate execution
behind its `runtime` feature. Shared models remain in the domain; resource crates remain
optional lower-level dependencies. Application `Lifecycle::prepare` supplies resources through `Setup<Self>`
and places each catalog on a declared lane. Plugin `const CATALOGS` is the single
source for inert inventory and executable binding.

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
