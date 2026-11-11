# Data Shape Identity

## Purpose

Provide stable, canonical identities for data shapes and refuse incompatible stored definitions without data loss.

## ADDED Requirements

### Requirement: Canonical Derived Revisions
Native and plugin streams SHALL share schema/window derivation. Records, transactions, typed stored objects and described system facts SHALL derive schema-only opaque revisions using canonical encoding. Presentation metadata SHALL NOT participate. Revisions SHALL be compared only for equality.

#### Scenario: Unchanged Shape
- **WHEN** a shape is recompiled or its label changes
- **THEN** its revision remains unchanged across processes and platforms

#### Scenario: Changed Shape
- **WHEN** a serialized shape changes
- **THEN** its derived revision changes without an authored ordinal

### Requirement: Safe Stored State
Recovery and snapshots SHALL use derived identities and refuse incompatible stored record definitions, including records preceding a compacted snapshot, naming the record and advising a fresh storage root.

#### Scenario: Restart
- **WHEN** a root is reopened with unchanged record and transaction definitions
- **THEN** its state and history replay correctly

#### Scenario: Refusal
- **WHEN** a root is reopened with a different definition of a stored record
- **THEN** open refuses without resetting or silently discarding data

### Requirement: Identity-Based Listener Binding
Record listeners SHALL declare identity and row schema without authored versions, validating the bound native record schema.

#### Scenario: Listener Registration
- **WHEN** a listener binds a registered record with matching identity and schema
- **THEN** registration succeeds without repeating its revision
