# Canonical Build

## ADDED Requirements

### Requirement: Canonical Native Configuration

All provider CI Product and Tests SHALL use existing canonical shipping feature configurations per platform in independent parallel producer jobs. Independent feature sets are permitted. Clippy SHALL precede compilation with matching configuration. Simulation and other noncanonical test features SHALL NOT enter CI or shipping binaries. Local installers SHALL NOT build tests by default.

#### Scenario: Ordinary Review

- **WHEN** an ordinary review requests Product and Tests
- **THEN** independent Product and Tests jobs produce separate exports, and downstream Test and Package consume their declared inputs without rebuilding; fixture preparation may join both manifests without compilation

### Requirement: Default Cargo Profiles

Product SHALL use default Cargo dev or release settings, Bench SHALL use default bench settings, and all Tests and their compiler checks SHALL use the default unoptimized test profile. No manifest, package or build environment optimization overrides SHALL remain. Graph profile namespaces SHALL remain separate from compiler profiles.

#### Scenario: Release Tests

- **WHEN** a release pipeline builds Tests
- **THEN** its test harness and test-owned fixtures compile with Cargo's test profile while their artifacts remain in the release graph namespace and receipts preserve the actual compiler profile

### Requirement: Measured Build Budget

Tests Build SHALL take no longer than Product Build for the same platform/profile under comparable cache conditions. The complete warm-cache pipeline SHALL finish below 45 minutes, with 30 minutes advisory. Cold timings SHALL remain separate, and warm-cache claims SHALL rely on observed restoration/compiler reuse. Breaches SHALL trigger investigation and repair without silent coverage removal; correctness-test deadlines SHALL NOT serve as performance criteria.

#### Scenario: Tests Exceeds Product

- **WHEN** comparable-cache Tests Build takes longer than Product Build
- **THEN** delivery requires reassessment of redundant compilation, feature scope or test structure, including moved work in the actual critical path

### Requirement: Compatibility Coverage

The explicit optional local `test:rust:extended` suite SHALL preserve noncanonical behavioral coverage and invoke `build:rust:compatibility` for declared all-features, no-default-features and cross-target compatibility checks. No provider CI context or ordinary installer SHALL invoke either optional task implicitly. Existing test assertions SHALL remain intact.

#### Scenario: Explicit Extended Validation

- **WHEN** a developer requests `mise run test:rust:extended`
- **THEN** the optional task builds its required artifacts and runs all preserved feature-dependent stories and compatibility checks


### Requirement: Correctness And Performance Budgets

Every correctness test SHALL have at least a 120-second deadline, preserving larger bounds. Historical runtime measurements SHALL NOT tighten correctness deadlines. Performance SHALL be assessed through benchmarks, with the application objective remaining under 10 seconds. Cargo defaults and product runtime limits SHALL remain unchanged.

#### Scenario: Slow System Story

- **WHEN** a System story exceeds its former optimized-runtime deadline
- **THEN** the correctness test retains the 120-second minimum while each observed failure is investigated separately for fixture I/O, validation/JIT, startup and actual-operation cost; the application performance objective remains open until measured
