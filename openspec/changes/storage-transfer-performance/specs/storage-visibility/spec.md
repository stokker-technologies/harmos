## ADDED Requirements

### Requirement: Passive Durable Cursor Observation

Storage SHALL expose a passive wait for a saved cursor strictly newer than a supplied cursor from the same store history. It SHALL reuse existing change notification, permit coalescing, and neither request persistence nor clear or retry failures.

#### Scenario: Advance Before Registration

- **WHEN** durable data advances between an observer reading its cursor and starting the wait
- **THEN** the wait returns the newer saved cursor without requiring another change

#### Scenario: Accepted Data And Cancellation

- **WHEN** only accepted pending data changes or a passive wait is cancelled
- **THEN** accepted data SHALL NOT be reported as saved, cancellation SHALL leave persistence state unchanged, and no additional sample retention queue SHALL be created

#### Scenario: Terminal State

- **WHEN** no newer durable cursor is available and storage closes, is unavailable, or has a persistence failure
- **THEN** the wait returns the corresponding explicit error without starting a retry
- **AND** already available newer durable progress is returned before a later wait reports terminal state
