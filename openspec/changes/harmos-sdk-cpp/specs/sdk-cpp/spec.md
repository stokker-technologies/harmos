## ADDED Requirements

### Requirement: C++ Sidecar Authoring
Developers SHALL write a Harmos sidecar in C++ using only `sdk/cpp`.

#### Scenario: Sample Sidecar
- **WHEN** the sample sidecar is built and launched by a Harmos host
- **THEN** it registers, serves an operation, publishes a stream and shuts down in order

#### Scenario: Conformance
- **WHEN** the conformance suite runs against the SDK
- **THEN** it passes on Linux and Windows

### Requirement: Simple Installation
Developers SHALL install the C++ SDK with CMake (`FetchContent` or a package manager), the language's idiomatic modern toolchain, and any
required command-line tools through mise.

#### Scenario: New Project
- **WHEN** a developer follows the install section in a new project
- **THEN** a few documented commands reach a building sample, with no manual downloads
