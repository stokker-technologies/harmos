# Proposal

## Why

Adopt the owner-locked Hub `generic-ci-and-release-layout` change, product task 2.2,
in Harmos. The provider-disabled render is already present; product-owned tasks
and release documentation must match its contracts.

## What Changes

- Use native parallel task groups for argument-bearing mise aggregates.
- Keep Schematic bundles as CI packages; publish library crates and the declared
  `cargo-harmos` archive, generated `CHANGELOG.md` and `SHA256SUMS`.
- Retain exact sibling pins and use the rendered temporary crate changelog staging.
- Describe credential-free rehearsal of existing release-profile packages and
  the owner action to register Trusted Publishing against `release.yml`.

## Capabilities

### New Capabilities

- `release-layout`: Harmos adoption of the locked release and task contracts.

## Impact

Product-owned `mise.toml`, `stkr.toml`, README and release guidance. Rendered
tooling is inspected and exercised, never hand-edited. Other parked changes and
worktrees are outside scope. Only targeted local proof and commits are authorized.
