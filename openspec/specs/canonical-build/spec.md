# canonical-build Specification

## Purpose

Define canonical native build selection and measured build and performance budgets.

## Requirements

### Requirement: Canonical Native Configuration

All provider CI Product and Tests SHALL use existing canonical shipping feature
configurations per platform in independent parallel producer jobs. Independent feature
sets are permitted. Simulation and other noncanonical test features SHALL NOT enter CI
or shipping binaries.

#### Scenario: Ordinary Review

- **WHEN** an ordinary review requests Product and Tests
- **THEN** independent Product and Tests jobs produce separate exports, and downstream Test and Package consume their declared inputs without rebuilding; fixture preparation may join both manifests without compilation

### Requirement: Measured Build Budget

Tests Build SHALL take no longer than Product Build for the same platform/profile under comparable cache conditions. The complete warm-cache pipeline SHALL finish below 45 minutes, with 30 minutes advisory. Cold timings SHALL remain separate, and warm-cache claims SHALL rely on observed restoration/compiler reuse. Breaches SHALL trigger investigation and repair without silent coverage removal; correctness-test deadlines SHALL NOT serve as performance criteria.

#### Scenario: Tests Exceeds Product

- **WHEN** comparable-cache Tests Build takes longer than Product Build
- **THEN** delivery requires reassessment of redundant compilation, feature scope or test structure, including moved work in the actual critical path

### Requirement: Correctness and Performance Budgets

Every correctness test SHALL have at least a 120-second deadline, preserving larger bounds. Historical runtime measurements SHALL NOT tighten correctness deadlines. Performance SHALL be assessed through benchmarks, with the application objective remaining under 10 seconds. Cargo defaults and product runtime limits SHALL remain unchanged.

#### Scenario: Slow System Story

- **WHEN** a System story exceeds its former optimized-runtime deadline
- **THEN** the correctness test retains the 120-second minimum while each observed failure is investigated separately for fixture I/O, validation/JIT, startup and actual-operation cost; the application performance objective remains open until measured
