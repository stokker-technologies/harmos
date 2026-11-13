# runtime-defined-operations Specification

## Purpose

Define safe per-call retry overrides for encoded submissions.

## Requirements

### Requirement: Encoded Retry Overrides

Encoded submissions SHALL accept an optional retry policy for one call while
preserving the declaration's default, deadline and attempt accounting.

#### Scenario: Unsafe Replay

- **WHEN** an enabled retry override targets unknown replay safety
- **THEN** admission refuses the override

#### Scenario: Safe Replay

- **WHEN** a replay-safe encoded handler fails transiently
- **THEN** an enabled override retries within the original deadline and records attempts
