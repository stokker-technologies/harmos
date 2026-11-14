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

### Requirement: C# Solution Ownership
The C# SDK SHALL build, test, format/check and pack every project through `sdk/csharp/Harmos.slnx`, with a pinned SDK, central properties and package versions, committed dependency locks and locked normal restore. The public package id `Harmos` and public namespaces SHALL remain unchanged.

#### Scenario: Generated Root Drift
- **WHEN** a committed library or test client root differs from its descriptor-derived output
- **THEN** check fails with the generation command without rewriting the file, and only generate updates committed roots

#### Scenario: Relocated Build Outputs
- **WHEN** a test or fixture producer locates the C# executable
- **THEN** it reads the MSBuild output manifest under ignored artifacts rather than constructing framework-specific output directories

#### Scenario: Local NuGet Consumption
- **WHEN** an independent .NET application restores Harmos from the local package feed
- **THEN** it consumes the workspace version, authoring APIs and client roots with complete package metadata and SourceLink, while conformance support remains test-owned

#### Scenario: Native Validation
- **WHEN** PR or Nightly validation runs
- **THEN** native Linux and Windows C# lanes check the solution and test compiled producer outputs using only their declared tools
