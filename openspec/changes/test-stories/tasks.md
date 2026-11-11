# Tasks

## 1. Contract

- [ ] 1.1 Recheck the locations in `design.md` against the current tree and lock the inventory per crate.

## 2. Shared Modules

- [ ] 2.1 Add the runtime launch, catalog registration and shutdown module and move existing tests onto it.
- [ ] 2.2 Merge the two storage fixture shells into one and add the wait-for-event module, replacing poll and yield helpers.

## 3. Stories

- [ ] 3.1 Rewrite the operations groups (retries, authored request lifecycle) as one story each.
- [ ] 3.2 Rewrite the state undo group and the mutation receipts group as one story each.
- [ ] 3.3 Rewrite the stored waveform query group as one story.
- [ ] 3.4 Generate the binding packages once and check every consumer facet against them.

## 4. Workloads and Time

- [ ] 4.1 Move the large projection, journal and quantile workloads to benchmarks and keep the smallest boundary-crossing case as the test.
- [ ] 4.2 Replace the remaining real-time dependencies in design.md with a controlled clock or awaited event, preserving hang guards and existing System serialization.
- [x] 4.2.1 Migrate the operation clock measurement fixture to the captured host clock.

## 5. Removal

- [ ] 5.1 Remove the tests listed in `design.md`, moving any useful check into a story.

## 6. Final Validation

- [ ] 6.1 Run the affected unit and integration lanes on Linux and Windows and record test counts and lines before and after.

## Workflow Follow-Up

- Tests of the sidecar hosting and transport areas are rewritten under this convention in `grpc-plugins`.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
