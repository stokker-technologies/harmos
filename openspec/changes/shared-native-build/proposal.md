# Shared Native Build

## Why

The agreed Hub graph consolidates native product and test compilation into one job per platform and profile, removing duplicate setup and artifact transfers while keeping distinct feature sets.

## What Changes

- Consume the validated Hub shared Build generator and canonical mise tasks.
- Use the declared GitHub-hosted platform defaults for every job; remove all local runner job overrides.
- Preserve product features, optional test features, Wasm-target proofs, independent artifacts and required Linux/Windows gates.
- Keep local installer commands product-only and run Test and Package consumers in parallel after Build.

## Impact

The hosted-only routing requirement replaces the earlier compile-runner-routing local-compilation policy.

Owned scope: stkr.toml, generated mise and CI files, affected tooling/task consumers, maintained documentation and required-check names. The Hub owns generators and common recipes. No release is authorized by this change.

