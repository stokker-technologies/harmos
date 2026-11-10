## ADDED Requirements

### Requirement: Language-Agnostic Entry Points
A developer SHALL be able to prepare, format, lint, check, build, test, benchmark and package the
project through one task each, without knowing which languages it contains.

#### Scenario: Selection
- **WHEN** a developer passes a language or a layer to the test task
- **THEN** it runs only that selection, and without arguments it runs everything

#### Scenario: No Orphan Tasks
- **WHEN** the task list is inspected
- **THEN** every task is called by a developer workflow, another task or CI

### Requirement: Machine Concerns Stay Out of Project Tooling
Project tasks and tooling SHALL NOT check or reclaim disk space, and incremental compilation
SHALL be disabled only by a variable set in CI.

#### Scenario: Local Build
- **WHEN** a developer builds locally
- **THEN** no compiler output is scanned or deleted first, and incremental compilation is used

### Requirement: Workflows Call Tasks
A CI job SHALL run a project task for its work and SHALL NOT carry build, test or packaging logic
of its own.

#### Scenario: Reproducing a Job
- **WHEN** a CI job fails
- **THEN** a developer reproduces it locally by running the task the job names with the same
  arguments

### Requirement: Self-Contained Checkout
Everything needed to prepare, build, test, package and release the project SHALL live in its own repository, so the tooling is this project's own copy and not a shared dependency.

#### Scenario: Fresh Clone
- **WHEN** a developer clones this repository alone and runs the prepare task
- **THEN** every entry point works, with no other repository checked out and no company tooling installed
