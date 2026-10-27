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
- `Handler` is the low-level instance contract (`initialize`, `run(&mut self, ..)`, `stop`, `cleanup`, `transferred`) used by
  carrier proxies, lifecycle built-ins and runtime fixtures. `Executable` selects a declaration's handler; authored operations
  get `Authored<O>` through a blanket impl. `RegistrationBatch::operation::<O: Executable>()` replaces both registration paths
  and `implementation::<D, H>()` stays for explicit pairs.

## Declared Dependencies And Verification

- `resources = [A, ..]` emits `Requires<A>` and `Dependent::RESOURCES`. Resource types are execution-only, so this projection is
  emitted under the consumer `runtime` feature like catalog `registration()`.
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

- Implementation ownership is this worktree alone, including the inherited partial edits. Task order is domain/SDK, macros,
  carriers, built-ins and all workspace consumers, then documentation and targeted proof. No push or PR is authorized.
- `Resource` and `Dependent` live in the SDK because they describe execution resources; domain owns `Requires` and `Invokes`.
- Shared stream access uses an asynchronous receiver mutex; receiving remains serialized and cancellation-safe.
- Stateful carrier and lifecycle machinery retains the internal `Handler` lifecycle; consumer `Operation` has only consuming `run`.
- The inherited deletion of `authoring-streamline` and folded current specs is retained as its retirement result.

- Per-attempt request clones copy byte payloads; large transfers belong in streams.
- Feature unification can enable a facade `runtime` without the consumer `runtime`; gating on the consumer feature avoids that.

- Schematic uses aggregated callees directly; duplicate explicit requirement catalogs are removed.
- Targeted fixture builds refresh stamped macro libraries because inherited compiler outputs no longer exist.

- Generated messages derive Clone when absent so nested request messages support retry snapshots; manual codecs own Clone.
- Declared native calls reuse the callee cache's erased response owner, preserving identity with direct invocations.

- Guest roots re-export SDK `Operation`, `Context` and `Response`; plugin lifecycle context stays at `kit::Context`.
- Remote-only declarations do not require execution when runtime is enabled. Catalog registration remains generated; individual
  operations register through `RegistrationBatch::operation`, without eager inherent registration bodies.
- Sandbox request mutation is discarded between attempts; shared retry state belongs to declared resources. Per-handler authored
  initialization and cleanup probes are removed; plugin lifecycle and carrier settlement remain distinct.
