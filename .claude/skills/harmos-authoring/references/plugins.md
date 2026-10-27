<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/references/plugins.md; sha256:b032d3b4567aa0a0d7ab2c790c90946528868b10b188275a701f7f7b2ce0db34 -->

# Plugins And Carriers

## Composed Product Plugin

A native plugin can retain a product runtime and supply its shared resources to
the original operation factories. The operations crate holds vertical contracts
and optional implementations; the runtime composes state and resource owners.
Plugin-private operations use the same vertical layout. Expose those factories
without adding another forwarding operation or another local operation tree.

## Advertisement And Registration

Native and guest plugin attributes describe provided and required catalogs.
Advertisement does not start execution. The same catalog provides portable
`Operations::catalog()` and runtime-gated `Operations::registration(dependencies)`.

Carrier setup constructs `WireTable::new(owner)` after host assignment. The
plugin's binding hook receives that table and calls
`table.register(Operations::registration(dependencies))`. Typed endpoint contracts
and consumer-selected stream limits remain with their existing owners.
`unregister(&QualifiedId)` withdraws an entry and awaits settlement.

The host authenticates advertisements and checks owner equality; plugins cannot
choose a registration prefix. Advertisements carry local IDs, while routed
frames carry already-qualified IDs, fingerprints and u64 generations. Transports
validate those IDs without adding a prefix. Old version-only peers are refused.

## Lifecycle Responsibilities

The shared SDK plugin contract supplies `Config`, `Error`, `Context` and
`Resources` associated types for plugin composition and these responsibilities:

| Hook | Responsibility |
| --- | --- |
| `prepare(config)` | Construct the plugin from decoded configuration. |
| `resources()` | Expose the retained shared resource owner. |
| `initialize(context)` | Acquire/start resources that need activation and retain their lifecycle owners. |
| `bind_operations(table)` | Register the catalog factories with a dependency snapshot. |
| `stop(context)` | Settle application work during shutdown. |
| `cleanup()` | Release resources, including partial initialization state. |

The shared SDK owner prepares the plugin, awaits initialization, then binds and
validates its advertised contracts before marking it initialized. Account for
failures in both initialization and binding. Stop active consumers before
releasing shared devices and connections; do not rely on dropping a handle
to settle asynchronous work.

## Requirements And Isolation

A plugin can require operations implemented by another registered provider.
The host matches complete contracts, including fingerprints, schema, policy,
visibility and replay behavior. Shared visibility and unambiguous selection
are required; explicit provider selection does not relax compatibility.

For genuine process isolation, a separate sidecar can host the product runtime
while another plugin requires its contracts through the entry crate's default
domain exports. There is no local-runtime registration path for attaching a
runtime on another machine. A separately implemented network protocol is a
different integration boundary.

## Native And Guest Capabilities

Use `harmos-sidecar` for native process carriers and `harmos-guest` for Wasm
components. Both share SDK contracts without depending on the full host runtime.
Host composition uses `harmos`; `plugin-sidecar` and `plugin-guest` select
the corresponding host facilities and imply `plugin`.

A guest is not automatically able to embed a native product runtime. Check
target-compatible dependencies, resource dependencies, host calls and permissions.
Reuse portable contracts and behavior; bind implementations supported by the
chosen carrier. Native observer contributions use their declared catalog;
guest observer callbacks are unsupported.

Framework-specific suites, templates, evidence and settings belong to the
framework's integration guidance. They do not redefine these product or
carrier ownership rules.
