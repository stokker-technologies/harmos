## Implementation

- [x] 1.1 Inspect storage boundaries and report refusal if module-only gating cannot preserve core runtime behavior.
- [x] 1.2 Remove unused guest dependencies after source and cargo-machete inspection; retain serde_json as a native test dependency and update all three locks.
- [x] 1.3 Audit duplicate families: current upstream constraints prevent a single-bump reduction; retain versions without forced pins.
- [x] 1.4 Measure runtime test compilation, count stories and inspect fixture duplication; retain required profile policy.
- [x] 1.5 Review README impact and record Hub feature-selection guidance; dependency-only removal changes no README claims.
- [ ] 1.6 Resolve guest version alignment in Cargo.toml: the authorized 2026-10-29 experiment unified all locks on Wasmtime 49.0.4/wit-bindgen 0.61.1/wasm-tools 0.258.3 and measured a faster cold build, but required native proof had two integration failures and one system failure (the latter reproduced with the original family). Restore the current pins, retain the comparison and proof evidence in design.md, and leave adoption or integration repairs for the owner's final decision; no downgrade is committed.

## Validation And Delivery

- [x] 2.1 Measure cold before/after for guest dependency cleanup and include evidence in its signed Conventional commit.
- [x] 2.2 Run requested check, feature proofs, runtime unit tests and storage integration stories; repair scoped failures.
- [x] 2.3 Remove owned cold targets and prepare local changes for coordinator adoption without pushing.
- [ ] 2.4 Finish the change after the guest alignment decision and any resulting implementation and validation.
