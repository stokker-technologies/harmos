# Tasks

## 1. Contract

- [x] 1.1 Recheck the locations in `design.md` against the current tree and lock the inventory per crate.

## 2. Shared Modules

- [x] 2.1 Add the runtime launch, catalog registration and shutdown module and move existing tests onto it.
- [x] 2.2 Merge the two storage fixture shells into one and add the wait-for-event module, replacing poll and yield helpers.

## 3. Stories

- [x] 3.1 Rewrite the operations groups (retries, authored request lifecycle) as one story each.
- [x] 3.2 Rewrite the state undo group and the mutation receipts group as one story each.
- [x] 3.3 Rewrite the stored waveform query group as one story.
- [ ] 3.4 Generate the binding packages once and check every consumer facet against them.

## 4. Workloads and Time

- [x] 4.1 Move the large projection, journal and quantile workloads to benchmarks and keep the smallest boundary-crossing case as the test.
- [ ] 4.2 Replace the remaining real-time dependencies in design.md with a controlled clock or awaited event, preserving hang guards and existing System serialization.
- [x] 4.2.1 Migrate the operation clock measurement fixture to the captured host clock.

## 5. Removal

- [ ] 5.1 Remove the tests listed in `design.md`, moving any useful check into a story.

## 6. Final Validation

- [ ] 6.1 Run the affected unit and integration lanes on Linux and Windows and record test counts and lines before and after.

## Workflow Follow-Up

- Tests of the sidecar hosting and transport areas are rewritten under this convention in `grpc-plugins`.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.

## Measurements for 6.1

Counts are Rust `#[test]` / `#[tokio::test]` declarations, not executed lane counts.
Lines include test trees (fixtures/support included) and inline test modules; benches excluded.
SDK derive is counted separately from SDK. Native Linux proof remains open.

| Crate | Tests Before | Test Lines Before | Tests After | Test Lines After |
| --- | ---: | ---: | ---: | ---: |
| harmos-domain | 86 | 4053 | pending | pending |
| harmos-guest | 20 | 3514 | pending | pending |
| harmos-operations | 41 | 1291 | pending | pending |
| harmos-protocol | 3 | 422 | pending | pending |
| harmos-runtime | 594 | 47868 | pending | pending |
| harmos-storage | 171 | 10335 | pending | pending |
| harmos | 10 | 801 | pending | pending |
| harmos-sdk-derive | 7 | 1856 | pending | pending |
| schematic-app | 1 | 307 | pending | pending |
| schematic-storage | 6 | 1745 | pending | pending |

## Locked Inventory

- [ ] 6.2 Record focused Windows proof and final requested command results; preserve 6.1 for Linux/Windows lane proof.

## Ownership Follow-Up

Task 3.4 now targets `tools/packaging/tests/acceptance/bindings.rs`, owned by the concurrent tooling worker. Its source and proof require that owner’s handoff.
