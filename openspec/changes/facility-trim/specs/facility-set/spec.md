## ADDED Requirements

### Requirement: Lean Facility Set
Harmos SHALL ship only facilities an application consumes or that belong to its runtime, storage
and plugin responsibilities.

#### Scenario: Removed Facilities
- **WHEN** the change completes
- **THEN** the repository contains no `cargo harmos` tool, no JSON storage backend and no public
  signal processing namespace, and no task, document or workflow refers to them

#### Scenario: Query Statistics Unchanged
- **WHEN** a storage query asks for an aggregate or a downsampled series
- **THEN** it returns the same result as before the change

### Requirement: State Properties Stay Tested
State recovery, replay identity and inverse laws SHALL remain covered by tests that do not
depend on the simulation facility.

#### Scenario: Restart
- **WHEN** a runtime with committed state and records restarts
- **THEN** the recovered state equals the state before the restart

#### Scenario: Undo
- **WHEN** a transaction is applied and then undone
- **THEN** the state equals the state before it was applied
