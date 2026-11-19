# Plugin Lifecycle Design

## Context

See proposal.md for motivation and the locked owner decisions. Paths are relative to the
repository root; cite symbols, not lines.

Current surface (landed, verified 2026-11-16):

- Runtime: `Runtime::launch` and `launch_with` (`crates/harmos-runtime/src/runtime/launch.rs`),
  `shutdown`, `shutdown_with_timeout` and `start_storage` (`runtime/lifecycle.rs`); the
  builder already has `register` and an `initialize` callback (`runtime.rs`).
- Application hooks: `Lifecycle::{prepare, initialize, stop, cleanup}`
  (`crates/harmos-operations/src/execution/lifecycle.rs`). Services have
  `Service::{initialize, run, stop, cleanup}` (`execution/services.rs`); `Carrier`,
  `ServiceInstance` and `NativeInstance` carry `stop` and `cleanup`
  (`execution/operations/authored.rs`, `execution/operations/registration.rs`).
- Native plugins: `Plugin::on_stop(Fn() -> Result<()>)` in Rust (`sdk/rust/src/plugin.rs`),
  Python instance hooks (`sdk/python/src/harmos/_authoring.py`), `OnStop(Func<Task>)` in C#
  (`sdk/csharp/src/Harmos/Plugin.cs`) and `on_stop(std::function<void()>)` in C++
  (`sdk/cpp/include/harmos/harmos.hpp`); no context, no initialize hook. The host calls
  `Instance.Stop` before removal or shutdown (`proto/harmos/plugin/v1/instance.proto`).
- Wasm guests: `harmos::wasm::Registration` with `on_initialize(&Context, Vec<u8>)`,
  `on_shutdown(&Context)` and `on_cleanup(Fn() -> Result<()>)`
  (`sdk/rust/src/wasm/registration.rs`); `GuestRegistration` carries `initialize` and
  `shutdown` policies and `GuestInitialize`/`GuestShutdown` commands
  (`proto/harmos/plugin/v1/guest.proto`). Cleanup already runs after failed initialization.
- Serving: Rust `serve` (`sdk/rust/src/native.rs`), Python `start(address) -> Endpoint`,
  `Endpoint.close` and `serve` (`sdk/python/src/harmos/_authoring.py`), C# `ServeAsync`, C++
  `serve`.
- Declarations: plugins compose builder chains (`operation`, `stream`, `observer`,
  `settings`, `section`); `declarations()` returns the frozen `harmos.model.v1.Declarations`
  reported by `Instance.Get` and `GuestRegistration`. docs/architecture.md (Guest Entries
  and Reported Declarations) keeps catalog terminology for internal registries and names
  the wire payload declarations.
- Aliases: products import the Harmos `Plugin` under renamed aliases, and the SDKs, their
  generators and tests contain import aliases and `#[path]`/`sys.path` overrides.

Touched invariant: docs/architecture.md says "There is no Catalog alias" for the reported
declarations. Its purpose, one name for the wire payload, holds: `catalog()` names the
author-side module contribution, never the payload, and `declarations()` stays.

## Goals / Non-Goals

**Goals:**

- One lifecycle vocabulary in every placement and language, each verb in its pair.
- Resource acquisition only in initialize and release only in finalize, with finalize
  guaranteed once initialize was attempted.
- Registration as inert, typed, accumulating declaration composition.

**Non-Goals:**

- No security or prerequisite phase between admission and initialize (later follow-up).
- No plugin start or stop, no per-execution registration, no host-side template scoping.
- No change to invocation-slot phases of Wasm guests (`GuestPrepare`, invocation
  initialize, attempt, slot finalize and cancel); they are per-call, not plugin lifecycle.
- No product kinds in Harmos; products compose through declaration sections.

## Decisions

Decision sources. Owner-locked (proposal.md): paired verbs, host verbs, plugin phases,
registration as admission, the registrar API with C++ `add`, module `catalog()`, one
vocabulary, no aliases or path overrides, SDK ownership. Derived by this design to make them
implementable; the owner may override each before its group starts:

