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
