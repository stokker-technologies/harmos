## Purpose

Close the storage operation gaps that force products to bypass operations: compound atomic mutation, object and scoped deletion, and storage operations available before or without a fully started runtime.

## ADDED Requirements

### Requirement: Compound Atomic Mutation

Storage SHALL offer one operation that applies an ordered list of object creates, conditional sets, deletes and record appends under one mutation identity in one backend commit, producing one cut and one replayable receipt.

#### Scenario: One Condition Fails

- **WHEN** one conditional set in a compound request names a stale revision
- **THEN** no item takes effect and the reply reports the conflicting item with the action to get again and retry

#### Scenario: Replay After Acceptance

- **WHEN** the same mutation identity and payload are resubmitted after acceptance
- **THEN** the original receipt and cut are returned without applying the items again

#### Scenario: Backend Without Transactions

- **WHEN** the registered backend does not declare compound commit support
- **THEN** the operation refuses with a capability error instead of applying items sequentially

### Requirement: Object Deletion

Storage SHALL delete an object by identity under an optional expected revision, recording a deletion revision. Reads at later cuts SHALL report the object absent; retained earlier cuts SHALL still read it.

#### Scenario: Read After Delete

- **WHEN** an object is deleted and a reader holds a cut taken before the deletion
- **THEN** the retained cut reads the object and a new cut reports it absent

### Requirement: Scoped Deletion

Storage SHALL delete every object, record, sample, projection row and file under one opaque scope in one operation with a replayable receipt. File bytes SHALL be reclaimed only after retained cuts that reference them are released.

#### Scenario: Delete a Scope

- **WHEN** a caller deletes a scope that owns objects, records, samples and files
- **THEN** new cuts contain none of them and their bytes are reclaimed once older cuts release

### Requirement: Storage Before Full Startup

The builder SHALL produce a storage-only runtime that admits storage operations without starting listeners, plugins or services, so products create startup prerequisites through operations.

#### Scenario: Prerequisite Before Listeners

- **WHEN** a product writes a credential journal object before starting its full runtime
- **THEN** the write uses the storage operation and no listener or plugin has started

### Requirement: Offline Maintenance Operations

Storage SHALL offer maintenance operations that copy a consistent backend snapshot to a target location and dispose of backend data, implemented through backend hooks and available on the storage-only runtime.

#### Scenario: Copy for Diagnostics

- **WHEN** a product requests a maintenance copy of its storage
- **THEN** the copy is consistent at one cut and the source remains usable

#### Scenario: Unsupported Maintenance

- **WHEN** the backend implements no maintenance hook for the request
- **THEN** the operation refuses with a capability error naming the hook
