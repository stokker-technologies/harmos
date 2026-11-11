# native-sdk-abi Specification

## Purpose
TBD - created by archiving change native-sdk-abi. Update Purpose after archive.

## Requirements

### Requirement: Shared Native Plugin Core

The SDK SHALL translate C++ author declarations over the Rust schema, operation
lifecycle and existing framed sidecar transport without changing Rust plugins.

#### Scenario: Native Author Operation

- **WHEN** a host launches the bench C++ sidecar
- **THEN** it discovers both operation contracts and receives a typed response,
  a nested operation result and decoded Arrow batch contents

### Requirement: Boundary Settlement

The ABI SHALL contain panics and foreign exceptions and settle work before free.

#### Scenario: Cancellation and Failure

- **WHEN** the host cancels capture or an author throws an exception
- **THEN** cancellation settles and the exception becomes an operation failure

#### Scenario: Retirement

- **WHEN** the host retires the plugin generation
- **THEN** callbacks settle and the physical sidecar process exits

#### Scenario: ABI Panic

- **WHEN** Rust panics inside the ABI boundary
- **THEN** the caller receives an error and no unwind escapes

#### Scenario: Cancellation Wait

- **WHEN** a C++ callback waits for cancellation and the host cancels its attempt
- **THEN** the Rust core wakes the callback without polling and the attempt settles

### Requirement: Derived Stream Revisions

The ABI and C++ SDK SHALL declare streams by name and window without authored
revisions; the Rust core SHALL derive revisions from the row schema and window.

#### Scenario: Arrow Publication

- **WHEN** the host activates the C++ sidecar's Arrow stream
- **THEN** the demo publishes through that activation and the host reads the
  current publication without assuming a numeric revision
