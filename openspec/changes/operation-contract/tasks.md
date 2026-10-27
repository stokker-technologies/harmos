## 1. Guest Contract Completion

- [x] 1.1 Re-export the shared SDK Operation and Context; isolate kit lifecycle Context and remove guest-root Handler/OperationContext
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
- [ ] 2.4 Commit closed steps locally with the requested trailer; no push or PR.
