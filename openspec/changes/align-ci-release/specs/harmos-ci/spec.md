## ADDED Requirements

### Requirement: Project-Owned CI
Harmos SHALL own CI workflows, actions and tooling. Hub baselines SHALL overwrite
CI only through explicit rendering. Hosted runners and existing caches SHALL remain.

#### Scenario: Development Push
- WHEN dev CI plans a source change
- THEN checks, package producers and Tests compilation run on both platforms without suites or release dry validation
- AND PRs retain selected suites and nightly retains the complete workload
- AND merges to dev remain ungated

### Requirement: Release Evidence
Release reviews and publication SHALL reject resolved git dependencies.
Nightly SHALL warn and skip incompatible crate dry publication.
Release tagging and publication SHALL require green exact-commit required CI.

#### Scenario: Missing Or Failed Evidence
- WHEN the target SHA lacks successful dev CI or a main ruleset required context
- THEN tagging and publication wait for pending evidence and refuse failed evidence
- AND repository settings remain owner-managed

### Requirement: Atomic Development Downloads
One serialized publisher SHALL replace the mutable dev-channel release after
a green dev build-and-package run, consuming its exact-SHA artifacts for both
platforms and all distribution assets in their package catalogs.

#### Scenario: Interrupted Upload
- WHEN any upload fails before the full inventory including SHA256SUMS is verified
- THEN the release remains drafted and the tag remains at its previous revision
- AND retry recovers the same release before moving the tag once and publishing

#### Scenario: Complete Packages
- WHEN both platforms' catalogs are staged
- THEN tool archives, Schematic ZIPs and target-qualified plugin bundles are checksum-covered downloads
- AND missing, changed or conflicting catalog bytes refuse publication
