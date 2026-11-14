## ADDED Requirements

### Requirement: Generated Typed Client Resources

The Python SDK SHALL expose descriptor-generated resource classes with typed
nested attributes and RPC methods. Unary methods SHALL preserve their request
and awaited response types; server-streaming methods SHALL preserve their
request and async-iteration response types. Per-call metadata, credentials,
deadline override and cancellation SHALL retain native typed gRPC controls.

#### Scenario: Product and Library Composition

- **WHEN** a product generates roots from its descriptors and composes them with the library
- **THEN** a checker SHALL resolve both resource hierarchies, message types and nested resources without dynamic attributes or runtime binding tables

#### Scenario: Caller Control

- **WHEN** a caller sets metadata and a deadline override on a generated streaming call and cancels demand
- **THEN** the carrier SHALL receive those options and stop that demand without changing a subsequent call's defaults

### Requirement: Typed Maintained Packages

The SDK and tooling packages SHALL ship typed-package markers and annotate every
maintained function. Ruff SHALL enable annotation, import, naming, pyupgrade and
bugbear rules alongside its existing correctness families. The pinned type
checker SHALL check SDK source, examples, tests and tooling without blanket
ignores. Dynamic protobuf internals SHALL be narrowed at their boundary.

#### Scenario: Runnable Examples

- **WHEN** the Python SDK checks and stories run
- **THEN** both annotated examples SHALL be checked and exercised through real gRPC host fixtures, using protocol completion and events rather than fixed waits

### Requirement: Descriptor-Owned Generation Inputs

Every SDK language generator SHALL derive its resource inventory in memory from
the protocol descriptors. No serialized resource inventory SHALL be committed,
shipped, written by generation or consumed at runtime.

#### Scenario: Generated Drift

- **WHEN** the Python generator check regenerates library and shared-fixture roots
- **THEN** it SHALL compare them with committed generated classes and leave the maintained tree unchanged
