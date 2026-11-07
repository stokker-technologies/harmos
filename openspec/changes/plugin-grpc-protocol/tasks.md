# Tasks

## 1. Prototype Source Inventory

- [x] 1.1 Read the existing transport, unified invocation, SDK-layout proposal and Dokime ingest baseline; preserve Wasm guests and storage Postcard.
- [x] 1.2 Add an opt-in `prototype-grpc` seam in harmos-sidecar: authenticated generation-bound Protobuf control over the existing WireTable, bounded Arrow data RPC, typed Invocation client and retirement.
- [x] 1.3 Opt Schematic's native sidecar into the seam only through the prototype feature; exercise its existing ListDirectories operation in a focused process story.
- [x] 1.4 Add ordered focused proofs for cancellation under blocked output, restart and retired credentials, and slow-consumer memory accounting; stop at the first failed gate.
- [x] 1.5 Measure identical Arrow IPC payloads through gRPC and today's framed stdin/stdout on Windows, repeatedly; record both distributions without changing budgets.
- [x] 1.6 Estimate removable versus added source under the owner's bounded continuation, regardless of Gate 5 outcome.
- [x] 1.7 Review README impact and record measured Gates 1–5, the Gate 5 failure and unmeasured Gate 6 in design.md.

## 2. Validation and Delivery

- [x] 2.1 Record ordered focused mise gate results, run `check:rust` and the required diff scan. No full simulation.
- [x] 2.2 Commit and push feat/plugin-grpc-prototype, verify remote SHA; no PR or merge.
- [x] 2.3 Preserve evidence in design.md, cargo clean only the owned targets, and keep the locked worktree.

## 3. Deferred Migration

- [ ] 3.1 Measure Linux gates and obtain owner direction before any migration.
- [ ] 3.2 Add harmos-protocol, checked-in protocol definitions, behaviour specification and conformance suite only after approval of all gates.
- [ ] 3.3 Migrate Rust SDK and runtime, remove the old wire, and adopt in Dokime only in an authorized follow-up.

## 4. Bounded Tuning Continuation

- [x] 4.1 Profile the unchanged gRPC stream: codec/copies, producer queue, HTTP/2 capacity, TCP and TLS before tuning.
- [x] 4.2 Tune prototype settings and stream ownership once; retain payloads, framed baseline and gate criterion. Owners: prototype.rs, prototype/{client,codec}.rs, Schematic src/{prototype,measurement}.rs and its process gates; manifests only if profiling requires it.
- [x] 4.3 Repeat matched seven-trial Gate 5, retain Gates 1–4, estimate Gate 6 and record evidence, busy-machine observations, remaining bottleneck and hybrid SDK costs in design.md. Review shipping README impact.
- [x] 4.4 Run focused mise proofs, check:rust and the required diff scan; commit, normal-push and verify remote SHA without PR or merge.
- [x] 4.5 Preserve results, cargo clean owned worktree targets and retain the worktree. Stop without a second tuning pass or migration.

## 5. Owner-Authorized Ordered Tuning

- [x] 5.1 Raise the stream window with application consumption credits; retain Gate 4's four-batch and memory assertions. Owners: prototype.rs, prototype/client.rs and the Schematic fixture settings and process stories. Measure this change alone with seven paired alternating repetitions and CPU samples.
- [x] 5.2 If no stop condition has fired, remove the contiguous output copy through codec.rs and the prototype HTTP body, then measure alone. Keep ordinary control on Tonic. Not started: Gate 5 passed both payloads after 5.1.
- [x] 5.3 If no stop condition has fired, measure Windows named pipes separately from TCP using Tonic's supported incoming/connector path; change opt-in manifests only if required. Not started: the owner-required stop fired after 5.1.
- [x] 5.4 If the gap persists without an earlier stop, subdivide residual cost with CPU/ETW profiling. Stop at both-payload Gate 5 success, two consecutive changes each below 10% gain on both payloads, or a profiled gRPC/h2 cost with no available setting or codec remedy. Record measurements, stop decision, SDK costs, updated Gate 6 and README impact in design.md. Gate 5 success stopped tuning; no residual profile was started.
- [x] 5.5 Run focused Gates 2–4 and check:rust; commit, require CLEAN from the owner's scan, normal-push and verify remote SHA. No PR, merge, migration or Dokime edits.
- [x] 5.6 Preserve evidence, run native Cargo cleanup for owned worktree targets and retain the worktree.

