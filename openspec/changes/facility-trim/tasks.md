# Tasks

## 1. Contract

- [x] 1.1 Lock the source inventory: features, exports, consumers, tests, benches, documents and tasks for each facility.

- [ ] 1.2 Confirm retention of the hand-maintained downsampling kernels with the owner.

## 2. Removal

- [x] 2.1 Delete `crates/cargo-harmos` with its templates and every reference to it.
- [x] 2.2 Delete the JSON storage backend and the benchmarks that depend on it.
- [x] 2.3 Make statistics and downsampling private to storage, keeping the existing kernels (see the proposal), and delete the public `signal` namespace and its features without waiting for Dokime to adopt FFT.
- [x] 2.4 Delete the operation response cache, the external series adapter with axial surveys and tracked reads, and statistical stream batches, with their tests and fixtures.
- [x] 2.5 Update the README and product guide.

## 3. Final Validation

- [ ] 3.1 Run the remaining focused storage query, stream and state proof on Linux and Windows after the combined removals.
- [x] 3.1.1 Run focused native Windows storage tests for the JSON backend removal.

## Workflow Follow-Up

- Dokime moves FFT and drops the removed features when it adopts the revision.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