- Reserved native lifecycle operation identifiers and admitted contexts.
- Initialize and finalize using existing operation policies and defaults.
- `Runtime::new()` as the construction step, `harmos::wasm::Guest` for guest capacity, and
  C++ `add` also on the plugin builder method that takes the register function.
- Python endpoint `close` becoming `stop`, and deleting the superseded builder chain methods.

### D1. Host Verbs (Proposed)

```rust
let runtime = harmos_runtime::Runtime::new()      // construct, nothing runs
    .register(product::catalog())                 // declare
    .start()                                      // begin activity, returns the running handle
    .await?;
runtime.stop().await?;                            // end activity, settle owners
// or: harmos_runtime::Runtime::new().register(product::catalog()).run().await?;
```

`launch`/`launch_with` become `start`/`start_with`; `shutdown`/`shutdown_with_timeout`
become `stop`/`stop_with_timeout`. `run` starts, waits for the stop signal and stops. SDK
serving entry points denote the same thing and become `run` (Rust and C++ `run`, Python
`run`, C# `RunAsync`); Python's `start(address) -> Endpoint` keeps `start` and its endpoint's
`close` becomes `stop`, completing the pair. The inventory task applies the renames only where
a symbol denotes start, stop or run; listener and storage internals that merely shut a
resource keep their names unless they are public verbs of the host. Alternative rejected:
keeping `launch`/`shutdown` beside `start`/`stop` as aliases.

### D2. Plugin Lifecycle Operations (Owner Decision, 2026-11-19)

Lifecycle hooks are ordinary operations with SDK-encoded well-known identifiers.
This replaces the dedicated initialization/finalization RPC design. Native wire
messages and `Instance.Get`, `Invoke`, `Samples` and `Stop` stay unchanged.

For plugin identifier `p`, the reserved operation identifiers are `p.initialize`
and `p.finalize`. Each uses `google.protobuf.Empty` for request and response,
Protected visibility and no replay safety. Both identifiers follow the existing
local operation grammar (nonempty dot-separated alphanumeric/underscore segments).
An SDK lists each hook only when defined and rejects ordinary author operations
using either reserved identifier. The host retains their admitted entries outside
normal routing, so clients, plugins and ordinary host calls cannot invoke them.

```python
class Author:
    async def initialize(self, ctx):
        self.resource = await acquire(ctx)

    async def finalize(self, ctx):
        await release(self.resource)

    async def execute(self, ctx, request):
        assert ctx.plugin is self
        return self.resource.execute(request)
```

`ctx.plugin` is the same author instance for hooks, ordinary operations, local
calls and stream contexts. Invocation resources (`ctx.set`/`ctx.get`) remain
invocation-owned. Python removes the context-free `@on_stop` path; Stop performs
transport shutdown only. Rust, C# and C++ must encode the same reserved operations
and replace their context-free cleanup callbacks in their assigned SDK work.

The host admits declarations and effective settings first. It invokes initialize
through normal operation admission, policy enforcement and the existing Invoke
carrier. Declared child calls, settings, secrets, logging and cancellation retain
ordinary invocation semantics. Ordinary operations refuse with the existing
`NotReady` error until initialize succeeds; Ready and sample demands follow its
success. Failure is retained as the activation failure. No hook means no added
wire call. Policies use the existing operation policy/default; no new setting or
lifecycle policy messages are introduced.

Every orderly ending stops new dispatch, settles sample demands and cancels and
settles admitted operations, invokes finalize exactly once, then sends the existing
Stop. A failed or cancelled initialize follows the same finalization/Stop path,
retaining its original failure. Finalize failure is recorded and Stop still runs.
A lost carrier cannot deliver Invoke; the host records that failure and releases
its admission. This design does not invent an SDK-local context or a second
finalization path on disconnect. Reconnection is a fresh activation.

#### Catalog Composition

Python keeps declaration-only `Catalog`/`Registrar` assembly. Catalog contributions
collect their async instance hooks in registration order; the plugin encodes one
reserved operation for each contributed phase. Each hook and ordinary handler
receives its own catalog instance through `ctx.plugin`, including local calls and
stream contexts. Hook composition introduces no second stop callback path.

Runtime stop closes listener admission first. Native plugin participants report
settlement only after accepted work and finalize complete, while the dispatcher
can still serve hook child calls. Dispatcher closure then precedes participant
transport Stop. The no-guest runtime configuration follows the same ordering;
owned listener signals and joins retain their existing owner.

#### Plugin-Lifetime Subscription Boundary

An Invoke child Watch belongs to its invocation and ends with that invocation.
`Samples` is a separate, host-opened publication demand: Open selects a declared
plugin stream; window responses may issue child calls only while the window is
unanswered. Answering that window ends its children. Device producers have no
child-call output carrier. Neither carrier names an activation-owned subscription
or lets a plugin transfer a Watch from initialize to finalize. Consequently a
subscription opened inside initialize cannot survive its return with the current
wire. Plugin-lifetime subscriptions are left out; they need a wire contract for
subscription lifetime and child replies beyond an invocation/window. Holding
initialize open, retaining its context or launching an untracked task is not a
substitute.

Wasm hook/verb migrations remain assigned to the separate SDK/guest tasks. Their
existing guest commands are unchanged by this native host/Python delivery.

#### Other SDK and Documentation Owners

Rust's `sdk/rust/src/plugin.rs` operation/declaration construction must encode
Protected Empty lifecycle entries and reserve their identifiers, replacing
`Plugin::on_stop`; its callback vector, `server::Cleanup::run` and the invocations
from `Server::stop`, `server::run` shutdown and the owned listener thread in
`sdk/rust/src/native.rs` become redundant. Listener stop signals and owned RPC/task
draining remain transport responsibilities. The separately assigned Wasm migration
replaces `Registration::on_initialize`, `on_shutdown` and `on_cleanup` with the
plugin's initialize/finalize operations; native serving does not alter those hooks.
C# needs the same operation entries in `sdk/csharp/src/Harmos/Plugin.cs`, replacing
`OnStop`, its cleanup callback array and their execution from `PluginService.Stop`. C++ needs them in
`sdk/cpp/include/harmos/harmos.hpp` and `sdk/cpp/src/catalog.cpp`, replacing
`on_stop`, its `cleanup_` callback vector and `Service::finish_cleanup`
from both Stop and the listener worker in `sdk/cpp/src/server.cpp`. The internal
transport stop signal, shutdown reactor and listener ownership remain. Each SDK must share
its author state with subsequent invocation contexts and retain ordinary operation
policy enforcement. The old callbacks and new lifecycle operations would otherwise
be two independent release paths; delete the context-free callbacks in those owners.

The documentation owner updates `docs/tooling.md` to replace `@on_stop` with async
instance hooks, explain transport-only Stop and use `FailureError(model.Failure(...))`.
`docs/python-sdk.md` documents `ctx.plugin` and invocation-owned resources separately.
`docs/plugin-manager.md` records initialization before Ready. `docs/architecture.md`
records hidden lifecycle operations, their ordinary policies and the required native
shutdown ordering; the native runtime now separates plugin settlement from transport Stop.
The Stop comments in `proto/harmos/plugin/v1/instance.proto` still describe SDK cleanup;
the protocol owner must reconcile those comments without changing wire messages.

#### Manager Actor Reentrancy

The manager retains one registry and settings owner. Its bounded read mailbox serves
settings, entry, activation and watch requests while an owned lifecycle task runs;
mutations remain ordered in their bounded mailbox. Replacement initialization runs
in an owned task, and activation workers own finalization. Their outcomes resume the
same ordered mutation, preserving rollback and replay receipts. Admission publishes
the description and materializes effective settings while the entry remains
Connecting; only successful initialize permits Ready. There is no second settings
store or reentrant mutation loop.

#### Native Shutdown Phase

The runtime first closes connection admission and settles connections, their calls
and connection-end observers while plugins, operations, application services and
storage remain usable. It then closes guest admission and settles guest plugins
before the existing participant authority requests native plugin settlement.
Native attachment and staged-replacement participants retain that settlement until
their admitted operations and publications settle and finalize completes. Running
attachments then acknowledge settlement and wait for the later participant stop.
The runtime keeps the dispatcher and host settings owner available through this
barrier, closes operation admission, settles listeners and application services,
closes the dispatcher, then requests participant stop, which sends the existing
transport Stop and reaps managed processes. Application lifecycle cleanup, state
sealing, journal drainage, storage closure and infrastructure cleanup follow.
Startup and failed activation
retain their existing cancellation, finalization and process-settlement policies.
No deadline or operation bound is lengthened, and no second dispatcher is added.


#### Overlapping Implementations Requiring Owner Resolution

- `plugin/manager/settings.rs::select` fills defaults and validates effective
  values for native startup/replacement and guest admission. `materialize` repeats
  that computation when actor commits persist settings. Delete the repeated
  default-selection computation in `materialize`, retaining its persistence work
  and calling the existing selection owner.
- `plugin/manager/actor.rs::describe` projects activation description/process
  metadata for admission and health. `plugin/manager/replacement.rs::replace`
  projects those same fields for staged replacement. Delete the replacement's
  repeated projection after one shared projection accepts the candidate version;
  do not read the previous bundle version during candidate projection.

### D3. Registrar and Typed Registration (Proposed)

**Owner amendment, 2026-11-16 (`plugin-boundary`):** products own the public
registrar and typed registration contract in each language. The generic
accumulation described below is internal carrier-helper machinery, not a public
Harmos author SDK or types re-exported to authors. Product APIs translate to it
without aliases or copying cancellation/window engines. No `PluginBackend` trait;
product admission/settings-resolution hooks are registered on a plugin kind.

`Registrar` accumulates operations, streams, observers, settings, requirements and product
sections; `register<T: Registration>(&mut self, item: T) -> Result<()>` is typed at entry,
erases handlers and appends descriptors, so `Plugin` keeps one type however many records are
declared. Internal helper registrations accumulate generic declarations and
sections; product-owned registrations collect product kinds and translate them
into their typed declaration section. Duplicate identities, missing requirements and descriptor
closure are checked when the plugin freezes its declarations, as today.

| Language | Phase Function | Registration Call | Hooks | Serving |
| --- | --- | --- | --- | --- |
| Rust | `fn register(ctx: &mut Registrar) -> Result<()>` | `ctx.register(settings::catalog())?` | `.initialize(f)`, `.finalize(f)` | `.run()` |
| Python | `def register(ctx: Registrar) -> None` | `ctx.register(settings.catalog())` | `async initialize(self, ctx)`, `async finalize(self, ctx)` | `run()`, `start()`/`stop()` |
| C# | `static void Register(Registrar ctx)` | `ctx.Register(Settings.Catalog())` | `.Initialize(f)`, `.Finalize(f)` | `RunAsync()` |
| C++ | `void register_plugin(Registrar &ctx)` | `ctx.add(settings::catalog())` | `.initialize(f)`, `.finalize(f)` | `run()` |

C++ uses `add` wherever the other languages call `register`, including the plugin builder
method that accepts the phase function. The Wasm `Registration` builder is dissolved: hooks
move to the plugin value, guest capacity moves to a `harmos::wasm::Guest` wrapper, and the
`harmos_register` export symbol stays as the ABI name of the register phase. The runtime's
`Registration<A>` trait for host catalogs keeps its name: the same verb and trait name at the
host and plugin placements is the intended harmony.

Alternative rejected: a host-invoked registration call after discovery, which would need a
second freeze and admission protocol.

### D4. Module Catalogs (Proposed)

Each module that contributes declarations exposes `catalog()` returning a value implementing
`Registration` (Rust), or the language equivalent. `declarations()` remains the frozen
host-facing description. Examples, fixtures, conformance plugins and SDK guides use
`settings::catalog()`, `operations::catalog()`, `observers::catalog()`, `streams::catalog()`.

### D5. Application, Service and Carrier Hooks (Proposed)

Decision 1's naming applies to every acquire/release hook: `Lifecycle::cleanup`,
`Service::cleanup`, `Carrier::cleanup`, `ServiceInstance::cleanup` and
`NativeInstance::cleanup` become `finalize` with unchanged semantics: finalize already runs
after partial setup, including the `resource-store` path where `prepare` returned an owner and
validation failed. `Lifecycle::prepare` (constructs the application and records setup, today
allowed to acquire) and the `stop` hooks of `Lifecycle` and `Service` (cooperative end of the
application or of `Service::run` before release) keep their names and behavior in this change;
see Open Questions.

### D6. Qualified Paths Instead of Aliases (Proposed, Owner Direction)

Sources import the names they use and qualify clashes by module path. Removed:
Rust `use x as y` and `pub use x as y`, Python `import x as y` and `from x import y as z`,
C# `using X = Y;`, C++ `namespace x = y;` and `using X = other::Y;` renames, Rust `#[path]`
and Python `sys.path` edits. Defining a new name for a composed type (for example a callback
signature `using Hook = std::function<void()>;`) is a definition, not a rename. Generators stop
emitting aliases. An exception is allowed only when unavoidable and must be named with its
reason in docs/architecture.md; the inventory task proposes candidates (for example Rust
integration-test targets that share one support module, or third-party generated code the
project does not author) and keeps only those with no alternative such as a support crate or
an ordinary module tree.

## Risks / Trade-Offs

- [Every plugin and product breaks at once] → one revision lands all renames; products adopt
  it in their own change; development policy allows clean breaks.
- [Finalize after a failed initialize sees partial resources] → authored finalize must tolerate absent resources; its context is an ordinary admitted invocation.
- [Carrier loss prevents remote finalization] → record the transport failure; no SDK-local emulation supplies a false admitted context.
- [Removing test `#[path]` sharing grows test structure] → shared support moves into an
  ordinary module tree or a dev-only crate; named exceptions remain where neither works.

## Migration Plan

Coordinate one final regeneration with `operation-boundary` and `plugin-boundary`;
the SDK tasks implement helper behavior and fixture product APIs, while real
products own their author surfaces. Generated code remains tracked.

1. Inventory and documentation contract (group 1).
2. Host verbs and hook renames (group 2), then registration (group 3), then plugin phases and
   protocol (groups 4–5), then aliases (group 6); each group keeps the build green and lands
   its own tests and documentation.
3. Products pin the landed revision in their own `plugin-lifecycle` change. No compatibility
   shims; rollback is the previous pin.

## Open Questions

1. **Application and service hooks outside the named placements.** Should
   `Lifecycle::prepare` become `register` (declaration only, with acquisition moved to
   `initialize`), and should the `stop` hooks of `Lifecycle` and `Service` merge into
   cancellation followed by finalize as plugins do? Proposed default: yes for both, in a
   follow-up change once the owner confirms; this change only renames `cleanup` to `finalize`,
   so its tasks do not depend on the answer.

## Lifecycle Worker Source Inventory

The base SDK's Rust `Plugin` was already monomorphic at `6702367a8`.
Typed registrar entry points preserve that property and replace the declaration
builder surface; no second erasure or catalog runtime is introduced.

Locked at `6702367a8` before implementation on 2026-11-17. This worker owns
runtime construction/start/stop/run and application/service/carrier hook names,
the four SDK registration and lifecycle helpers, Instance/guest lifecycle protocol,
placement lifecycle calls, their conformance fixtures and affected examples/docs.
Host-client deletion and manager ownership remain with the other workers.

| Owner | Sources | Consumers |
| --- | --- | --- |
| Host Lifecycle | `crates/harmos-runtime/src/runtime.rs`, `runtime/launch.rs`, `runtime/lifecycle.rs` | Runtime stories/benches, Schematic, conformance |
| Resource Hooks | `crates/harmos-operations/src/execution/**`, runtime services/authored/component adapters | Application, service, listener and storage carrier implementations |
| SDK Lifecycle/Registration | `sdk/{rust,python,csharp,cpp}` authoring, serving and guest helpers | SDK fixtures/examples, conformance, Schematic sidecar/guest |
| Carrier Lifecycle | `proto/harmos/plugin/v1/{instance,guest}.proto`, model declarations, guest declaration/dispatch and gRPC placement | Generated tracked bindings, reference servers and protocol tests |

Resource listener and manager shutdown APIs retain their names; only runtime host
verbs change. Per-invocation guest finalize remains distinct from plugin finalize.
The initial state is a required construction argument in the current runtime.
`Builder::new(origin)` constructs the existing registration builder; application
`Runtime::start(origin, config)` retains application-owned preparation.
`run` accepts the product's stop-request future, then settles stop when that future
resolves. This avoids library-owned signal policy and permits deterministic proof.
The worker task overrides the original tasks.md prohibition on revision pins.

Constructor deviation from D1: the running handle remains `Runtime<A>` and the
pending registration value is `Builder<A>`. Its constructor is
`Builder::new(origin) -> Self`; `Runtime::new -> Builder` fails the mandatory
`clippy::new_ret_no_self` gate. The old `Runtime::builder` is deleted rather than
retained as an alias. Replacing the running handle type would unnecessarily
cross the parallel runtime-ownership group's source boundary.

## Qualified Name Inventory

The lifecycle caller migration also qualified the private aliases encountered in
its files. Rust names migrated include `GuestError`, `WireCancel`, `WireResult`,
`WireWireResult`, `WireFrame`, `StdResult`, `StorageError`, `QueryError`, `Storage`,
`EngineManager`, `EngineWorker`, `TraceContext`, `TaskContext`, `RequestFrame`,
`WireFailure`, `WireKind`, `ProtocolDuration`, `SleepOperation`, `Batch`,
`DeliveryContext`, `EntryState`, `RuntimeResult`, `Graph`, `RuntimeError`,
`ExchangeOperation`, `AnalyzeOperation`, `UploadOperation`, `OperationError`,
`DownloadTransfer`, `PutTransfer`, `TakeTransfer`, `UploadTransfer`, `ProtoDuration`,
`ChangeKind`, `PlacementKind`, `SettingKind`, `RpcServer`, `InvocationRequest`,
`InvocationResponse`, `Service` and `ProtoMessage`; module/function aliases
`config`, `reflection`, `blocking`, `placement` and `add` now use their owners.
C++ namespace aliases `chrono`, `wire`, `authoring`, `fixture_types`, `sdk` and
`host_wire` are gone in touched sources; same-name message aliases became ordinary
using declarations. C# `SdkContext` and `SdkPlugin` are qualified, and same-name
message/status aliases became imports or qualified references. Python `WireWindow`
and `JsonValue` use their owning modules, and `WireFailure` uses `FailureValue`.

Anonymous Rust trait imports bring colliding extension traits into method scope
without introducing a renamed symbol. The SDK's `extern crate self as harmos`
serves derive expansion inside its defining crate. Composed C++ callback/container
type definitions (`Observe`, `Requirements`, `ValueOperation`) are definitions,
not imported renames.

The existing public runtime clock reexports (`Clock as Source`, `Execution as
Clock`), shared integration/benchmark test path overrides and generated public
payload renames remain open. Renaming that public clock surface or relocating
shared fixture modules overlaps other workers' consumers; the requested scope
decision is pending. Host-client generator aliases belong with the parallel
host-client removal. These are unresolved items, not accepted exceptions.

## Native Phase Integration Boundary

`operations/manager/entry.rs::retirement_control` recognizes reserved native
lifecycle bindings alongside the separately assigned SDK `Action::Shutdown`.
`GrpcAdmission` retains initialize/finalize entries outside ordinary routing and
uses the existing dispatcher, operation policy and settlement authority. Native
admission gates, failure rollback and participant settlement are implemented;
the Rust, C# and C++ authoring and serving layers still require their assigned
operation encoding and callback removal. Wasm hook replacement and combined
platform validation remain open, so this native host/Python delivery does not
complete the entire plugin-lifecycle change.

The required C++ language check exposed registration callback parameter shadowing
and generic resource-constructor shadowing. Catalog callbacks now name the registrar
explicitly, and the shared C++ renderer names its incoming constructor argument
`connection` while retaining the existing field and constructor type. This is a
minimal generator correction required for the existing shadowing gate; host-client
surface removal stays with its owner. Outputs are regenerated, never hand-edited.

Runtime feature-gated fields and statement blocks in `runtime.rs` and
`runtime/lifecycle.rs` still need the runtime owner's module reorganization.
This checkpoint does not claim company-style closure for those existing layouts;
no new item-level feature gates are introduced. Test-only SDK imports are now
inside their test module instead of individually gated imports.
