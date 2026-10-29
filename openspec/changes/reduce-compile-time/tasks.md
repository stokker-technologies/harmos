## Implementation

- [x] 1.1 Inspect storage boundaries and report refusal if module-only gating cannot preserve core runtime behavior.
- [x] 1.2 Remove unused guest dependencies after source and cargo-machete inspection; retain serde_json as a native test dependency and update all three locks.
- [x] 1.3 Audit duplicate families: current upstream constraints prevent a single-bump reduction; retain versions without forced pins.
- [x] 1.4 Measure runtime test compilation, count stories and inspect fixture duplication; retain required profile policy.
- [x] 1.5 Review README impact and record Hub feature-selection guidance; dependency-only removal changes no README claims.
- [ ] 1.6 Resolve guest version alignment in Cargo.toml: the current Wasmtime 50.0.0/wit-bindgen 0.63.0 pins retain the mismatched upstream Wasm families; await the owner decision on retaining versions (recommended to preserve async fixes) or testing the older family described in design.md. The integrated dependency refresh does not authorize the deferred downgrade.

## Validation And Delivery

- [x] 2.1 Measure cold before/after for guest dependency cleanup and include evidence in its signed Conventional commit.
- [x] 2.2 Run requested check, feature proofs, runtime unit tests and storage integration stories; repair scoped failures.
- [x] 2.3 Remove owned cold targets and prepare local changes for coordinator adoption without pushing.
- [ ] 2.4 Finish the change after the guest alignment decision and any resulting implementation and validation.
