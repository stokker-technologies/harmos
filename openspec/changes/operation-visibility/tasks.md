# Tasks

Locked with the owner on 2026-10-29 for the next Harmos release.

## 1. Implementation

- [ ] 1.1 Rename `Shared` to `Public` across domain, macros (short form), runtime, wire/serialized forms, fixtures, docs and skills; no alias.
- [ ] 1.2 Admit a plugin's calls to all its own Private and Public operations on submission, gRPC and discovery; keep Host refused for plugin callers and the no-self-requirement rule.
- [ ] 1.3 Reject `grpc(...)` on Private operations at compile time and at runtime registration.
- [ ] 1.4 Replace owner-name host detection with a typed caller kind.
- [ ] 1.5 Regressions for every scenario; targeted proof.

## 2. Delivery

- [ ] 2.1 Ships in the next Harmos release; Dokime and HCL adopt the rename with the version pin.
