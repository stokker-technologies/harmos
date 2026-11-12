# Operation Invocation Delta

## ADDED Requirements

### Requirement: No Automatic Streaming Twins

Projection SHALL NOT automatically generate RunMethod twins. Each operation SHALL
have one canonical gRPC method. Ordinary long work SHALL use unary resource actions
and Get/Watch observation. Server streaming SHALL be reserved for Watch and the
plugin protocol's own carriers, except the explicit managed replacement below.
A managed stream SHALL replace the ordinary gRPC method only
when the declaration explicitly promises a cooperative final result after cancel.

#### Scenario: Pure Getter Projection

- **WHEN** a unary getter is projected
- **THEN** its descriptor contains that canonical method and no automatic Run-prefixed gRPC method or managed envelopes

#### Scenario: Explicit Cooperative Result

- **WHEN** a method explicitly promises a typed final summary after cancellation
- **THEN** it has one managed gRPC method instead of an ordinary twin, and cancellation can settle with that summary within the declared grace

#### Scenario: Observation Loss

- **WHEN** a caller loses Get/Watch observation of admitted resource-owned work
- **THEN** the work follows its resource lifecycle rather than acquiring an implicit observer-owned lifetime

### Requirement: Distinct Stop and Cancel Actions

Cancel SHALL interrupt running work through its cooperative cancellation path.
For the product control contract, queued work SHALL refuse Cancel with a failure
naming the attempted action. Stop SHALL finish at a safe boundary and SHALL NOT
be translated to Cancel. The library SHALL preserve the product's refusal rather
than inventing a queued scheduling transition.

#### Scenario: Queued Control Refusal

- **WHEN** a product control operation refuses Cancel because the target is queued
- **THEN** the library returns the structured refusal naming Cancel and does not reinterpret it as Stop, deletion or success

#### Scenario: Running Work

- **WHEN** running work receives a declared cooperative cancellation request
- **THEN** its handler and children observe cancellation and settle under the operation policy
- **AND** a separate Stop request remains a safe-boundary action

#### Scenario: Native Transport Abandonment

- **WHEN** the caller aborts the gRPC transport instead of sending the cooperative control
- **THEN** no final typed result is promised on that abandoned channel

### Requirement: Single-Resource Rename Scope

A metadata Set used to rename a resource SHALL affect only that identified
resource. The library SHALL preserve caller-selected identity and scope and SHALL
NOT expand the mutation to sibling attempts or other resources sharing a parent.
The product owns that rename implementation; no new library Rename RPC is required.

#### Scenario: Shared Parent

- **WHEN** the product renames one identified resource while a sibling attempt shares its parent
- **THEN** only the selected resource's name and revision change and the sibling retains its prior name
