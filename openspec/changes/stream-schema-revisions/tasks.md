## Implementation

- [x] 1.1 Core and Runtime: Canonical revision derivation, collision and owner validation, explicit committed current selection, atomic staging and rollback, bounded retired windows.
- [x] 1.2 Producers and Consumers: Remove authored stream versions and generated constants; migrate Rust, guest, sidecar, listeners, C++ prototype, templates and affected documentation. Review README impact.
- [x] 1.3 Deterministic Proof: Reload, restart on one storage root, both row definitions, rollback, label stability, canonical golden and counting retirement proof.

## Validation and Delivery

- [x] 2.1 Run focused compilation and affected deterministic tests with eight build jobs; no full lanes, lint or simulations under the owner's explicit gate.
- [ ] 2.2 Run the required diff scanner and require CLEAN; prepare the authorized candidate for immediate squash delivery.
