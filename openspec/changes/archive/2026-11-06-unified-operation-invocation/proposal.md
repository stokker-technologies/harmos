# Unified Operation Invocation

## Why

Runtime builders, handler futures and Dokime template calls currently expose different execution shapes and lose call-site retry choices across carrier boundaries. One SDK invocation must preserve each surface's authority, metadata and declared policy.

## What Changes

- Use `ctx.operation(request).await?` and `Invocation<H: OperationHost, O, E = ()>` everywhere, with `.retry(policy)`, `.endpoints(...)` and `.start()`.
- Rename SDK `Host` to `OperationHost` and Dokime `StepHost` to `StepOperationHost`; retain the Dokime host plugin.
- Remove request-taking `ctx.call`, `.run()`, template `ctx.host()`, the separate handler future, Invocation's routing enum and `.id()`. Preserve no-argument handler `ctx.call()` metadata.
- Return `Response<T>` with its existing value access and `into_owned()`.
- Keep `Invokes<T>` and `Called<C>` authorization on each surface method; preserve existing built-in exemptions and keep runtime-only `.lane()` in an extension trait.
- Carry optional retry through native, sidecar, guest and wire paths. Macro policy remains the sole default; overrides affect only one call and never change deadlines. Refuse unsafe retry overrides on unknown replay safety.
- Preserve admission, cancellation, child metadata, attempt evidence, routing, stages and harness behavior in OperationHost implementations.

## Impact

Harmos SDK/runtime/carrier APIs and examples change together. Dokime adopts the merged Harmos revision afterward, migrates templates, observers, starter and authoring references, and refreshes bindings if affected. The private plugin repository is excluded; delivery reports its mechanical migration. Owner authorization includes immediate squash merges into dev and cleanup, with focused proof, `check:rust` and lint only; no full simulation.
