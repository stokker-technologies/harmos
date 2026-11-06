# Tasks

## 1. Implementation

- [ ] 1.1 Move the crate to `sdk/rust` and update workspace members, tasks and CI paths.
- [ ] 1.2 Update project checks or their declared exceptions if they assume `crates/`.
- [ ] 1.3 Update documentation and adopt the revision in Dokime.

## 2. Final Validation

- [ ] 2.1 Run `check:rust` and focused SDK tests.

## Workflow Follow-Up

- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
