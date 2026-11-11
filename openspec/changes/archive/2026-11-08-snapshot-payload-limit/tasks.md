## Implementation

- [x] 1.1 Implement segment spool, manifest verification and bounded storage admission in harmos-storage.
- [x] 1.2 Implement atomic filesystem snapshot publication without journal format changes; migrate storage fixtures and tests.
- [x] 1.3 Implement runtime segment encoding, entry refusal diagnostics and verified recovery; migrate runtime snapshot tests.
- [x] 1.4 Add deterministic multi-segment replay parity, interrupted-write and oversized-entry coverage.
- [x] 1.5 Update product guide and affected README claims; review all snapshot consumers in Harmos.

## Validation

- [x] 2.1 Run focused runtime state and affected storage tests with CARGO_BUILD_JOBS=8.
- [x] 2.2 Run mise run format and verify an idempotent second pass; run stkr project check with no new findings.
- [x] 2.3 Review final diff and obtain CLEAN from the owner-provided scan script.
