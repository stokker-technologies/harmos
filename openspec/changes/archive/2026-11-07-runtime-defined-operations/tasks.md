# Tasks

## Implementation

- [x] 1.1 Add owned encoded bindings in operations registration and prepared-native dispatch, preserving generation and contract validation.
- [x] 1.2 Add encoded retry overrides for manager, caller, child context and plugin callback without changing execution policy.
- [x] 1.3 Expose plugin-scoped observer staging for embedded providers.
- [x] 1.4 Add focused operation, replay-safety, cancellation and generation tests and update the product guide; review README impact.

## Validation

- [x] 2.1 Run focused tests, check:rust and lint, and review every diff with the required-term scan.

## Workflow Follow-Up

- Publish one PR to Harmos dev and squash immediately; adopt its published revision only in Dokime feat/dokime-python, then clean and retire this worktree and branch.
