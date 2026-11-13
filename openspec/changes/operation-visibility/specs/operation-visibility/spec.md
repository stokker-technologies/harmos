# Operation Visibility

## ADDED Requirements

### Requirement: Two Visibility Levels

Operations SHALL declare `Private` (default; callable by the declaring plugin and host) or `Public` (callable by everyone connected). The Host level SHALL NOT exist.

#### Scenario: Plugin Calls Its Own Public Operation

- **WHEN** a plugin's template or operation calls a Public operation the same plugin declares
- **THEN** the call is admitted on submission, gRPC and discovery without a self-requirement

#### Scenario: Another Plugin Calls a Private Operation

- **WHEN** a plugin calls another plugin's Private operation
- **THEN** the call is refused as unauthorized

### Requirement: No gRPC on Private Operations

A Private operation SHALL NOT declare a gRPC binding.

#### Scenario: Private Operation with gRPC

- **WHEN** an operation with Private visibility declares `grpc(...)`
- **THEN** compilation fails, and runtime registration of such a contract is refused

### Requirement: Typed Host Caller

Host authorization SHALL use a typed caller kind rather than an owner name.

#### Scenario: Plugin Named Host

- **WHEN** a plugin whose owner name is `host` calls another plugin's Private operation
- **THEN** the call is refused
