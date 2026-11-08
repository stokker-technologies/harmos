## Implementation

- [x] 1.1 Extend domain schema declaration, graph, canonical identity and codecs to retain Serde wire representation, preserving existing golden identities.
- [x] 1.2 Replace the stored shape derive with Serde-aware naming, tagging, payload and omission handling; reject unsupported attributes naming the type.
- [x] 1.3 Add deterministic structural Serde comparisons, identity tests and macro compile-fail coverage.
- [x] 1.4 Update the product guide and review README impact.

## Final Validation and Delivery

- [x] 2.1 Pass targeted domain and macro proofs.
- [x] 2.2 Build CI workspace test targets and run lint:rust; repair failures.
- [x] 2.3 Obtain CLEAN from the supplied scanner for the authorized PR.
