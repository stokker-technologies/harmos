## ADDED Requirements

### Requirement: One Asset Layout

A stable release and a Nightly SHALL publish the same asset layout: one protocol archive, one
conformance archive per platform, one compliance archive, one index `release.json` and
`SHA256SUMS`.

#### Scenario: Asset List Matches the Layout

- **WHEN** a release or a Nightly finishes publication
- **THEN** every published asset name matches an entry of the layout and no entry is missing

### Requirement: Nightly Reports

Nightly SHALL attach `harmos-reports-<version>.zip` with benchmark results and coverage and SHALL
summarize them in its release notes. A failed benchmark or coverage job SHALL NOT prevent
publication.

#### Scenario: Results Are Published

- **WHEN** a Nightly completes its benchmark and coverage jobs
- **THEN** the reports archive is attached and the notes show coverage per language, benchmarks beyond their threshold and suite results
