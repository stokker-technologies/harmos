# Plugins And Carriers

## Composed Product Plugin

A native plugin can retain a product runtime and supply its shared resources to
the original operation factories. The operations crate holds vertical contracts
and optional implementations; the runtime composes state and resource owners.
Plugin-private operations keep one definition in `operations/<op>/mod.rs`,
with native implementations in adjacent `windows.rs` and `unix.rs`. Expose those factories
without adding another forwarding operation or another local operation tree.

## Advertisement And Registration

Authors depend on the `harmos` facade with `sidecar` or `guest`. Use the matching
`harmos::sidecar` or `harmos::guest` attribute, including explicit
requirements and publication streams. The shared `Plugin` trait extends
`Lifecycle` and declares its complete operation inventory once:

```rust
const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
```

That slice drives both inert metadata inspection and executable binding. Keep
backend selection on module and re-export declarations, keeping this portable
plugin's catalog variants ungated. General application catalogs can instead
declare platform-only operations as described in the operations reference. Inspection never
runs prepare or initialize. Portable catalogs remain available without runtime
execution dependencies.

Async `prepare(config, &mut Setup<Self>)` acquires and inserts resources using
`setup.resource::<T>(value)` and adjusts optional settings through
`setup.settings()`. Plugin setup has no operations registration method: the SDK
owner installs constant catalogs and observer attachments, validates every setup
error, then calls initialize. Plugin authors never construct a WireTable or a
resource store. Typed stream endpoint contracts and consumer-selected stream
limits keep their existing owners.

The host authenticates advertisements and checks owner equality; plugins cannot
choose a registration prefix. Advertisements carry local IDs, while routed
frames carry already-qualified IDs, fingerprints and u64 generations. Transports
validate those IDs without adding a prefix. Old version-only peers are refused.

`host` is a valid plugin owner with ordinary admission and grant checks. That
name grants no application caller authority; authority comes from the runtime
context. Actual qualified operation and service identity collisions are refused.

## Lifecycle Responsibilities

The shared `Lifecycle` contract owns `Config`, `Error` and `Context`:

| Hook | Responsibility |
| --- | --- |
| `prepare(config, setup)` | Asynchronously acquire resources, configure setup and return their owner. |
| `initialize(context)` | Activate the validated owner with the assigned carrier context. |
| `stop(context)` | Close resources after supervised consumers drain. |
| `cleanup()` | Release ownership, including partial initialization state. |

The SDK validates resources, catalogs, attachments and contracts before
initialize. Stop and cleanup remain fallible and preserve supervised cleanup
ownership; dropping a handle does not settle asynchronous work.

## Requirements And Isolation

A plugin can require operations implemented by another registered provider.
The host matches complete contracts, including fingerprints, schema, policy,
visibility and replay behavior. Public visibility and unambiguous selection
are required; explicit provider selection does not relax compatibility.

Native observers do not declare a typed resources bundle. Their initialization
receives `&ExecutionContext`, and callbacks receive `ObserverContext<'_, Host>`
(with `Supervisor` as the default host type). Call `resource::<T>()` on that
context to resolve an `Arc<T>`; resource lookup returns `Result<_, ResourceError>`.

Observer `interval_ms` accepts a constant `u64` expression, including a policy
constant or associated constant. The generated inventory evaluates it at compile
time and rejects zero. Runtime configuration cannot change this static cadence.

For genuine process isolation, a separate sidecar can host the product runtime
while another plugin requires its contracts through the entry crate's default
domain exports. There is no local-runtime registration path for attaching a
runtime on another machine. A separately implemented network protocol is a
different integration boundary.

## Native And Guest Capabilities

Use `harmos` with `sidecar` for native process carriers or `guest` for Wasm
components and native guest hosting. The corresponding `sidecar` and `guest`
modules expose symmetric plugin, catalog and operation attributes. Shared
operation implementations import `harmos::{Context, Operation, Response}` and
consume their request in `run(self, ctx)`. Declare resources and callees with
`resources = [...]` and `operations = [...]`; use infallible
`ctx.resource::<T>()`, `ctx.operation(request)` and derived `ctx.streams()`.
Lifecycle contexts are `harmos::sidecar::Supervisor` and
`harmos::guest::kit::Context`. They share the SDK lifecycle without introducing a
native host runtime into Wasm. Required remote operations need only portable
declarations.

A guest is not automatically able to embed a native product runtime. Check
target-compatible dependencies, resource dependencies, host calls and permissions.
Reuse portable contracts and behavior; bind implementations supported by the
chosen carrier. Native observer contributions use their declared catalog;
guest observer callbacks are unsupported.

Framework-specific suites, templates, evidence and settings belong to the
framework's integration guidance. They do not redefine these product or
carrier ownership rules.
