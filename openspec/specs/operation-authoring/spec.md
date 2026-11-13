# Operation Authoring Specification

## Purpose
Define operation policies, portable request contracts, and invocation behavior.

## Requirements

### Requirement: Actionable Authoring Diagnostics

Authoring diagnostics SHALL identify the offending payload or field and explain the required authoring action rather than relying only on an internal Described trait-bound failure.

#### Scenario: Invalid Payload

- **WHEN** a declaration includes an unsupported request field or a response or stream item lacking the required portable-message contract
- **THEN** diagnostics identify the affected authoring input and explain what must be supplied or corrected.

### Requirement: Automatic Contract Identity

Operation contracts SHALL use deterministic fingerprints instead of manually maintained versions. Fingerprints SHALL cover stable operation identity, encoding identity and complete request, response and stream schemas; invocation retry overrides SHALL not change them.

#### Scenario: Payload Schema Change

- **WHEN** a request, response or stream schema changes, including a nested schema
- **THEN** the contract fingerprint reflects that structural change without a manually edited version.
