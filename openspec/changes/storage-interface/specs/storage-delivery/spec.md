## ADDED Requirements

### Requirement: Original Publication Identity in Storage Delivery
Storage Batch SHALL expose each plugin-owned publication's original ManagedPublication by borrow through its existing publications iterator, together with its stream definition, cursor and state position. Owner, admitted plugin generation and schema SHALL remain those of the retained publication, without a second feed or per-row allocation. Typed select and select_shared SHALL remain host-only because plugin publications carry encoded schemas rather than host StreamRow types.

#### Scenario: Delivery After Plugin Restart
- **WHEN** a plugin admits a publication and restarts before a storage consumer reads the batch
- **THEN** the delivered handle reports the owner, generation and schema admitted for that publication rather than the current activation, and exposes the original row bytes

#### Scenario: Host-Owned Typed Rows
- **WHEN** a batch contains host-owned typed rows alongside plugin-owned publications
- **THEN** select and select_shared return the host rows unchanged while publications exposes the plugin rows with their retained identity
