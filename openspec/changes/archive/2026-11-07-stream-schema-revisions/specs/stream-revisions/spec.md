## ADDED Requirements

### Requirement: Automatic Stable Revisions
The core SHALL derive each plugin stream revision from a canonical encoding of its row schema and window, excluding presentation metadata, without an author version.

#### Scenario: Changed Row Across Reload or Restart
- **WHEN** a plugin changes its row under the same id during reload or after restart on one storage root
- **THEN** it receives a distinct revision and rows of both revisions remain readable with their own definitions

#### Scenario: Canonical Stability
- **WHEN** equivalent schemas arrive with different internal graph numbering or presentation labels
- **THEN** their revisions agree, including a pinned canonical golden example

### Requirement: Committed Current Selection and Retirement
Current lookup SHALL follow the committed activation rather than revision ordering. Staging SHALL remain atomic and rollback SHALL preserve the old current revision. Retired revisions SHALL release their live windows when readers and ingest no longer need them.

#### Scenario: Failed Reload
- **WHEN** a staged replacement fails
- **THEN** the previous revision remains current and its rows remain readable

#### Scenario: Many Reloads
- **WHEN** successive revisions commit and old readers and ingest release them
- **THEN** a counting proof shows retired live windows are released

### Requirement: Frozen Activation Inventory
The core SHALL reject owner changes, native collisions, revision collisions and inventory mutation during transport restart with actionable messages.

#### Scenario: Transport Restart
- **WHEN** a transport attempts to change inventory inside one committed activation
- **THEN** admission fails and directs the caller to reload the plugin
