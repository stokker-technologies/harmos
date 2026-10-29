# release-layout Specification

## Purpose
Defines how product tasks compose into releases, the public release asset inventory and the credential-free release rehearsal.

## Requirements

### Requirement: Product Task Composition

Harmos SHALL use native parallel mise run groups for aggregate task references
that carry arguments, preserving flag and profile forwarding.

#### Scenario: Aggregate Arguments

- **WHEN** an aggregate receives its supported flags or profile
- **THEN** each selected leaf receives those arguments through a native task group

### Requirement: Public Release Inventory

Harmos SHALL publish library crates and its declared Cargo tool archive with
generated changelogs, while keeping example bundles in CI packages only.

#### Scenario: Tool And Example Packaging

- **WHEN** matching native packages are staged for release
- **THEN** downloads contain the Cargo tool archive, CHANGELOG.md and SHA256SUMS
- **AND** Schematic bundles and internal publication manifests are not downloads

#### Scenario: Crate Changelogs And Pins

- **WHEN** library crates are dry-run for publication
- **THEN** each crate contains generated CHANGELOG.md and exact sibling versions
- **AND** temporary source and manifest edits are restored

### Requirement: Credential-Free Rehearsal

Rehearsal SHALL consume existing release-profile packages and dry-run registry
publication with credentials removed, without creating provider releases.

#### Scenario: Local Rehearsal

- **WHEN** release-profile packages are available for the configured targets
- **THEN** rehearsal stages and verifies them and dry-runs crate publication locally
- **AND** no provider release, tag or registry upload is created
