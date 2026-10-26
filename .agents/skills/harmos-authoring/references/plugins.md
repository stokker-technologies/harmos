# Plugins And Carriers

## Composed Product Plugin

A native plugin can expose a reusable product without owning its contracts:

```text
product-plugin
    -> product (runtime feature)
        -> product-domain
        -> product-runtime
            -> product-domain
            -> resource libraries
```

The sidecar process already hosts that composition. Retain the product runtime
owner in the plugin, share its resources and expose the original contracts.
The plugin may also own private operations, observers and listeners. Put its
private operation declaration and handler together in a vertical
`operations/` file; promote reusable behavior to the product domain/runtime.

## Advertisement And Binding

The shipped native example uses `#[harmos_sidecar::plugin]` on its
`Plugin` implementation, with `provides` and `requires` naming domain
declaration catalogs. The corresponding guest uses `harmos_guest::plugin`.
Advertisement describes contracts; it does not install executable handlers.

`bind_operations` receives a `WireTable<Self::Context>`. Its implementation
binds a handler catalog to shared resources using `bind_operations` or
`bind_streaming_operations`, as appropriate. Stream binding also supplies
explicit supported limits. Choose handlers compatible with that context and
resource type.

```text
Domain Declaration Catalog -> Advertised Contracts
Runtime Handler Catalog + Shared Resources -> Executable Bindings
```

The operation macro generates handler machinery at compilation. The plugin
serving lifecycle invokes binding at runtime. Do not describe the macro as
starting a runtime or invoking lifecycle hooks itself.

## Lifecycle Responsibilities

The shared SDK plugin contract supplies `Config`, `Error`, `Context` and
`Resources` associated types and these responsibilities:

| Hook | Responsibility |
| --- | --- |
| `prepare(config)` | Construct the plugin from decoded configuration. |
| `resources()` | Expose the retained shared resource owner. |
| `initialize(context)` | Acquire/start resources that need activation and retain their lifecycle owners. |
| `bind_operations(table)` | Bind compatible handlers using those resources. |
| `stop(context)` | Settle application work during shutdown. |
| `cleanup()` | Release resources, including partial initialization state. |

The shared SDK owner prepares the plugin, awaits initialization, then binds and
validates its advertised contracts before marking it initialized. Account for
failures in both initialization and binding. Stop active consumers before
releasing shared devices and connections; do not rely on dropping a handle
to settle asynchronous work.

## Requirements And Isolation

A plugin can require operations implemented by another registered provider.
The host matches complete contracts, including version, schema, policy,
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
target-compatible dependencies, context types, host calls and permissions.
Reuse portable contracts and behavior; bind implementations supported by the
chosen carrier. Native observer contributions use their declared catalog;
guest observer callbacks are unsupported.

Framework-specific suites, templates, evidence and settings belong to the
framework's integration guidance. They do not redefine these product or
carrier ownership rules.
