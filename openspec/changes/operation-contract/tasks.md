## 1. Guest Contract Completion

- [x] 1.1 Re-export the shared SDK Operation and Context; isolate kit lifecycle Context and remove guest-root Handler/ExecutionContext
  authoring exports.
- [x] 1.2 Migrate all seven sandbox guest handlers to request-owned operations in vertical files, declared resources and callees; remove
  superseded per-handler lifecycle probes and replace with request/attempt proofs.
- [x] 1.3 Migrate guest carrier consumers, scaffold templates, shared fixture declarations and affected host assertions; review README and
  update guest guide and skill references.

- [x] 1.4 Remove eager per-operation registration helpers that require remote-only declarations to implement execution; keep catalog
  registration and explicit RegistrationBatch::operation.

- [x] 1.5 Retain genuine format:rust output and revert unrelated aggregate-format/Hub-render changes.

- [x] 1.6 Align sandbox schema build optimization with the schematic guest, retain fuel limits, and repair guest facade contexts and
  qualified encoded-call assertions found by focused proof.

## 2. Targeted Proof

- [x] 2.1 Run format:rust and focused harmos-guest, harmos-macros and harmos-sdk tests.
- [x] 2.2 Build the Wasm sandbox and schematic guest using project tasks; check schematic consumer crates.
- [x] 2.3 Run the affected runtime guest operation stories against the refreshed components.
- [x] 2.4 Commit closed steps locally with the requested trailer; no push or PR.
- [ ] 2.5 Retire the change with `stkr specification finish operation-contract` after Hub disambiguates change/spec validation.

## 3. Workspace Clean Break

- [x] 3.5 Rename the shared execution snapshot and all references without aliases; focused five-package compilation succeeds.
- [ ] 3.6 Emit declared resources for execution-only runtime/SDK macro paths and migrate the first stateless fixtures; preserve runtime gating for reusable guest declarations.

- [ ] 3.1 Audit every remaining lifecycle-shaped implementation; migrate runtime test support, tests, benches and sidecar fixtures to request-owned operations with declared resources, streams and dotted IDs.
- [ ] 3.2 Restrict low-level handlers to carrier machinery; rename the shared execution snapshot and migrate SDK/runtime/service consumers and exports without compatibility aliases.
- [ ] 3.3 Migrate lifecycle built-ins and inspect guest fixtures, examples, scaffold templates and README claims; remove superseded operation lifecycle stories.
- [ ] 3.4 Run the requested five-package focused tests, Wasm/guest fixture builds, schematic consumer check and Rust formatting; record signature counts and commit closed steps locally.
