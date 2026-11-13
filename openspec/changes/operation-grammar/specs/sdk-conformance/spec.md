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

### Requirement: Caller-Owned Controls on Generated Roots

Every generated resource root SHALL accept the caller's metadata/credentials,
deadline override and cancellation for one call. C++ methods SHALL take the
caller's `grpc::ClientContext&`, including Watch methods whose readers borrow
that context until completion. No root SHALL replace it with an inaccessible
per-operation context. Rust requests, Python call options and C# CallOptions
SHALL preserve the same per-call concept. All method forwarding SHALL be generated.

#### Scenario: Metadata, Deadline and Cancellation on One Call

- **WHEN** a caller supplies metadata and a deadline override, observes a generated Watch opening and then cancels that call
- **THEN** the metadata and override reach its transport and the server stream settles under caller cancellation in all four SDKs

#### Scenario: Independent Following Call

- **WHEN** the caller starts another resource call
- **THEN** that call uses its own options rather than inheriting the previous call's metadata, deadline or cancellation
