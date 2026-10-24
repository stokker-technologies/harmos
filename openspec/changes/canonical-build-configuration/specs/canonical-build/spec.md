# Canonical Build

## ADDED Requirements

### Requirement: Canonical Native Configuration

All provider CI Product and Tests SHALL use existing canonical shipping feature configurations per platform and profile in independent parallel producer jobs. Independent feature sets are permitted. Clippy SHALL precede compilation with matching configuration. The existing release-tests compiler-profile exception SHALL remain. Simulation and other noncanonical test features SHALL NOT enter CI or shipping binaries. Local installers SHALL NOT build tests by default.

#### Scenario: Ordinary Review

- **WHEN** an ordinary review requests Product and Tests
- **THEN** independent Product and Tests jobs produce separate exports, and downstream Test and Package consume their declared inputs without rebuilding; fixture preparation may join both manifests without compilation

### Requirement: Measured Build Budget

Tests Build SHALL take no longer than Product Build for the same platform/profile under comparable cache conditions. The complete warm-cache pipeline SHALL finish below 45 minutes, with 30 minutes advisory. Cold timings SHALL remain separate, and warm-cache claims SHALL rely on observed restoration/compiler reuse. Breaches SHALL trigger investigation and repair without silent coverage removal or deadline relaxation.

#### Scenario: Tests Exceeds Product

- **WHEN** comparable-cache Tests Build takes longer than Product Build
- **THEN** delivery requires reassessment of redundant compilation, feature scope or test structure, including moved work in the actual critical path

### Requirement: Compatibility Coverage

The explicit optional local `test:rust:extended` suite SHALL preserve noncanonical behavioral coverage and invoke `build:rust:compatibility` for declared all-features, no-default-features and cross-target compatibility checks. No provider CI context or ordinary installer SHALL invoke either optional task implicitly. Existing test assertions SHALL remain intact.

#### Scenario: Explicit Extended Validation

- **WHEN** a developer requests `mise run test:rust:extended`
- **THEN** the optional task builds its required artifacts and runs all preserved feature-dependent stories and compatibility checks

