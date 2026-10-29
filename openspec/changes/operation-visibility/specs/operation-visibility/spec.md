# Operation Visibility

## ADDED Requirements

### Requirement: Three Visibility Levels

Operations SHALL declare one of `Private` (default; callable by the declaring plugin), `Public` (callable by the declaring plugin, by other plugins that declare it, and by the host application) or `Host` (callable by the host application and its clients only).

#### Scenario: Plugin Calls Its Own Public Operation

- **WHEN** a plugin's template or operation calls a Public operation the same plugin declares
- **THEN** the call is admitted on submission, gRPC and discovery without a self-requirement

#### Scenario: Plugin Calls A Host Operation

- **WHEN** a plugin calls a Host operation
- **THEN** the call is refused as unauthorized

### Requirement: No gRPC On Private Operations

A Private operation SHALL NOT declare a gRPC binding.

#### Scenario: Private Operation With gRPC

- **WHEN** an operation without `Public` or `Host` visibility declares `grpc(...)`
- **THEN** compilation fails, and runtime registration of such a contract is refused

### Requirement: Typed Host Caller

Host authorization SHALL use a typed caller kind rather than an owner name.

#### Scenario: Plugin Named Host

- **WHEN** a plugin whose owner name is `host` calls a Host operation
- **THEN** the call is refused
