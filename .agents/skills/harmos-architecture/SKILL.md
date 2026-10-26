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
| `<p>-domain` | Portable models, operation declarations, settings and pure behavior. | Host-free declaration libraries such as `harmos-domain`. |
| `<p>-runtime` | Operation implementations, state coordination, live resources and execution lifecycle. | Domain, applicable Harmos execution facilities and resource libraries. |
| `<p>-cli`, `<p>-gui`, `<p>-plugin`, `<p>-sidecar`, `<p>-guest` | Product interfaces and carrier entry points. | Product entry library plus interface or carrier dependencies. |
| Resource libraries | One external system, such as desktop automation or an external gRPC client. | Their own protocols; no dependency back on the product runtime. |

Place these crates under the product's `crates/` directory. Create only the
owners the product needs. Runtime is the execution composition layer below
the entry library; domain remains independently accessible beside it.
An existing application crate that owns execution fills the runtime role.

Interfaces access product functionality through `<p>`. A contract-only
interface uses its default domain exports; a host enables its `runtime`
feature. Domain and runtime can depend directly on their lower-level owners;
this facade rule applies to interfaces, not every internal dependency.

## Operation Placement

In a reusable product, declare each operation once in `<p>-domain` and
implement it in `<p>-runtime`. Catalogs reference those owners. Interfaces
expose existing implementations without introducing forwarding operations
merely to publish the same behavior.

In a self-contained crate, colocate each operation's declaration and handler
in a vertical module under `operations/`. Do not require parallel local
`domain/operations/` and `operations/` trees. A composed plugin may use
this layout for plugin-private operations while exposing reusable product
operations from its runtime. Extract contracts when independent reuse or
dependency requirements justify it.

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
The plugin retains that owner, shares its resources and binds compatible
handlers. Do not add another process without an isolation requirement.
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
