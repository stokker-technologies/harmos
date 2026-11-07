## MODIFIED Requirements

### Requirement: Safe Stored State
Recovery and snapshots SHALL use derived identities and refuse incompatible host built-in record and transaction definitions, including records preceding a compacted snapshot, naming the definition and advising a fresh storage root. Plugin-owned history SHALL retain its written definition and SHALL NOT require a currently installed plugin to open the root. Historical plugin entries SHALL remain available without automatic conversion into current state.

#### Scenario: Restart
- **WHEN** a root is reopened with unchanged host record and transaction definitions
- **THEN** its state and history replay correctly

#### Scenario: Refusal
- **WHEN** a root is reopened with a different host built-in definition of a stored record or transaction
- **THEN** open refuses without resetting or silently discarding data

#### Scenario: Plugin Changes or Disappears
- **WHEN** a plugin changes shape or is absent at root opening
- **THEN** stored plugin entries remain readable under their written definitions without blocking host recovery
