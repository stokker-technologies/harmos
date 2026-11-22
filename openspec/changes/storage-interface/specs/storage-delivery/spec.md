## ADDED Requirements

### Requirement: Original Publication Identity in Storage Delivery
Storage Batch SHALL expose each plugin-owned publication's original ManagedPublication by borrow through its existing publications iterator, together with its stream definition, cursor and state position. Owner, admitted plugin generation and schema SHALL remain those of the retained publication, without a second feed or per-row allocation. Typed select and select_shared SHALL remain host-only because plugin publications carry encoded schemas rather than host StreamRow types.

#### Scenario: Delivery After Plugin Restart
- **WHEN** a plugin admits a publication and restarts before a storage consumer reads the batch
- **THEN** the delivered handle reports the owner, generation and schema admitted for that publication rather than the current activation, and exposes the original row bytes

#### Scenario: Host-Owned Typed Rows
- **WHEN** a batch contains host-owned typed rows alongside plugin-owned publications
- **THEN** select and select_shared return the host rows unchanged while publications exposes the plugin rows with their retained identity

### Requirement: One Plugin Window Registration
Guest and gRPC plugins SHALL register validated windows and writer leases through one implementation. Every plugin writer SHALL retain the owner, activation generation and schema admitted for it, regardless of transport. Guest-only staged visibility, commit and rollback SHALL remain intact. The superseded connection registration implementation SHALL be removed without a compatibility path or additional per-row allocation.

#### Scenario: gRPC Publication Across Reconnect
- **WHEN** a gRPC plugin publishes and reconnects before a storage consumer reads the retained batch
- **THEN** each delivered publication reports its admitting activation's owner, generation and schema, including rows admitted before reconnect

#### Scenario: Guest Staging Remains Atomic
- **WHEN** a guest activation stages windows and rows before its generation commits or aborts
- **THEN** the shared registration preserves staged visibility, original generation identity and rollback of an aborted activation
