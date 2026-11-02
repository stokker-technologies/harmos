# Reloadable gRPC Certificate Specification

## Purpose
Replace a live gRPC listener certificate without interrupting existing connections or in-flight RPCs.

## Requirements

### Requirement: Live Certificate Replacement
The listener SHALL preserve existing connections during certificate replacement and validate the next certificate before publication.

#### Scenario: Streaming During Replacement
- **WHEN** names refresh replaces a live TLS certificate
- **THEN** the existing stream continues and new handshakes receive the replacement certificate

### Requirement: Bounded Safe Recovery
The implementation SHALL preserve lifecycle and certificate replacement safety.

#### Scenario: Invalid Replacement Or Shutdown
- **WHEN** replacement material is invalid or the listener shuts down
- **THEN** invalid material leaves the active certificate intact and background work stops before admission closes
