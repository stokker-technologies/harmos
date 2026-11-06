# Tasks

## 1. Prototype Source Inventory

- [x] 1.1 Read the existing transport, unified invocation, SDK-layout proposal and Dokime ingest baseline; preserve Wasm guests and storage Postcard.
- [ ] 1.2 Add an opt-in `prototype-grpc` seam in harmos-sidecar: authenticated generation-bound Protobuf control over the existing WireTable, bounded Arrow data RPC, typed Invocation client and retirement.
- [ ] 1.3 Opt Schematic's native sidecar into the seam only through the prototype feature; exercise its existing ListDirectories operation in a focused process story.
- [ ] 1.4 Add ordered focused proofs for cancellation under blocked output, restart and retired credentials, and slow-consumer memory accounting; stop at the first failed gate.
- [ ] 1.5 Measure identical Arrow IPC payloads through gRPC and today's framed stdin/stdout on Windows, repeatedly; record both distributions without changing budgets.
- [ ] 1.6 Estimate removable versus added source, review README impact, and record every gate in design.md.

## 2. Validation and Delivery

- [x] 2.1 Record the focused mise test result, run `check:rust` and the required diff scan. No full simulation; the real-path story failed and later gates were not run.
- [x] 2.2 Commit and push feat/plugin-grpc-prototype, verify remote SHA; no PR or merge.
- [x] 2.3 Preserve evidence in design.md, cargo clean only the owned targets, and keep the locked worktree.

## 3. Deferred Migration

- [ ] 3.1 Measure Linux gates and obtain owner direction before any migration.
- [ ] 3.2 Add harmos-protocol, checked-in protocol definitions, behaviour specification and conformance suite only after approval of all gates.
- [ ] 3.3 Migrate Rust SDK and runtime, remove the old wire, and adopt in Dokime only in an authorized follow-up.

## Workflow Follow-Up

- This slice is the Windows prototype and measurements only. No SDK move, guest change, protocol crate, wire removal, PR, merge or simulation is authorized.
- A failed gate terminates subsequent gate work; retain its implementation and evidence without a workaround.
- Gate 1 failed during real Schematic SDK initialization: host.publish has no host assignment. Per owner instruction, stop prototype implementation and leave Gates 2–6 unproven; design.md retains the failure and diagnostic chain. Source closure is not claimed.
