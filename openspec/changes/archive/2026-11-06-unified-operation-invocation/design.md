# Unified Operation Invocation

## Locked Public Shape

```rust
let response = ctx.operation(request).retry(settings.retry).await?;
let value = response.into_owned();
let running = ctx.operation(request).endpoints(endpoints).start().await?;
```

`Invocation<H: OperationHost, O, E = ()>` lives in harmos-sdk. It owns a concrete host, request, optional retry and endpoints. `IntoFuture` resolves through the host, whose default joins the instance returned by `.start()`. Remote hosts resolve awaited calls through their existing scoped wire route; generation lifecycle contexts cannot own submitted handles, so `.start()` retains its attempt-scope requirement. Each host admits options and endpoints through its own dispatch; routing is never an Invocation enum. Runtime hosts carry their lane choice, exposed only by the runtime extension trait. There are no aliases for retired APIs.

## Authorization and Ownership

Handlers retain `O: Invokes<T>` at `Context::operation`; their adapter captures the attempt execution snapshot and its existing authorized Calls dispatch. The dispatch retains scoped admission, ancestor lanes, cancellation and call context. Templates retain `Called<C>` on `Context::operation` and criterion/observer surfaces. Built-in operations currently available without requires remain available through that same method. `StepOperationHost` owns stage routing and local harness selection. Every host reports its associated Error; each surrounding Result converts it.

## Retry Findings and Decision

Verified inherited behavior: `RetryPolicy::retry_after` permits transient failures with unknown replay safety; ambiguous failures require Idempotent or Deduplicated. Preserve macro-declared retry behavior. A call-site retry override that enables retries with unknown replay safety is refused before execution; disabling retry remains permitted. Overrides do not change declaration deadlines. Existing attempt evidence is tracing and live status, not a persisted per-attempt record. Add an awaited per-attempt middleware callback to the execution loop; Dokime persists it in the execution log with operation, instance, stage, zero-based attempt and outcome. The operation execution loop owns attempt numbering; carrier calls transport the optional choice rather than retrying independently.

## Encoded Provider Routing

Native handler adapters must dispatch the already captured provider entry for encoded callees, preserving its owner and generation. A declaration id is not an owner-qualified wire id; reinterpreting it as one can route to another owner. Keep the same scoped authority, admission ancestors and portable context through that dispatch. Prove carrier calls through this path.

## Coupled Adoption

Worktrees: `../harmos.wt/invocation` and `../dokime.wt/invocation`, branch `refactor/unified-operation-invocation` in each. Harmos owns the producer and lands first. Dokime rebases on fresh origin/dev immediately before migration and delivery, adopts the merged producer SHA and migrates any newly landed calls. Existing consumer evidence is `crates/dokime-runtime/src/execution/{calls.rs,context/authoring.rs}`: its plain call returns an owned value and its separate StepHost exposes built-ins. The SDK capability replaces both paths without private plugin edits. No unpublished path overrides enter delivery.

## Validation and Retirement

Prove one handler and one template before repository-wide migration. Focused tests cover admission, cancellation, retries/defaults, attempt metadata/evidence, undeclared calls and both carriers. Run `check:rust` and lint in each repository without a full simulation. Scan each diff for the owner-prohibited names. Merge Harmos then pin and merge Dokime immediately, preserving hooks. Archive this change through `stkr specification finish`, clean owned compiler outputs and retire the exact merged worktrees and branches.
