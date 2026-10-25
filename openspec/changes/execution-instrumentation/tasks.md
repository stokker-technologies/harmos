# Execution Instrumentation Tasks

## Implementation

- [x] 1.1 Runtime worker: instrument all operation admission/execution/resident/stream paths and state/storage runtime boundaries; propagate context through queues, spawned tasks and carrier paths; add deterministic integration proof.
- [x] 1.2 Storage worker: instrument every storage interface and backend call, queue/worker/query/file/recovery handoffs and shared-batch causal links; add deterministic integration proof, including distinct-subscriber batch inputs.
- [x] 1.3 Report worker: qualify native Perfetto exporter; capture and flush runtime/guest system-story artifacts with readable parent and causal relationships, failure evidence and distinct process identity.
- [x] 1.4 Orchestrator/report worker: enable native Criterion readable artifacts and replace custom benchmark summaries through their Hub owner; preserve native results, baseline identity and regression findings; migrate affected consumers.
- [x] 1.5 Orchestrator: integrate manifests/locks, explicit focused compiler-preflight task support, maintained README/CONTRIBUTING and affected guidance; remove superseded paths.
- [x] 1.6 Orchestrator: review complete operation/storage inventory, exporter semantics and all worker handoffs; run targeted checks and close every source item.
- [x] 1.7 Hub owner: include `.stkr/traces` in `crates/stkr/src/ci/mod.rs` artifact ownership and supply the repaired immutable override; Harmos rerenders without editing Hub source.
- [x] 1.8 Hub owner: remove inherited `STKR_TEST_REPORT_DIR` from benchmark child environments in `src/templates/python/test/run.py`; prove ordinary-leaf isolation and rerender Harmos and its starter copies.
- [x] 1.9 Hub owner: repair hosted nightly benchmark baseline persistence in canonical cache generation; permit trusted scheduled/manual runs and advance immutable cache keys while preserving compatible restore identity.

## Validation And Delivery

- [x] 2.1 Apply the user's delivery authorization through integration into dev; verify the refreshed Hub override and resolve any actual published dependency pins before final proof.
- [ ] 2.2 Push the authorized candidate, verify remote SHA, start required native local simulation alongside hosted checks and enable authorized ready review auto-merge.
- [ ] 2.3 Repair failing proof and report actual local/hosted results without claiming pending checks passed.
- [ ] 2.4 Retire the completed OpenSpec change through its canonical lifecycle after every task is satisfied.
