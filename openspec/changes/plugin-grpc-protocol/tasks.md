# Tasks

## 1. Prototype Source Inventory

- [x] 1.1 Read the existing transport, unified invocation, SDK-layout proposal and Dokime ingest baseline; preserve Wasm guests and storage Postcard.
- [x] 1.2 Add an opt-in `prototype-grpc` seam in harmos-sidecar: authenticated generation-bound Protobuf control over the existing WireTable, bounded Arrow data RPC, typed Invocation client and retirement.
- [x] 1.3 Opt Schematic's native sidecar into the seam only through the prototype feature; exercise its existing ListDirectories operation in a focused process story.
- [x] 1.4 Add ordered focused proofs for cancellation under blocked output, restart and retired credentials, and slow-consumer memory accounting; stop at the first failed gate.
- [x] 1.5 Measure identical Arrow IPC payloads through gRPC and today's framed stdin/stdout on Windows, repeatedly; record both distributions without changing budgets.
- [ ] 1.6 Estimate removable versus added source only if Gate 5 passes. Unmeasured after the required stop at Gate 5.
- [x] 1.7 Review README impact and record measured Gates 1–5, the Gate 5 failure and unmeasured Gate 6 in design.md.

## 2. Validation and Delivery

- [x] 2.1 Record ordered focused mise gate results, run `check:rust` and the required diff scan. No full simulation.
- [ ] 2.2 Commit and push feat/plugin-grpc-prototype, verify remote SHA; no PR or merge.
- [ ] 2.3 Preserve evidence in design.md, cargo clean only the owned targets, and keep the locked worktree.

## 3. Deferred Migration

- [ ] 3.1 Measure Linux gates and obtain owner direction before any migration.
- [ ] 3.2 Add harmos-protocol, checked-in protocol definitions, behaviour specification and conformance suite only after approval of all gates.
- [ ] 3.3 Migrate Rust SDK and runtime, remove the old wire, and adopt in Dokime only in an authorized follow-up.

## Workflow Follow-Up

- This slice is the Windows prototype and measurements only. No SDK move, guest change, protocol crate, wire removal, PR, merge or simulation is authorized.
- A failed gate terminates subsequent gate work; retain its implementation and evidence without a workaround.
- The owner corrected the earlier initialization refusal: missing host.publish assignment is an integration defect, not a measured gate failure. Repair admission, complete Gate 1, then measure Gates 2–6 in order. Stop only at a gate's own measured failure; later gates are unmeasured.
- Locked continuation inventory: prototype.rs and prototype/{client,codec}.rs own the initialization boundary and bounded stream instrumentation; Schematic src/prototype.rs and tests/system/grpc_prototype.rs own process fixtures and ordered measurements; manifests change only for those opt-in fixtures. No production migration. design.md owns evidence and README impact review.
- Gate 5 adds Schematic src/measurement.rs for matched optimized process measurements and exposes the already implemented focused `run` operation in the project Cargo task's CLI choices, documented in docs/tooling.md. This retains its standard preflight and exact-selector Clippy; no one-off task or standalone compilation.
- Gates 1–4 passed; Gate 5 failed its completed optimized Arrow comparison for both payload sizes. Gate 6 is unmeasured. The mandated gate stop leaves the estimate and all migration work open; no source-complete migration claim is made.
