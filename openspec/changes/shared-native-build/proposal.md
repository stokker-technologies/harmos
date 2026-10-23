# Shared Native Build

## Why

The agreed Hub graph consolidates native product and test compilation into one job per platform and profile, removing duplicate setup and artifact transfers while keeping distinct feature sets.

## What Changes

- Consume the validated Hub shared Build generator and canonical mise tasks.
- Route compilation through the existing native Build runner overrides; remove retired check/test-build job overrides.
- Preserve product features, optional test features, Wasm-target proofs, independent artifacts and required Linux/Windows gates.
- Keep local installer commands product-only and run Test and Package consumers in parallel after Build.

## Impact

Owned scope: stkr.toml, generated mise and CI files, affected tooling/task consumers, maintained documentation and required-check names. The Hub owns generators and common recipes. No release is authorized by this change.