## 6. Owner-Directed SDK Simplification

- [x] 6.1 Restore the pre-credit server-streaming Data RPC, ordinary client stream and 64 KiB stream window in kit/prototype.rs, prototype/client.rs and Schematic's prototype, measurement and process fixture settings. Retain every other tuned setting and every Gate 4 assertion.
- [x] 6.2 Apply the owner's 2026-11-07 decision: "i am satisfied with the grpc results, i would try to minimize the machinery needed in the sdks. Above 200 MiB/s is good for now." Gate 5 requires a gRPC median strictly above 200 MiB/s on both unchanged payloads; paired framed measurements remain reported. A sub-floor median above 80% CPU is inconclusive pending a quiet rerun.
- [x] 6.3 Run focused Windows Gates 1–4 and seven alternating paired Gate 5 repetitions with timed CPU samples; attempt Linux Gates 1–5 only using an already available repository container, without provisioning. Record distributions, Linux availability, complete C++/Go costs, the reduced Gate 6 estimate and README impact in design.md. Preserve credits evidence as history.
- [x] 6.4 Run check:rust, strict specification validation and the owner's exact CLEAN scan; commit and normal-push this branch, verify remote SHA, run native cargo clean on owned worktree targets and retain the worktree.

## Workflow Follow-Up

- Section 6 supersedes the historical no-regression criterion and ordered tuning authorization. No further tuning, copy removal, named pipes, migration, PR, merge or Dokime edits; tasks 3.2 and 3.3 remain deferred. Task 3.1 may measure existing Linux infrastructure only. No full simulation is authorized for this bounded prototype continuation.

- On 2026-11-07 the owner authorized the ordered continuation in section 5. This supersedes every earlier no-second-round statement and the earlier Gate 5 stop within this prototype; the new explicit stop conditions control the order. No CI simulation is requested for this Windows-only prototype slice.

- The owner authorizes one bounded tuning pass after profiling, plus Gate 6 despite Gate 5 failure. This supersedes the original ordered stop only for this continuation. No migration, PR, integration merge or second round; fresh origin/dev merges into this branch first.

- This slice is the Windows prototype and measurements only. No SDK move, guest change, protocol crate, wire removal, PR, merge or simulation is authorized.
- A failed gate terminates subsequent gate work; retain its implementation and evidence without a workaround.
- The owner corrected the earlier initialization refusal: missing host.publish assignment is an integration defect, not a measured gate failure. Repair admission, complete Gate 1, then measure Gates 2–6 in order. Stop only at a gate's own measured failure; later gates are unmeasured.
- Locked continuation inventory: prototype.rs and prototype/{client,codec}.rs own the initialization boundary and bounded stream instrumentation; Schematic src/prototype.rs and tests/system/grpc_prototype.rs own process fixtures and ordered measurements; manifests change only for those opt-in fixtures. No production migration. design.md owns evidence and README impact review.
- Gate 5 adds Schematic src/measurement.rs for matched optimized process measurements and exposes the already implemented focused `run` operation in the project Cargo task's CLI choices, documented in docs/tooling.md. This retains its standard preflight and exact-selector Clippy; no one-off task or standalone compilation.
- The original run passed Gates 1–4 and failed Gate 5, leaving Gate 6 unmeasured at that checkpoint. The bounded continuation above supersedes the unmeasured estimate and original stop only within its authorized scope; all migration work remains deferred.
