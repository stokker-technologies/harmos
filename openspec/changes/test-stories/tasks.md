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

- [x] 5.1 Remove the tests listed in `design.md`, moving any useful check into a story.

## 6. Final Validation

- [ ] 6.1 Run the affected unit and integration lanes on Linux and Windows and record test counts and lines before and after.

## Workflow Follow-Up

- Tests of the sidecar hosting and transport areas are rewritten under this convention in `grpc-plugins`.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.

## Measurements for 6.1

Counts are Rust `#[test]` / `#[tokio::test]` declarations, not executed lane counts.
Lines include test trees (fixtures/support included) and inline test modules; benches excluded.
SDK derive is counted separately from SDK. Shared source is counted once in its owning crate.
Native Linux proof and full lane counts remain open.

| Crate | Tests Before | Test Lines Before | Tests After | Test Lines After |
| --- | ---: | ---: | ---: | ---: |
| harmos-domain | 86 | 4053 | 86 | 4053 |
| harmos-guest | 20 | 3514 | 19 | 3501 |
| harmos-operations | 41 | 1291 | 40 | 1280 |
| harmos-protocol | 3 | 422 | 3 | 422 |
| harmos-runtime | 594 | 47868 | 584 | 47819 |
| harmos-storage | 171 | 10335 | 166 | 10532 |
| harmos | 10 | 801 | 10 | 801 |
| harmos-sdk-derive | 7 | 1856 | 7 | 1856 |
| schematic-app | 1 | 307 | 1 | 307 |
| schematic-storage | 6 | 1745 | 6 | 1745 |

## Ownership Follow-Up

Task 3.4 targets `tools/packaging/tests/acceptance/bindings.rs::export`, owned by the concurrent tooling worker. Each consumer still regenerates both packages; source and proof require that owner’s handoff.

Task 4.2 has controlled-clock/event source in place. Its two guest System stories fail during
`harmos.initialize`, before the deadline assertions, with `wasm trap: all fuel consumed by WebAssembly`.
The failure persists after rebuilding Tests and joining fixtures. The stack reaches
`crates/harmos-operations/src/inventory/publications.rs::validate_match_with_settings` while decoding
`StreamField` metadata. This is a production initialization repair, outside the authorized
product test-seam edits. Existing fuel, deadline and System serialization settings are retained.
Repair and successful native guest proof remain required before 4.2 can close.

## Windows Focused Measurements for 6.1

| Scope | Current Tests Passed | Current Tests Failed |
| --- | ---: | ---: |
| Runtime Integration | 11 | 0 |
| Storage Integration | 15 | 0 |
| Guest ABI Integration | 1 | 0 |
| Runtime Guest System | 0 | 2 |

Every rewritten story was selected by its exact name. These count unique current test declarations, excluding repeat attempts and feature variants,
not full lanes. No Linux or full test lane was run. The guest failures remain repair obligations.

Required local commands passed: `mise run format`, `mise run check -- --language rust`,
`mise run check -- --language python`, `mise run check -- --repository` and
`mise run build -- --artifact tests`. Rust checks include the new benchmark targets.
Benchmarks were compiled, not measured. No push or delivery was performed.
