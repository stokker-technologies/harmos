## 1. Contract

- [x] 1.1 Domain: dotted local operation IDs and qualification, `OperationDeclaration::OPERATIONS`, `Requires`/`Invokes` markers,
  `Resource` and `Dependent`; delete `RequestFor`.
- [x] 1.2 SDK: `Handler`, `Executable`, authored `Operation` with `Context`, `ResourceError`, `resource::<T>()`, derived
  `StreamEndpoints`, the carrier `Caller`, and registration verification of declared resources.
- [x] 1.3 Macros: struct-as-request `#[harmos::operation]` (defaults, short visibility, dotted IDs, `resources`, `operations`,
  `codec = manual`), catalog variant attributes and callee projection, plugin requirement aggregation; compile tests.

## 2. Carriers

- [x] 2.1 Runtime: `operation(request)` invocation, native `Caller`, declared-operation verification at registration, adapters on
  `Handler`.
- [x] 2.2 Sidecar and guest kits: encoded `Caller`, carrier-provided resources, kit adapters.

## 3. Migration

- [x] 3.1 Built-ins: the 30 harmos-operations declarations adopt the contract and dotted IDs.
- [x] 3.2 Consumers: runtime, sidecar, guest and macro fixtures, templates and the schematic example.
- [x] 3.3 Documentation: product guide, authoring skill, architecture skill plugin folder layout, rendered copies.

## 4. Proof

- [x] 4.1 Targeted proof: harmos-macros compile tests, focused tests for harmos-macros, harmos-operations, harmos-runtime and
  harmos-sdk, and the schematic consumer crates.
