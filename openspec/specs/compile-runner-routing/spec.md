# Compile Runner Routing Specification

## Purpose
Reserve local runner capacity for crate compilation while hosted runners execute artifact consumers and other noncompiling jobs.

## Requirements

### Requirement: Reserve Local Runners For Crate Compilation
Jobs that compile Rust crates, including nested fixture builds and package verification, SHALL use declared local runners. All other jobs SHALL use native hosted runners while preserving required platforms, bootstrap, artifacts and deadlines.

#### Scenario: Artifact Consumer
- **WHEN** a unit, integration, system, benchmark execution or package assembly job consumes existing native artifacts without compiling crates
- **THEN** it runs on the hosted runner for its declared platform

#### Scenario: Nested Compilation
- **WHEN** acceptance, coverage, Dry, Rehearsal or Tag invokes crate compilation directly or through nested tasks
- **THEN** it uses the declared local runner for its native platform
