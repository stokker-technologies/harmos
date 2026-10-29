## MODIFIED Requirements

### Requirement: Clean Consumer Migration
The workspace SHALL remove RequestFor, infer_request, operation_for, dependency accessors and DependencyError. Stateful carrier
machinery SHALL use the distinct internal Carrier contract. Consumer documentation SHALL show one operation file with complete Definition Module
and Runtime Module divisions, each including its Tests section. The runtime module SHALL follow definition tests and be execution gated. Guest `Operation`, `Context` and `Response` SHALL
re-export the host-free SDK types. Guest plugin lifecycle context SHALL remain distinct at `kit::Context`. Individual operations
SHALL register through `RegistrationBatch::operation`; catalogs SHALL retain generated registration.

#### Scenario: Consumer Invocation
- **WHEN** a consumer invokes an operation
- **THEN** it passes the operation struct to runtime.operation and configures retry and lane through the invocation builder

#### Scenario: Guest Execution
- **WHEN** a Wasm guest executes an authored operation
- **THEN** it consumes a request clone with declared resources and callees, without authored operation initialization

#### Scenario: Required Remote Declaration
- **WHEN** a runtime-enabled carrier declares a remote callee without implementing it locally
- **THEN** the declaration compiles without a local Operation or Executable implementation

## ADDED Requirements

### Requirement: Runtime-Computed Operation IDs
Registration SHALL bind an existing executable operation type under a runtime-computed local ID through the ordinary batch mechanism. Schema, policy, resources and implementation SHALL come from the type. The selected ID SHALL participate in the fingerprint. No arbitrary runtime schemas or handlers SHALL be accepted.

#### Scenario: Named Typed Operation
- **WHEN** an application or embedded plugin registers a typed operation under a configured ID
- **THEN** catalog metadata, fingerprint, typed dispatch and encoded dispatch agree on that ID and retain the type's contract

#### Scenario: Conflicting Registration
- **WHEN** a batch repeats an ID or collides with an existing catalog entry from the same owner
- **THEN** registration refuses atomically and leaves the original binding intact

### Requirement: Ordinary Host Plugin Owner
The plugin owner `host` SHALL have ordinary admission and grant checks. Its name SHALL neither be reserved nor confer application caller authority. Runtime authority SHALL come from the caller context; actual qualified identity collisions SHALL still be refused.

#### Scenario: Host Owner Admission
- **WHEN** a plugin named `host` registers and invokes a declared shared application operation
- **THEN** it receives the same admission, duplicate and scope-grant checks as any other plugin owner

#### Scenario: Owner Name Confers No Authority
- **WHEN** a plugin named `host` declares a private or Host-only operation from another provider
- **THEN** activation refuses it without granting authority from the name
