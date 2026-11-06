## ADDED Requirements

### Requirement: Measured Migration Gate
Migration SHALL proceed only after a prototype on a real plugin passes the recorded gates.

#### Scenario: Gate Fails
- **WHEN** a gate fails against the measured baseline
- **THEN** the old transport stays and the failure and its measurements are recorded

### Requirement: One Sidecar Protocol
After migration, gRPC SHALL be the only sidecar protocol.

#### Scenario: Old Wire Removed
- **WHEN** migration completes
- **THEN** the framed stdin/stdout wire and its adapters no longer exist

### Requirement: Private Local Endpoint
The host SHALL expose the plugin endpoint only to its authenticated child generation.

#### Scenario: Other Local Process
- **WHEN** another local process connects without the generation's credentials
- **THEN** the connection is refused

#### Scenario: Retired Generation
- **WHEN** a retired generation calls the host
- **THEN** the call is refused

### Requirement: Separate Data Plane
Stream data SHALL cross as Arrow batches in one streaming RPC, independent of control calls.

#### Scenario: Cancellation Under Load
- **WHEN** a plugin saturates the data stream and the host cancels an operation
- **THEN** the cancellation arrives without waiting for buffered data to drain

### Requirement: Language-Neutral Contract
The protocol SHALL be defined by checked-in `.proto` files, a behaviour spec and a conformance
suite, none requiring Cargo to consume.

#### Scenario: Definition Drift
- **WHEN** Rust definitions change without updating the checked-in files
- **THEN** a repository check fails

#### Scenario: Conformance
- **WHEN** an SDK in any language runs the conformance suite
- **THEN** it exercises handshake, lifecycle, operations, streams, cancellation and errors
