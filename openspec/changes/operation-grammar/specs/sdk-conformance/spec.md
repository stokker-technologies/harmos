# SDK Conformance Delta

## ADDED Requirements

### Requirement: Packaged Importable Fixture

The one SDK package in each language SHALL contain a stable public constructor
for the language-neutral conformance plugin and its required generated bindings.
Executable entry points SHALL be thin callers of that definition. A layered SDK
SHALL be able to construct and serve it without importing a command-line module
or compiling library test sources.

#### Scenario: Independent Package Consumer

- **WHEN** a consumer installs the Python wheel or NuGet package, unpacks the Rust crates, or builds the C++ source package with tests disabled
- **THEN** its application imports the public fixture from that SDK alone, serves it and passes every matching reference-host rule

#### Scenario: Layered SDK Fixture

- **WHEN** a layered SDK exposes the library fixture through its own transport integration
- **THEN** the fixture and generated message definitions come from the same installed SDK package and need no library checkout
