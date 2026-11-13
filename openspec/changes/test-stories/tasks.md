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
- [x] 3.4 Generate the binding packages once and check every consumer facet against them.

## 4. Workloads and Time

- [x] 4.1 Move the large projection, journal and quantile workloads to benchmarks and keep the smallest boundary-crossing case as the test.
- [x] 4.2 Replace the remaining real-time dependencies in design.md with a controlled clock or awaited event, preserving hang guards and existing System serialization.
- [x] 4.2.1 Migrate the operation clock measurement fixture to the captured host clock.

## 5. Removal

- [x] 5.1 Remove the tests listed in `design.md`, moving any useful check into a story.

## 6. Final Validation

- [ ] 6.1 Run the affected unit and integration lanes on Linux and Windows and record test counts and lines before and after.

## 7. Developer Feedback

- [x] 7.1 Measure the existing focused selections, package/layer binaries, linking, execution and CPU use on Windows; distinguish cold and warm builds and list deferred guest costs.
- [x] 7.2 Extend the test selection in `tools/python/src/harmos_tools/ci/tasks.py` and a new focused-test module to select a story, package or changed Rust paths without the shared archive; preserve configured hang guards and System serialization. Prove selection with focused tooling stories.
- [x] 7.2.1 Include prepared-fixture consumers on product changes and reject excluded fixture workspaces rather than selecting their parent package.
- [x] 7.3 Consolidate retained slow stories and reduce benchmark workloads to boundary cases, preserving distinct behaviors; keep guest replacement suites unchanged.
- [x] 7.3.1 Gate the runtime benchmark server behind its benchmark fixture feature; enable it in the benchmark producer, consumers and benchmark lint selection without building it for ordinary tests.
- [x] 7.3.2 Remove empty conformance executable test/benchmark harness targets while retaining the executable fixtures and conformance stories.
- [x] 7.4 Document focused invocations in `docs/tooling.md`, review README impact, repeat measurements and run each touched suite through its focused task.
- [x] 7.5 Run the requested format, Rust/Python/repository checks and Tests artifact build; repair owned failures and report native Linux proof separately.
- [x] 7.5.1 Update the feature-catalog compiler fixture's moved policy import and rerun its focused story.
- [x] 7.6 Repair runtime typed admission in the host rebuild and prove closing/admission refusal and charged-request release with both existing assertions unchanged.

## Workflow Follow-Up

- Tests of the sidecar hosting and transport areas are rewritten under this convention in `grpc-plugins`.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
