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

## Workflow Follow-Up

- The owner authorizes one bounded tuning pass after profiling, plus Gate 6 despite Gate 5 failure. This supersedes the original ordered stop only for this continuation. No migration, PR, integration merge or second round; fresh origin/dev merges into this branch first.

- This slice is the Windows prototype and measurements only. No SDK move, guest change, protocol crate, wire removal, PR, merge or simulation is authorized.
- A failed gate terminates subsequent gate work; retain its implementation and evidence without a workaround.
- The owner corrected the earlier initialization refusal: missing host.publish assignment is an integration defect, not a measured gate failure. Repair admission, complete Gate 1, then measure Gates 2–6 in order. Stop only at a gate's own measured failure; later gates are unmeasured.
- Locked continuation inventory: prototype.rs and prototype/{client,codec}.rs own the initialization boundary and bounded stream instrumentation; Schematic src/prototype.rs and tests/system/grpc_prototype.rs own process fixtures and ordered measurements; manifests change only for those opt-in fixtures. No production migration. design.md owns evidence and README impact review.
- Gate 5 adds Schematic src/measurement.rs for matched optimized process measurements and exposes the already implemented focused `run` operation in the project Cargo task's CLI choices, documented in docs/tooling.md. This retains its standard preflight and exact-selector Clippy; no one-off task or standalone compilation.
- The original run passed Gates 1–4 and failed Gate 5, leaving Gate 6 unmeasured at that checkpoint. The bounded continuation above supersedes the unmeasured estimate and original stop only within its authorized scope; all migration work remains deferred.
