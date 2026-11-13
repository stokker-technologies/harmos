## ADDED Requirements

### Requirement: Project-Owned CI
Harmos SHALL own CI workflows, actions and tooling. Shared tooling SHALL NOT render or overwrite project CI. Hosted runners and existing caches SHALL remain.

#### Scenario: Development Push
- WHEN dev CI plans a source change
- THEN checks and Product and Tests compilation run on both platforms without packages, suites or release dry validation
- AND PRs run every applicable job and nightly retains the complete workload
- AND merges to dev remain ungated

### Requirement: Release Evidence
Release reviews and publication SHALL reject resolved git dependencies.
Nightly SHALL warn and skip incompatible crate dry publication.
Release tagging and publication SHALL require green exact-commit required CI.

#### Scenario: Missing or Failed Evidence
- WHEN the target SHA lacks successful dev CI or a main ruleset required context
- THEN tagging and publication wait for pending evidence and refuse failed evidence
- AND repository settings remain owner-managed

### Requirement: Atomic Development Downloads
One serialized publisher SHALL replace the mutable nightly release after the
nightly run's build and package jobs, independent of its suites, consuming its
exact-SHA release-profile artifacts for both platforms and all distribution
assets in their package catalogs.

#### Scenario: Interrupted Upload
- WHEN any upload fails before the full inventory including SHA256SUMS is verified
- THEN the release remains drafted and the tag remains at its previous revision
- AND retry recovers the same release before moving the tag once and publishing

#### Scenario: Complete Packages
- WHEN both platforms' catalogs are staged
- THEN tool archives, Schematic ZIPs and target-qualified plugin bundles are checksum-covered downloads
- AND missing, changed or conflicting catalog bytes refuse publication
