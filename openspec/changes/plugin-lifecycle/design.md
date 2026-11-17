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
  `@on_stop` in Python (`sdk/python/src/harmos/_contracts.py`), `OnStop(Func<Task>)` in C#
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

- `Instance.Initialize` as a new host-invoked RPC: the owner's "no new host-invoked phase"
  is read as covering registration only, since initialize must follow host admission.
- Finalize reasons (`Stopped`, `InitializeFailed`, `Disconnected`) and SDK-local finalize on
  carrier loss.
- Initialize and finalize policies moving into the model declarations.
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

### D2. Plugin Phases (Proposed)

The sketch uses a fixture product's author API; helper types are internal under
the later `plugin-boundary` owner amendment. Local register freezes declarations,
then host admission runs generic checks and the registered product-kind callback;
initialize follows only after both accept and effective settings resolve.

```rust
fn main() -> Result<()> {
    fixture::Plugin::new("thermal", env!("CARGO_PKG_VERSION"))
        .register(register)?
        .initialize(initialize)
        .finalize(finalize)
        .run()
}

fn register(ctx: &mut fixture::Registrar) -> Result<()> {
    ctx.register(settings::catalog())?;
    ctx.register(operations::catalog())?;
    ctx.register(streams::catalog())?;
    Ok(())
}

async fn initialize(ctx: &Context) -> Result<()> { ctx.insert(Chamber::open(ctx).await?) }
async fn finalize(ctx: &Context, reason: Finalization) -> Result<()> { Chamber::close(ctx).await }
```

- **Admission:** `Instance.Get` (native) and `GuestRegistration` (Wasm) report the frozen
  declarations; the host checks identities, schemas, references, permissions and limits and
  creates the activation. No author code runs on the host's behalf before initialize.
- **Initialize:** host-invoked once per activation after admission and before the first
  invocation or sample window: a new `Instance.Initialize` RPC for native plugins and the
  existing `GuestInitialize` for guests. Its context carries settings, the plugin host
  facilities, declared child calls and cancellation, like the existing Wasm initialization
  context. A plugin without an initialize hook answers immediately.
- **Finalize:** `Instance.Stop` becomes `Instance.Finalize`; `GuestShutdown` becomes
  `GuestFinalize` and the guest `shutdown` policy becomes `finalize`. Before removal or stop the
  host refuses new invocations, cancels in-flight invocations and sample windows, waits for
  them to settle within their policies, then calls finalize. Finalize runs once initialize was
  attempted, including after it failed, timed out or was cancelled; the finalize context
  reports the reason (`Stopped`, `InitializeFailed`, `Disconnected`). A managed plugin exits
  after replying; a remote plugin keeps running and a later connection is a new activation.
- **Lost carrier:** when a native connection closes after initialize was attempted and
  before finalize, the SDK runs finalize locally with `Disconnected`; host facilities are then
  unavailable and calls fail with the existing disconnected failure.
- **Policies:** initialize and finalize policies are declared by the plugin in its
  declarations (moving the guest's lifecycle policies into `harmos.model.v1`), so the callee
  owns them; products override one call through existing overrides only.
- Native `on_stop`, Wasm `on_shutdown` and `on_cleanup` are deleted, not deprecated.
  Alternative rejected: keeping a cooperative plugin stop hook before finalize; cancellation
  already reaches every in-flight call, and a second hook splits release logic.

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
| Python | `def register(ctx: Registrar) -> None` | `ctx.register(settings.catalog())` | `initialize=`, `finalize=` | `run()`, `start()`/`stop()` |
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
- [Finalize after a failed initialize sees partial resources] → finalize receives the reason
  and SDK-owned resources are released by the SDK; authored finalize must tolerate absent
  resources, documented in each SDK guide.
- [Local finalize on carrier loss cannot reach the host] → the reason says so; release logic
  that needs host calls belongs before the loss and is best effort after it.
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

`operations/manager/entry.rs::retirement_control` currently recognizes only staged
SDK `Action::Shutdown`; `submission.rs::admit_entry` refuses other submissions
after operation admission closes. Native gRPC activations use `GrpcAdmission`
instead of that staged lifecycle. The remaining initialize/finalize integration
must use one manager-owned policy/retirement authority, including admission gates
and failure rollback. The lifecycle worker has not added an alternate scheduler,
carrier protocol or teardown bypass while ownership of that integration is
unresolved. The existing native and Wasm plugin hooks remain until that complete
replacement is integrated; this checkpoint does not complete plugin-lifecycle.

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
