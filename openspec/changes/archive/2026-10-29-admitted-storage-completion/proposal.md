# Admitted Storage Completion

## Why

Dokime's report publication admitted a file creation just before Harmos's retry ledger expired. The file was staged, but the waiting call received an expiry error instead of its completion. Ordinary thirty-second System guards also failed on a slower hosted runner despite passing the same profile locally.

## What Changes

- Preserve completion ownership for already-admitted file mutations and deletion across retry-ledger expiry, without accepting expired new requests or retries.
- Keep retained memory bounded and preserve cancellation, failure, durable receipt, lease and retention contracts.
- Apply the user-authorized 120-second minimum to ordinary test watchdogs and fixture budgets, preserving longer guards and intentional timeout scenarios.
- Develop against Dokime through an untracked Cargo patch, then adopt a verified published Harmos revision.

## Capabilities

### New Capabilities

- `storage-mutations`: Admitted-call completion and retry identity lifetime.

### Modified Capabilities

- `ci-validation-policy`: Ordinary correctness-test watchdog baseline.

## Impact

The storage receipt ledger and its suspended file/deletion waiters, public-manager regressions, authored test policy and starter configuration, CONTRIBUTING and superseded timeout claims. No public schema or dependency version change is proposed. Work stays on feat/drop-2026-10-25 and the retained worktree is not retired. The paired consumer is Dokime's hub-ci worktree and embedded-server-and-ci change.
