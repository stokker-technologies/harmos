# Tasks

Locked with the owner on 2026-10-29 for the next Harmos release.

## 1. Implementation

This worktree owns 1.1, 1.3, 1.4 and their 1.5 regressions: domain visibility/contract and inventory metadata, macro operation expansion, runtime registration/admission consumers, shipped operations/examples, fixtures, product guide and authoring skills (including generated provider copies). Rename first, then enforce Private/gRPC admission and run focused domain, macro and runtime proofs. README claims are reviewed with each item. Task 1.2 is already implemented and remains integration-owned; final delivery stays open. No unresolved contract decisions.

- [x] 1.1 Rename `Shared` to `Public` across domain, macros (short form), runtime, wire/serialized forms, fixtures, docs and skills; no alias.
- [ ] 1.2 Admit a plugin's calls to all its own Private and Public operations on submission, gRPC and discovery; keep Host refused for plugin callers and the no-self-requirement rule.
- [x] 1.3 Reject `grpc(...)` on Private operations at compile time and at runtime registration.
- [x] 1.4 Replace owner-name host detection with a typed caller kind. Already satisfied: `operations/manager/admission.rs:97` uses `Source::Host`; `tests/plugin/owners.rs:210` covers a plugin named `host`.
- [x] 1.5 Regressions for every scenario; targeted domain metadata, compiler visibility, runtime registration/owner/self-call and protobuf export proof.

## 2. Delivery

- [ ] 2.1 Ships in the next Harmos release; Dokime and HCL adopt the rename with the version pin.

## Workflow Follow-Up

- [x] 3.1 Triage released observer cancellation in `fix/observer-cancellation`. Propagation already exists: `crates/harmos-runtime/src/services/sidecar/observers.rs:222` cancels the invocation; `crates/harmos-sidecar/src/kit/observer/attachment.rs:164` interrupts its activation on attempt drop and `:233` selects interruption against observation.
- [x] 3.2 Regress a suspended observer callback and an idle tick wait through real demand release, requiring settlement within the existing lifecycle policy and successful plugin/runtime shutdown. Evidence: `crates/harmos-runtime/tests/sidecar/observer_demand.rs:477` and `:482`; the normal integration binary passes all five demand stories. README/lifecycle claims remain accurate; no production behavior changed. Commit locally without pushing.
- [x] 3.3 Compare the supplied Dokime spin/spin2 drafts read-only with Dokime `feat/next-release` and crates.io Harmos 0.1.13. The seven relevant cancellation/serving/demand/history files are identical to 0.1.13. Dokime's `spin2/.../sampling.rs:256,269` counts resident keys as running and waits for their removal; stopped snapshots persist until scope retirement. Dokime must await zero leases and `Snapshot.lifecycle.phase() == OperationPhase::Stopped` within its operation policy instead. `crates/harmos-runtime/tests/sidecar/observer_demand.rs:487` covers the same unpolled publication watch, own collector call, release and successful shutdown, including retained stopped history. No Harmos production fix is indicated; the original subsequent Unavailable is not reproduced by the corrected sequence.
- [x] 3.4 Complete the normal targeted runtime integration executable proof after its LLVM out-of-memory failure. The normal binary now compiles with focused Clippy and passes all five `sidecar::observer_demand::` tests; the disposable harness is no longer the only behavioral proof.
