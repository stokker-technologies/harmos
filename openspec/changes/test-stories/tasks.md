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
| harmos-runtime | 594 | 47868 | 578 | 44174 |
| harmos-storage | 171 | 10335 | 159 | 10012 |
| harmos | 10 | 801 | 10 | 801 |
| harmos-sdk-derive | 7 | 1856 | 7 | 1856 |
| schematic-app | 1 | 307 | 1 | 307 |
| schematic-storage | 6 | 1745 | 6 | 1745 |

## Second Pass for 6.1

| Crate | Tests at c9f09cfc | Test Lines at c9f09cfc | Tests After | Test Lines After |
| --- | ---: | ---: | ---: | ---: |
| harmos-runtime | 584 | 47819 | 578 | 44174 |
| harmos-storage | 166 | 10532 | 159 | 10012 |

The same inventory method counts Rust test declarations, complete test trees and inline
`#[cfg(test)]` tails; shared source is counted once, benches excluded. Empty skeleton
headings account for 3,524 removed runtime lines and 526 removed storage lines. The remaining
changes include formatting, behavioral consolidation, redundant readbacks and moved workloads; these mechanical
line reductions are not presented as removed behavioral coverage.

Runtime lane capacities and snapshot payload limits are tables; principal availability and
undo share one story. Snapshot launches reuse their existing fixture. Storage file retry,
completion, slice and projection stages share one file, object codec facets share one catalog,
and mapped readers share one snapshot. Nine binding consumer facets share two generated
packages instead of regenerating both per facet. Export refusal, deterministic generation
and aggregate typing retain independent stories, each in its own file.
The edited lane, principal, snapshot, reader, mapping and codec stories each have their own
story file; snapshot setup and backend mechanics remain shared.

The 2,048-file receipt workload lives in the storage query benchmark. A deterministic receipt
admission unit test crosses the configured eviction boundary and verifies pending/owned
receipts survive, the oldest reclaimable outcome is evicted and the newest still replays.
The 730-position, 100,000-word snapshot workload lives in the runtime benchmark. The retained
12-position story still crosses segmentation, compares independent journal replay, replays
keyed receipts and proves undo/redo after a post-checkpoint commit. File round trips retain
one small payload and a 32-byte payload across a 31-byte batch boundary.

- Recovery faults remain: different persistence/cleanup owners must each be interrupted.
- gRPC network/endpoint cases remain where native/transcoded envelopes use different authority or settlement paths.
- Opened/frozen file cases remain where source mutation, cancellation, unlinking or Windows access changes ownership.
- Typed catalog aggregation remains: registration, leaf-error propagation and mutually inverse types are distinct from handwritten enum conversions.

## Initialization and Proof

`e4eb6cf3` moved inventory decoding from domain to operations without moving its development
optimization policy. Both guest manifests now optimize operations at the existing level;
metadata decoding already existed at `8f176ef1`. No fuel, deadline or memory allowance changed.
The pending-shutdown story now inspects the retained typed timeout; the previous Debug
substring stopped matching when `TimedOut` acquired structured policy fields.
Dependency admission stories use paused Tokio time and check propagated refusal codes instead of
diagnostic prose. Its preflight and retirement probes retain the surrounding built-in host
inventory. Guest child routing selects the authored request type before validating its assigned
wire fingerprint, preserving distinct declarations with structurally identical messages.

The Tests producer and Rust lint selector explicitly enable guest hosting and Python generation,
retaining guest System stories and binding consumers after incidental feature unification
was removed by relocation. Shipping features remain unchanged.

Windows focused proof passes: 20 runtime integration stories, four storage integration
stories, the receipt-admission unit test, four binding acceptance stories and the workflow
filter test. The 17 runtime stories relocated into individual files and all four storage
stories were rerun after relocation; each exact selection executed one test. Both original
guest deadline stories pass individually after cleaning guest artifacts. The final serial
guest System selection passes all 37 selected stories (three other System stories filtered
out). Rust, Python, C# and C++ SDK conformance passes.

Format, Rust/Python/repository checks, the shared Tests artifact build and packaging pass. Linux and
full lane counts remain open under 6.1; this task runs focused Windows selections only.
No push or delivery is authorized.
