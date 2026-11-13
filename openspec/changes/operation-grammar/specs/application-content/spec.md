# Application Content Delta

## ADDED Requirements

### Requirement: Application-Owned Immutable Content

StorageManager SHALL accept one registered ContentSource for immutable references
outside its own history. Resolution SHALL pass the inherited caller authorization
scope unchanged and obtain authoritative length, SHA-256 and a bounded lease with
explicit expiry. The application SHALL retain bytes and metadata; the library SHALL
retain ordering, bounds, digest checking, range framing and the separate faces.
Diagnostics and bundle installation SHALL use the same library content boundary,
with no second content-source trait or persistence owner.

#### Scenario: Client and Plugin Complete a Read

- **WHEN** an authorized client and plugin read application-owned content through their GetFileBytes faces
- **THEN** each follows bounded ranges of the same retained lease to completion and verifies the advertised total length and digest without copying content into library persistence

#### Scenario: Expired Lease

- **WHEN** the controlled clock reaches a lease's explicit expiry and the caller continues its issued read
- **THEN** the library refuses with an error naming GetFileBytes and does not silently resolve a new lease

#### Scenario: Application Refusal and Scope Binding

- **WHEN** the application refuses a reference or a continuation supplies a different caller scope
- **THEN** GetFileBytes refuses without reading or returning content under another caller's lease

#### Scenario: Library-Owned Reference

- **WHEN** a caller reads the library's own published immutable file
- **THEN** existing saved-history selection, reference checks, bounded ranges and digest checks remain in force
