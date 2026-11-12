## ADDED Requirements

### Requirement: Manager-Owned Client Access
The gRPC manager SHALL expose revision-guarded Listener.Get/Set, durable
Pairings.Create/List/Delete and live Certificates.Create on the client face.

#### Scenario: Pair and Rotate Without Restart
- **WHEN** a client changes the listener and creates a pairing
- **THEN** a second client connects using its pinned certificate and credential
- **AND WHEN** the certificate is regenerated
- **THEN** old trust and credentials refuse new access and a new pairing succeeds without host restart.

#### Scenario: Credential Store Unavailable
- **WHEN** the host-supplied credential store cannot load or atomically commit
- **THEN** the operation fails naming the action and leaves persisted and live access unchanged.

### Requirement: Contributed Host Status
The service manager SHALL supply host/application/library identity, availability
and named health, with host-owned contribution registration and observable changes.

#### Scenario: Host Component Changes
- **WHEN** a registered host component changes state
- **THEN** Get includes its state and reason and an already open Watch observes the change.

### Requirement: Bounded Redacted Status Histories
Logs and TraceEvents SHALL provide bounded position-resolved pages and race-free
watches, refusing expired positions, mismatched continuations and retention gaps.

#### Scenario: Page Then Follow Redacted Events
- **WHEN** clients page a resolved log/trace history and watch from its boundary
- **THEN** pages retain their boundary and watches observe subsequent events
- **AND** registered secrets and sensitive fields are redacted before retention and delivery.

### Requirement: Bounded Ranged Diagnostics
Diagnostics.Get SHALL freeze an archive with host contributions and expose bounded
unary ranges identifying the same content length and digest on every continuation.

#### Scenario: Collect Contributed Diagnostics
- **WHEN** a client reads all ranges of a diagnostics archive
- **THEN** its digest verifies, contributed entries are present, and secrets are redacted.
