# Native SDK ABI

## ADDED Requirements

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
