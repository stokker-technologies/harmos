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

### Requirement: Verified Inventory Replacement

Draft and public inventory checks SHALL download, size and hash every asset regardless
of optional API digest metadata. Nightly reports SHALL preserve the original notes and
replace the complete inventory through the shared draft publisher, guarded by the run
identity and source revision. Report reruns SHALL replace the previous results table.

#### Scenario: Corrupt Download With Matching API Digest

- **WHEN** remote bytes differ while API size and digest metadata still match
- **THEN** publication fails and the previous public inventory remains available

#### Scenario: Report Replacement

- **WHEN** the owning run completes its suites or retries its report
- **THEN** the shared publisher swaps verified reports, index and checksums together and retains the original notes

### Requirement: Oldest Duplicate Run Proceeds

Freshness SHALL suppress a revision only for older queued or running runs; a newer
queued run SHALL NOT prevent the oldest run from proceeding.

#### Scenario: Duplicate Runs Are Queued

- **WHEN** two runs for the same revision reach freshness
- **THEN** the older run proceeds and the newer run skips duplicate work
