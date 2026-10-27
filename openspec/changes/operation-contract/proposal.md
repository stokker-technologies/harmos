# Operation Contract

## Why

Authoring an operation still needs a separate request type, `request =`, `infer_request`, `RequestFor`, an instance built by
`initialize`, three associated types and fallible `dependency::<T>()` lookups. Resource and child-operation needs are invisible
until a call fails at run time, and catalog variants cannot be platform gated.

## What Changes

- The operation struct is its request message. `#[harmos::operation]` derives the message, defaults `response` to `()` and
  `visibility` to `Private` (short forms `Shared` and `Host`) and keeps `policy` required. Operation IDs are dotted lowercase
  segments; the host still prepends the owner. Clean break: `request =`, `infer_request`, `RequestFor`, `operation_for` and
  manual operation instances are removed without aliases.
- `impl Operation` declares only `type Error` and `async fn run(self, ctx: Context<Self>)`. `Context` exposes `call()`,
  infallible `resource::<T>()`, `streams()` and `operation(request)`.
- `resources = [..]` and `operations = [..]` declare what an operation uses. They generate compile-time `Requires<T>` and
  `Invokes<T>` bounds, aggregate into the plugin's declared dependencies and are verified when the batch is registered or the
  plugin activates.
- Renames: `Endpoints` to `StreamEndpoints`, `DependencyError` to `ResourceError`, `dependency::<T>()` to `resource::<T>()`. The
  low-level instance contract becomes `Handler`.
- Runtime invocation is `runtime.operation(Cancel { .. }).retry(..).lane(..).run()`.
- `#[harmos::catalog]` carries variant attributes such as `#[cfg(..)]` into the variant and every projection.
- The 30 built-ins adopt dotted IDs (`host.operations.cancel`, `harmos.initialize`, `storage.append_file`, ...).
- The product guide, authoring skill and architecture skill (plugin folder layout) document the contract.

## Capabilities

### New Capabilities

- `operation-contract`: struct-as-request operations, declared resources and operations, the three verification layers,
  dotted identifiers and cfg-gated catalog variants.

### Modified Capabilities

None.

## Impact

harmos-domain, harmos-macros, harmos-sdk, harmos-runtime, harmos-operations, harmos-sidecar, harmos-guest, the facade, every
fixture, template and the schematic example. Wire and storage formats stay v1; operation IDs and fingerprints change with the
clean break. Dokime adopts in its next Harmos revision bump.
