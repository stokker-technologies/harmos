# release-layout Specification

## Purpose

Define credential-free rehearsal of existing release-profile packages and registry publication.

## Requirements

### Requirement: Credential-Free Rehearsal

Rehearsal SHALL consume existing release-profile packages and dry-run registry
publication with credentials removed, without creating provider releases.

#### Scenario: Local Rehearsal

- **WHEN** release-profile packages are available for the configured targets
- **THEN** rehearsal stages and verifies them and dry-runs crate publication locally
- **AND** no provider release, tag or registry upload is created
