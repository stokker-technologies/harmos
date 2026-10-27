## ADDED Requirements

### Requirement: Flat Feature-Gated Operation Authoring

An operation SHALL be authored vertically in one flat source file with a single operation struct. The struct-level operation macro SHALL generate portable metadata without serializing implementation state. The runtime feature SHALL gate live resource fields, execution imports, the Operation impl and optional runtime dependencies. The feature-disabled struct SHALL remain a usable portable declaration without an additional authored marker struct or generated contract crate.

#### Scenario: Contract-Only Consumer

- **WHEN** a consumer builds without runtime enabled in its dependency graph
- **THEN** requests, responses, operation definitions and catalogs are available without compiling optional execution dependencies.

#### Scenario: Execution Consumer

- **WHEN** runtime is enabled
- **THEN** the same operation struct contains its resource fields and implements Operation in the same crate, without an orphan-rule workaround.

### Requirement: Explicit Lifecycle And Shared Definition

Authors SHALL explicitly implement Operation. initialize(context) SHALL return Result<Self, Self::Error> and construct local fields. A separate Resources type and OperationImplementation trait SHALL not be mandatory. The macro SHALL obtain payload types from its arguments, produce a shared OperationDefinition value, and verify enabled implementation associated types agree with those arguments.

#### Scenario: Definition Without Implementation

- **WHEN** Read::definition() is used in a contract-only build
- **THEN** it returns an OperationDefinition containing the portable contract without needing the feature-gated Operation impl or live resources.

#### Scenario: Mismatched Payload Types

- **WHEN** an enabled Operation impl specifies a request or response type inconsistent with the macro arguments
- **THEN** compilation reports the mismatch at the authoring boundary.

#### Scenario: Flat Authoring Example

- **WHEN** an author declares the operation below with the agreed product dependencies
- **THEN** one source file and operation struct provide both portable definition access and feature-enabled local execution.

The final operation-file and catalog/registration examples in design.md define the consumer surface. Operation SHALL expose Request, Response, Error and Endpoints; initialize(OperationContext) constructs Self, and run(&mut self, OperationContext, &mut Request, &mut Endpoints) returns Response<ResponseType> through a Send future. Stop and cleanup default to successful completion. Context dependencies are typed, carrier-local Arc values; each invocation owns its instance and retries reuse it.

The macro uses policy = OperationPolicy and optional infer_request. Inference SHALL designate at most one operation per request type; callers SHALL select reused request types explicitly with operation_for::<O>(request). Runtime-gated associated types SHALL agree with portable declaration metadata. Live fields SHALL never enter payload schemas or encoding. Feature unification remains an acknowledged Cargo constraint, not hard crate isolation.
