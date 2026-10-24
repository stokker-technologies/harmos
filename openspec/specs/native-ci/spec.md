# Native CI Specification

## Purpose
Run parallel native Product and Tests jobs per platform and profile with matched Clippy, independent artifacts and unchanged native integration gates.

## Requirements

### Requirement: Parallel Native Builds

The pipeline SHALL have separate independent Product and Tests jobs per selected platform/profile using canonical shipping configurations. The existing release-tests compiler-profile exception SHALL remain. Test instrumentation and noncanonical feature/target compatibility checks SHALL remain explicit optional local tasks rather than provider CI work.

#### Scenario: Tests Selected

- **WHEN** selected jobs consume native tests
- **THEN** Product and Tests run matched Clippy and compilation independently in parallel and export separate artifacts; downstream consumers declare real artifact dependencies and may join fixture manifests without compilation.

#### Scenario: Installer Only

- **WHEN** a user invokes the development installer task
- **THEN** only required product outputs are built and test binaries are not added by default.

#### Scenario: Native And Wasm Proof

- **WHEN** the canonical candidate is validated
- **THEN** Linux and Windows enforce canonical shipping tests and actual shipping Wasm producers; optional local extended tasks retain noncanonical feature/target coverage.

#### Scenario: Hosted GitHub Runners

- **WHEN** any GitHub verification, nightly or release job is selected
- **THEN** it uses a declared GitHub-hosted platform default, with unchanged tasks, profiles, deadlines and required native gates.
