## Implementation

- [x] 1.1 Inspect storage boundaries and report refusal if module-only gating cannot preserve core runtime behavior.
- [x] 1.2 Remove unused guest dependencies after source and cargo-machete inspection; retain serde_json as a native test dependency and update all three locks.
- [x] 1.3 Audit duplicate families: current upstream constraints prevent a single-bump reduction; retain versions without forced pins.
- [x] 1.4 Measure runtime test compilation, count stories and inspect fixture duplication; retain required profile policy.
- [x] 1.5 Review README impact and record Hub feature-selection guidance; dependency-only removal changes no README claims.
- [ ] 1.6 Adopt the owner-approved Wasmtime 49.0.4/wit-bindgen 0.61.1/wasm-tools 0.258.3 family across all locks after repairs; pass the same native Windows integration/system selections and workspace check and record cold measurements and upstream differences.
- [x] 1.7 Repair exact_live_bindings at its root: the sandbox now stages an extra handler in mismatch mode; the unchanged catalog-refusal condition and SDK lifecycle smoke pass on the current family without policy changes.
- [ ] 1.8 Reproduce both gRPC preparation-credit failures on the current family and repair actual-workspace admission in the raw adapter if confirmed; preserve budgets and add bounded-admission regression proof.

## Validation And Delivery

- [x] 2.1 Measure cold before/after for guest dependency cleanup and include evidence in its signed Conventional commit.
- [x] 2.2 Run requested check, feature proofs, runtime unit tests and storage integration stories; repair scoped failures.
- [x] 2.3 Remove owned cold targets and prepare local changes for coordinator adoption without pushing.
- [ ] 2.4 Finish the change after the guest alignment decision and any resulting implementation and validation.
