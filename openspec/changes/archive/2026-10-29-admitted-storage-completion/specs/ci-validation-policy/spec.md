# CI Validation Policy

## ADDED Requirements

### Requirement: Ordinary Correctness Watchdogs

Ordinary correctness-test watchdogs and fixture operation budgets SHALL use a minimum of 120 seconds and preserve larger existing guards. Intentional timeout semantics, simulated clock inputs, polling intervals, production budgets and measured benchmark limits SHALL remain unchanged. Root and starter policy SHALL declare unit and integration deadlines of 120 seconds and acceptance and system deadlines of 900 seconds.

#### Scenario: Slower Native Runner

- **WHEN** an ordinary System story previously bounded at thirty seconds runs on a slower required native executor
- **THEN** its watchdog allows 120 seconds while retaining the same assertions, canonical build profile and serial System execution policy

#### Scenario: Deadline Semantics Test

- **WHEN** a test intentionally verifies short deadline propagation or cancellation
- **THEN** its semantic duration remains unchanged rather than being replaced with the ordinary hang guard
