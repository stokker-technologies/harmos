# Tasks

## 1. Contract

- [ ] 1.1 Confirm the simulation removal with the owner and lock the source inventory: features, exports, consumers, tests, benches, documents and tasks for each facility.

## 2. Removal

- [ ] 2.1 Delete `crates/cargo-harmos` with its templates and every reference to it.
- [ ] 2.2 Delete the JSON storage backend and the benchmarks that depend on it.
- [ ] 2.3 Make statistics and downsampling private to storage, replace the downsampling kernels with `tsdownsample` where equivalent, and delete the public `signal` namespace and its features once Dokime owns FFT.
- [ ] 2.4 Rewrite the state recovery, replay and inverse-law tests on the paused clock, then delete the simulation facility, its feature branches and its benchmark.
- [ ] 2.5 Update the README and product guide.

## 3. Final Validation

- [ ] 3.1 Run focused storage query, stream and state tests on Linux and Windows.

## Workflow Follow-Up

- Dokime moves FFT and drops the removed features when it adopts the revision.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
