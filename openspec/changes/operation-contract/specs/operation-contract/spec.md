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
