# Operation Contract Specification

## Purpose
Define request-owned operations, declared dependencies, activation verification, and conditional catalogs.

## Requirements

### Requirement: Operation Structs Are Requests
The operation attribute SHALL define the request from the annotated named-field or unit struct. It SHALL default the response to `()`
and visibility to `Private`, accept short `Shared` and `Host`, and reject removed request-selection metadata. The authored `Operation`
trait SHALL consume the request in `run(self, Context<Self>)`, require only the error associated type, and derive `StreamEndpoints`.

#### Scenario: Unit Operation And Default Response
- **WHEN** a unit struct declares an operation with a policy
- **THEN** its request is that struct, its response is `()`, and its visibility is private

### Requirement: Dotted Operation Identity
Local operation IDs SHALL match `segment(.segment)*`, with each segment matching `[a-z][a-z0-9_]*`. The runtime SHALL qualify the local
ID by prepending its plugin owner. The thirty built-ins SHALL use dotted IDs.

#### Scenario: Invalid Segment
- **WHEN** an operation literal contains an uppercase, empty, or numeric-leading segment
- **THEN** declaration compilation fails

### Requirement: Declared Resources And Operations
The attribute SHALL generate `Requires<T>` and `Invokes<T>` bounds for declared resources and operations. Context resource access SHALL
return `Arc<T>` infallibly after activation verifies availability. Context operation calls SHALL propagate the caller's metadata and
use the callee policy. Catalogs SHALL aggregate callees into plugin requirements, excluding operations provided by the same plugin.

#### Scenario: Undeclared Resource
- **WHEN** an authored operation requests a resource absent from its declaration
- **THEN** compilation fails on the generated resource bound

#### Scenario: Missing Activation Target
- **WHEN** registration lacks a required resource or callee
- **THEN** activation fails before any operation executes

#### Scenario: Cross-Plugin Visibility
- **WHEN** an operation declares a callee belonging to another plugin
- **THEN** activation requires compatible fingerprints and Shared visibility, allowing Host visibility for a host caller

### Requirement: Shared Stream Access
Context SHALL expose the derived stream endpoint set through a shared reference. Unary operations SHALL use `()`. Stream input receives
SHALL serialize without changing cancellation behavior.

#### Scenario: Duplex Operation
- **WHEN** an operation declares input and output items
- **THEN** its derived endpoints are a pair of typed input and output endpoints

### Requirement: Conditional Catalog Variants
Catalog expansion SHALL preserve variant conditions on regenerated variants, portable projections, and runtime registration.

#### Scenario: Disabled Platform Variant
- **WHEN** a catalog variant is disabled by cfg
- **THEN** its referenced type need not resolve and contributes no registration or dependency

### Requirement: Clean Consumer Migration
The workspace SHALL remove RequestFor, infer_request, operation_for, dependency accessors and DependencyError. Internal lifecycle
machinery SHALL use Carrier. Consumer documentation SHALL specify vertical operation modules, shared guest authoring exports and
type-owned registration as described in the scenarios below.

#### Scenario: Consumer Invocation
- **WHEN** a consumer invokes an operation
- **THEN** it passes the operation struct to runtime.operation and configures retry and lane through the invocation builder

#### Scenario: Guest Execution
- **WHEN** a Wasm guest executes an authored operation
- **THEN** it consumes a request clone with declared resources and callees, without authored initialization; Operation, Context and Response re-export host-free SDK types, while plugin lifecycle context remains kit::Context

#### Scenario: Operation Source And Registration
- **WHEN** a consumer authors and registers an executable operation
- **THEN** one file contains complete Definition and runtime-gated Runtime Modules, each with Tests; runtime follows definition tests; individual operations use RegistrationBatch::operation and catalogs retain generated registration

#### Scenario: Required Remote Declaration
- **WHEN** a runtime-enabled carrier declares a remote callee without implementing it locally
- **THEN** the declaration compiles without a local Operation or Executable implementation

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
