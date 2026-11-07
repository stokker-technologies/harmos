# runtime-defined-operations Specification

## Purpose
TBD - created by archiving change runtime-defined-operations. Update Purpose after archive.

## Requirements

### Requirement: Owned Encoded Bindings

Embedded plugins SHALL stage runtime-owned canonical contracts and native encoded
handlers through the existing generation-scoped operation registration.

#### Scenario: Runtime Handler

- **WHEN** a runtime-owned handler is invoked after its plugin commits
- **THEN** normal admission, authorization, deadlines, cancellation and settlement apply

#### Scenario: Retired Generation

- **WHEN** a retained callback refers to a retired plugin generation
- **THEN** invocation refuses rather than routing to a replacement

### Requirement: Encoded Retry Overrides

Encoded submissions SHALL accept an optional retry policy for one call while
preserving the declaration's default, deadline and attempt accounting.

#### Scenario: Unsafe Replay

- **WHEN** an enabled retry override targets unknown replay safety
- **THEN** admission refuses the override

#### Scenario: Safe Replay

- **WHEN** a replay-safe encoded handler fails transiently
- **THEN** an enabled override retries within the original deadline and records attempts

### Requirement: Embedded Observer Scope

Public plugin observer staging SHALL preserve the supplied activation and resources.

#### Scenario: Plugin Retirement

- **WHEN** the staged observer's plugin generation retires
- **THEN** its observer stops through normal plugin settlement
