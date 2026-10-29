# Compilation Specification

## Purpose
Reduce dependency and compilation work while preserving supported Harmos behavior.

## Requirements

### Requirement: Preserve Behavior While Reducing Compilation Work

Dependency simplification SHALL preserve supported runtime, storage and guest behavior without forced transitive pins or profile-policy exceptions.

#### Scenario: Dependency Alignment

- **WHEN** the guest dependency family is aligned
- **THEN** workspace all-feature checks and required runtime stories pass and cold before/after measurements accompany the signed commit

#### Scenario: Entangled Storage

- **WHEN** storage cannot be disabled at existing module boundaries
- **THEN** the implementation remains unchanged and the entanglement is reported
