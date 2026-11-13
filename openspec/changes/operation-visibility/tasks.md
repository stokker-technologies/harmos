# Tasks

## 1. Implementation

Inventory: domain visibility and contracts, macro expansion, runtime admission/discovery,
built-in operations, examples, fixtures and product/authoring guides. The two-level contract
in the proposal supersedes Host visibility. Final native proof and delivery remain separate.

- [x] 1.1 Rename `Shared` to `Public` across domain, macros (short form), runtime, wire/serialized forms, fixtures, docs and skills; no alias.
- [x] 1.2 Admit a plugin's own Private and Public operations without a self-requirement; admit host callers to Private operations and refuse other plugins and clients.
- [x] 1.3 Reject `grpc(...)` on Private operations at compile time and at runtime registration.
- [x] 1.4 Replace owner-name host detection with a typed caller kind and cover a plugin named `host`.
- [x] 1.5 Regressions for every scenario; targeted domain metadata, compiler visibility, runtime registration/owner/self-call and protobuf export proof.

- [x] 1.6 Remove Host visibility and migrate built-in operations to the two-level mapping in the proposal.
- [ ] 1.7 Confirm the built-in Private mapping with the owner.

## 2. Delivery

- [ ] 2.1 Ships in the next Harmos release; Consumers adopt the rename with the version pin.

## Workflow Follow-Up

- [x] 3.1 Verify observer cancellation propagates through runtime invocation and sidecar activation interruption.
- [x] 3.2 Add real-demand release regressions for suspended callbacks and idle tick waits, including successful plugin/runtime shutdown.
- [x] 3.3 Verify the retained stopped-history contract with an unpolled watch and collector call; hand off the zero-lease/Stopped consumer predicate.
- [x] 3.4 Run the normal targeted runtime integration executable for observer-demand stories.
