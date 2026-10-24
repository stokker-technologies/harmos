# Compile Runner Routing Specification

## Purpose
Use declared GitHub-hosted platform defaults for every job while preserving native validation, artifacts, deadlines and publication gates.

## Requirements

### Requirement: Hosted GitHub Runners

Every GitHub verification, nightly and release job SHALL use the declared GitHub-hosted platform default, including jobs that compile Rust crates directly or through nested tasks.

#### Scenario: Artifact Consumer

- **WHEN** a job consumes existing native artifacts without compiling crates
- **THEN** it runs on the hosted runner for its declared platform.

#### Scenario: Crate Compilation

- **WHEN** Build, acceptance, coverage, Dry, Rehearsal or Tag invokes crate compilation directly or through nested tasks
- **THEN** it runs on the hosted runner for its declared platform without weakening its tasks, profile, deadline or native gates.
