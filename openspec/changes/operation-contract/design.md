# Operation Contract Design

## Context

Locked by the owner on 2026-10-27 (operation authoring contract, its refinements, operation dependencies, catalog variant cfg
gating and plugin folder layout). Clean break: no compatibility paths, no deprecated aliases, wire and storage formats stay v1.

## Authoring Shape

```rust
#[harmos::operation(
    id = "host.operations.cancel",          // dotted segments; the host prepends the owner
    response = InstanceState,               // default ()
    policy = POLICY,                        // required
    visibility = Shared,                    // default Private; Shared | Host
    resources = [InstanceAccess],           // Requires<InstanceAccess>
    operations = [other::Read],             // Invokes<other::Read>
    // input = Item / output = Item select StreamInput / StreamOutput / their pair
    grpc(package = "harmos.host.v1", service = "Operations", method = "Cancel"),
)]
pub struct Cancel { pub instance: u64 }      // the struct is the request message

impl Operation for Cancel {
    type Error = Error;
    async fn run(self, ctx: Context<Self>) -> Result<Response<InstanceState>, Error> {
        let access = ctx.resource::<InstanceAccess>();
        let read = ctx.operation(other::Read { .. }).await?;
        ...
    }
}
```

- The macro expands `#[harmos::message]` on the struct (named fields or unit) and adds `Clone`. `codec = manual` keeps an
  authored codec (built-in `product!` codecs, in-process test requests) and leaves Clone to the author. The declaration sets `type Request = Self`.
- Operation IDs match `segment(.segment)*` with `segment = [a-z][a-z0-9_]*`; qualified IDs are `owner.local`, split at the first
  dot. The derived default ID stays the snake-cased struct name.
- `Operation: OperationDeclaration<Request = Self> + Dependent + Clone`. `run` consumes a clone of the request per attempt, so
  retries keep their original request.
- `Context<'a, O>` borrows the attempt: `call() -> &CallContext`, `resource::<T>() -> Arc<T>` where `O: Requires<T>`,
  `streams() -> &O::StreamEndpoints`, `operation(request)` where `O: Invokes<T>`.
  `StreamEndpoints` is derived from the declaration's `Input`/`Output` by a blanket SDK impl.
- `Carrier` is the low-level instance contract (`bind`, `attempt(&mut self, ..)`, `stop`, `cleanup`, `transferred`) used by
  carrier proxies and lifecycle built-ins. Its request and response come from one associated `Declaration`. `Executable`
  selects that carrier; authored operations get `Authored<O>` through a blanket impl. Consumer registration uses
  `RegistrationBatch::operation::<O>()`; the hidden `carrier::<D, C>()` entry is reserved for transport and lifecycle adapters.

## Declared Dependencies And Verification

- `resources = [A, ..]` emits `Requires<A>` and `Dependent::RESOURCES`. Portable facade declarations gate execution on the
  consumer `runtime` feature; role-specific runtime, sidecar and guest attributes emit these bounds unconditionally, independent of path.
- `operations = [T, ..]` emits `Invokes<T>` and `OperationDeclaration::OPERATIONS` (contract level, unconditional).
- Catalogs project `OPERATIONS` (`Callees`); a plugin's required contracts are its explicit `requires` plus its provided
  operations' callees not provided by the plugin itself.
- Three layers:
  - Compile time: `ctx.resource` and `ctx.operation` compile only for declared types.
  - Activation time: registration refuses a batch whose snapshot plus carrier-provided resources lacks a declared resource,
    and a declared operation that is missing, fingerprint-incompatible or, across owners, not `Shared` (`Host` for a host
    caller). Plugin requirements reuse the existing provider selection and contract check.
  - Run time: only the operation `Error`, deadlines and carrier faults remain.
- `ctx.operation` dispatches through a carrier `Caller` inserted in the execution snapshot: native typed admission in the
  runtime (owner-retaining responses), encoded child calls in sidecar and guest kits. Caller call metadata (deadline, lane)
  propagates; the callee's policy applies.

## Invocation, Catalogs And Built-Ins

- `runtime.operation(request)` and `scope.operation(request)` take the operation struct; `RequestFor` and `operation_for` are gone.
- `#[harmos::catalog]` copies each variant's outer attributes (notably `#[cfg]`) onto the variant and every generated entry,
  impl and registration step.
- Built-ins use dotted IDs: `host.catalog`, `host.publish`, `host.operations.{cancel,status,watch}`,
  `harmos.{initialize,shutdown}`, `storage.<verb>` for the 23 storage operations.

## Risks

- Runtime follow-ups belong to `harmos.wt/runtime`, branch `feat/runtime-next`; storage belongs to its sibling owner. Only local
  commits and targeted proof are authorized here. Final validation, delivery and retirement belong to the integration owner.
- `Resource` and `Dependent` live in the SDK because they describe execution resources; domain owns `Requires` and `Invokes`.
- Shared stream access uses an asynchronous receiver mutex; receiving remains serialized and cancellation-safe.
- Stateful carrier and lifecycle machinery retains the internal `Carrier` lifecycle; consumer `Operation` has only consuming `run`.
- Per-attempt request clones copy byte payloads; large transfers belong in streams.
- Feature unification can enable a facade `runtime` without the consumer `runtime`; gating on the consumer feature avoids that.

- Guest roots re-export SDK `Operation`, `Context` and `Response`; plugin lifecycle context stays at `kit::Context`. Remote-only declarations need no local execution implementation.
- Generated messages derive Clone when absent; manual codecs own Clone. Shared retry resources and erased response owners retain their existing ownership.
- Sandbox schema, JSON and SHA-256 development/test optimization retains the unchanged fuel budget and matches schematic schema/JSON settings.

## Runtime-Computed IDs (Owner Decision 2026-10-29)

`Registration::new(lane).operation::<O>(id)?` binds an existing executable type. The existing batch supplies startup, runtime, scope and embedded-plugin registration; no arbitrary schemas/handlers or second binding mechanism.
`OperationDefinition::named::<O>(id)` and `WireDeclaration::of::<O>().contract()?.with_id(id)` retain type-owned schema/policy and compute the fingerprint using the new local ID. Request grants retain scopes under that ID; foreign grants remain invalid. Factory validation rejects other mutations. Static catalog projections remain unchanged.
`runtime.operation(request).id(id)` (including endpoints), `operations.submit_at::<O>(id, request)` and encoded calls use normal dispatch. Duplicate IDs, catalog collisions, missing resources and incompatible metadata retain ordinary atomic refusal.

## Runtime Follow-Up Triage

- Core operation requests, dotted IDs, declared callees/resources and catalog cfg propagation already landed in `1b0630a2`.
- Path-independent execution bounds already landed in `ab85df55`; `harmos-macros/tests/fixtures/facade_runtime_catalogs.rs` exercises all three role paths without a consumer `runtime` feature.
- Hashed field spans already fixed in `1b0630a2` and verified in `ab85df55`; the focused SDK regression passes on this candidate.
- Deterministic CLI readiness already landed in `ab85df55`; `harmos-runtime/tests/system/schematic.rs` awaits Initializing/Ready events and derives its handshake bound from the operation policy.
- Plugin folders, declaration-only benchmark gates and operations-crate orphan guidance already landed in `ab85df55`.
- Owner decision (2026-10-29): future-depth and sealing tasks close without source changes; reopen only for a failing reproducer or measured consumer budget miss.
- Observer cadence keeps its static `u64` millisecond contract and accepts constant expressions; generated inventory rejects zero during const evaluation. No runtime cadence configuration is introduced.
