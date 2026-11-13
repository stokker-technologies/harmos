## ADDED Requirements

### Requirement: Storyline Tests
A test at integration level and above SHALL follow one user journey in one file and read top to
bottom as actions and checks, with mechanics provided by shared modules.

#### Scenario: Reading a Story
- **WHEN** a maintainer opens a test at integration level or above
- **THEN** each stage is named by a short comment and every line is an action or a check, with no
  scenario or assertion hidden in a helper

#### Scenario: Shared Mechanics
- **WHEN** a test needs a running runtime, a registered catalog, a storage fixture or to wait for
  an event
- **THEN** it uses the shared module for it and does not rebuild it

### Requirement: Deterministic Time in Tests
The result of a test SHALL NOT depend on real elapsed time.

#### Scenario: Deadline Behaviour
- **WHEN** a test proves what happens when a deadline passes
- **THEN** it advances a controlled clock and no real time is waited

#### Scenario: Hang Guard
- **WHEN** a test bounds how long it may wait for an external process
- **THEN** the bound only stops a hung test and no assertion depends on the time taken

### Requirement: Workloads Belong to Benchmarks
A correctness test SHALL use the smallest input that crosses the boundary it proves, and larger
workloads SHALL run as benchmarks.

#### Scenario: Large Input
- **WHEN** a behaviour is already proven by a small input
- **THEN** the same check over a large input exists only as a benchmark
