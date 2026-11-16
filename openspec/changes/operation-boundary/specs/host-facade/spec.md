## Purpose

Define the public Rust embedding surface for runtime building, typed operation calls and backend authoring, with internal carrier helpers separate from product author APIs and managers kept private.

## ADDED Requirements

### Requirement: Builder Owns Runtime Construction

The runtime builder SHALL accept the storage backends, object catalog, storage settings and runtime settings and SHALL construct, start and close every manager itself. A product SHALL NOT construct, register or retain a manager.

#### Scenario: Product Assembles a Runtime

- **WHEN** a product passes its backend pair, catalog and settings to the builder and starts it
- **THEN** the runtime owns storage, operation, plugin, stream, record, state and service lifecycles and the product holds only the runtime handle and its typed facade

### Requirement: Operations Are the Calling Surface

After start, a product SHALL reach runtime behavior only through the typed in-process facade, whose methods each dispatch one registered native operation with that operation's registered policy. The runtime handle and the facade SHALL expose no manager, no manager getter and no `Deref` to a manager.

#### Scenario: Manager Access Is Absent

- **WHEN** product code attempts to obtain a storage, operation or plugin manager from the runtime handle
- **THEN** the code does not compile because no such public path exists

### Requirement: Operation Results Carry Operation-Owned Types

Public operation requests and results SHALL contain only operation-owned DTOs and protocol values. Engine, query, reader, table or accumulator types SHALL NOT appear in any public operation signature.

#### Scenario: Row Query Result

- **WHEN** a product lists rows through a storage operation
- **THEN** the result carries an operation-owned batch payload and schema, not an engine query output

### Requirement: Backend Authoring Contracts Stay Public

The facade SHALL export, through one authoring module, the storage and state backend traits, their futures and errors, the complete DTO closure of their signatures, object and index catalogs and payload and schema codecs needed to implement a backend.

#### Scenario: Product Backend Compiles Through the Facade

- **WHEN** a product implements both backend traits importing only the facade's authoring module
- **THEN** the implementation compiles without a direct dependency on any internal Harmos crate

### Requirement: Internal Crates Are Fenced

Products and examples SHALL depend only on the host facade and internal carrier-helper crate. Products SHALL own their author-facing SDKs. A repository check SHALL reject direct dependencies on the domain, query, storage, operations or protocol crates outside Harmos's implementation crates.

#### Scenario: Example Adds an Internal Dependency

- **WHEN** an example manifest declares a direct dependency on the query crate
- **THEN** the repository check fails naming the dependency and the facade to use instead

### Requirement: Facade Calls Pass Typed Values

A facade call SHALL hand its typed request to the native operation and return the typed response without encoding either. No Harmos built-in reachable through the facade SHALL require protobuf encoding in-process.

#### Scenario: In-Process Storage Call

- **WHEN** a product appends samples through the facade's storage group
- **THEN** the native append operation receives the typed request under its registered policy and no protobuf encoding or decoding occurs

#### Scenario: One-Call Override

- **WHEN** a product passes a deadline override on the call context of one facade call
- **THEN** only that invocation uses the override and later calls use the registered policy
