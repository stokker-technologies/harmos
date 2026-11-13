# Resource Store Specification

## Purpose

Define aggregate setup errors before initialization.

## Requirements

### Requirement: Aggregate Setup Errors

Duplicate resource types, duplicate catalogs, missing declared operation resources and
unknown lanes SHALL be typed errors, collected and reported together when prepare returns.
Invalid setup SHALL NOT initialize or serve.

#### Scenario: Multiple Setup Errors

- **WHEN** prepare records duplicate resources/catalogs, missing resources and unknown lanes
- **THEN** launch reports every corresponding error and initialize is never called.

#### Scenario: Invalid Setup Cleanup

- **WHEN** prepare returns an owner but setup validation fails
- **THEN** its cleanup releases prepared resources without initialize or stop, and failed cleanup retains that owner for retry.
