## REMOVED Requirements

### Requirement: Reserve Local Runners For Crate Compilation

**Reason**: The updated project policy uses GitHub-hosted runners for all jobs.

**Migration**: Remove local job runner overrides while preserving tasks, native platforms, profiles, artifacts, deadlines and integration gates.

## ADDED Requirements

### Requirement: Hosted GitHub Runners

Every GitHub verification, nightly and release job SHALL use the declared GitHub-hosted platform default, including jobs that compile Rust crates directly or through nested tasks.

#### Scenario: Artifact Consumer

- **WHEN** a job consumes existing native artifacts without compiling crates
- **THEN** it runs on the hosted runner for its declared platform.

#### Scenario: Crate Compilation

- **WHEN** Build, acceptance, coverage, Dry, Rehearsal or Tag invokes crate compilation directly or through nested tasks
- **THEN** it runs on the hosted runner for its declared platform without weakening its tasks, profile, deadline or native gates.
