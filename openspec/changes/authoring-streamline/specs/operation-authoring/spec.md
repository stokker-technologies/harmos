## ADDED Requirements

### Requirement: Model-Owned Operation Policy

Operation policy values and their pure behavior SHALL have one definition owner under harmos-domain/src/model/operations/. Operation declarations SHALL remain under operations/. Moving definitions SHALL preserve required policy behavior; request policy SHALL exclude resident restart behavior, whose final ownership follows the service design.

#### Scenario: Policy Ownership

- **WHEN** an author or maintainer locates OperationPolicy and its request retry, backoff and jitter values
- **THEN** their definitions and pure validation are owned by model/operations/policy.rs, with no duplicate implementation under operations/common/.

### Requirement: Curated Policy Imports

Harmos SHALL explicitly export OperationPolicy, RetryPolicy, RestartPolicy, Backoff and Jitter at its facade root and curated prelude. Existing qualified domain and runtime operation paths SHALL continue to refer to the same types.

#### Scenario: Root Imports

- **WHEN** a consumer imports the five policy authoring types from harmos
- **THEN** those imports resolve without requiring the operations namespace.

#### Scenario: Qualified Type Identity

- **WHEN** a policy is constructed through an existing qualified operation path and used through a root or prelude import
- **THEN** it is the identical domain-owned type and requires no conversion.

### Requirement: Cohesive Operation Authoring

Operation SHALL name the local executable implementation with separate portable request and response types. Its struct macro SHALL support the flat feature-gated layout specified in executable-operation. Payloads SHALL retain ordinary derives, standalone message authoring and explicit custom-codec support.

#### Scenario: Local Implementation State

- **WHEN** an operation implementation retains a device or client handle
- **THEN** that live state is not treated as a portable request field merely because the type declares an operation.

#### Scenario: Custom Message Implementations

- **WHEN** a supported payload uses custom portable-message implementations
- **THEN** operation authoring does not emit conflicting message implementations.

### Requirement: Actionable Authoring Diagnostics

Authoring diagnostics SHALL identify the offending payload or field and explain the required authoring action rather than relying only on an internal Described trait-bound failure.

#### Scenario: Invalid Payload

- **WHEN** a declaration includes an unsupported request field or a response or stream item lacking the required portable-message contract
- **THEN** diagnostics identify the affected authoring input and explain what must be supplied or corrected.

### Requirement: Automatic Contract Identity

Operation contracts SHALL use deterministic fingerprints instead of manually maintained versions. Fingerprints SHALL cover stable operation identity, encoding identity and complete request, response and stream schemas; invocation retry overrides SHALL not change them.

#### Scenario: Payload Schema Change

- **WHEN** a request, response or stream schema changes, including a nested schema
- **THEN** the contract fingerprint reflects that structural change without a manually edited version.

### Requirement: Invocation Builder

The execution surface SHALL provide operation(request).retry(policy).run() and start(). Preparation SHALL be inert; terminal methods SHALL consume the builder and validate and snapshot per-invocation policy at admission. Handler behavior SHALL use an explicit Operation implementation independently of portable payload declarations.

#### Scenario: Run And Start

- **WHEN** a caller awaits run
- **THEN** it receives the completed response through the same execution path used by start and joining its returned handle.

#### Scenario: Isolated Retry Override

- **WHEN** one invocation overrides retry policy and awaits start
- **THEN** admission returns a running handle without mutating other calls or the request schema, and preserves cancellation, status, replay safety and response ownership guarantees.



