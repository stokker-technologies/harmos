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

### Requirement: Durable Scoped Cut Ownership

Storage SHALL persist retained-cut ownership in its backend, independently of native registries and reader leases. Scope-qualified ownership SHALL retain only that exact scope's rows and files and SHALL survive backend restart. Scope deletion SHALL atomically release its owners. Independent reader leases SHALL survive deletion until released, after which unreferenced rows and files SHALL be reclaimed.

#### Scenario: Restart with a Sealed Scope

- **WHEN** a product retains a scope cut, closes all managers and readers, and reopens its backend
- **THEN** its persisted handle reopens the same scope image even after deletion of an unrelated scope, without pinning that unrelated scope's data

#### Scenario: Delete an Owner with an Independent Reader

- **WHEN** scope deletion releases a retained owner while an independent reader holds its image
- **THEN** new opens of that owner refuse, the reader remains readable until released, and unreferenced rows and files are then reclaimed

### Requirement: Complete Durable Watch Frontiers

Storage SHALL observe committed compound entries and applied conditional revisions. Conflicts SHALL produce progress without row events. Every issued watch cursor and projection frontier SHALL identify a complete mutation frontier. Bounded row pages MAY retain an earlier complete cursor and replay rows on resume; they SHALL NOT skip a compound suffix.

#### Scenario: Bounded Pages Split a Compound Mutation

- **WHEN** one compound mutation spans several bounded watch pages
- **THEN** row frames retain the preceding complete cut and progress advances only after its selected rows are delivered

#### Scenario: Deletion or Expired History

- **WHEN** a watch crosses object or scope deletion, loses retained history, or opens an expired cut
- **THEN** it emits resync-required and terminates so the product lists a fresh image and reopens from that issued cut

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
