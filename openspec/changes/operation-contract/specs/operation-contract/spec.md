## ADDED Requirements

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
The workspace SHALL remove RequestFor, infer_request, operation_for, dependency accessors and DependencyError. Stateful carrier
machinery SHALL use the distinct Handler contract. Consumer documentation SHALL show one operation file with complete Definition Module
and Runtime Module divisions, each including its Tests section. The runtime module SHALL follow definition tests and be feature gated.

#### Scenario: Consumer Invocation
- **WHEN** a consumer invokes an operation
- **THEN** it passes the operation struct to runtime.operation and configures retry and lane through the invocation builder
