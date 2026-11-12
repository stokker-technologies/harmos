# Operation Invocation Delta

## ADDED Requirements

### Requirement: No Automatic Streaming Twins

Projection SHALL NOT generate automatic RunMethod twins. Each wire operation
SHALL have one face-local method signature; context methods SHALL stay
child-carried and native hooks unprojected. Ordinary long work SHALL use unary
actions and Get/Watch. Streams SHALL serve Watch or plugin carriers, except a
promised cooperative final result after cancel SHALL replace the ordinary method
with one managed stream, never a twin.

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

### Requirement: Explicit Plugin Catalog Visibility

Each plugin-served catalog operation carried by Instance.Invoke SHALL explicitly
choose Private (declaring plugin and host) or Public (everyone connected,
including other plugins and host clients). All four SDKs SHALL require the choice
with no default. Admission SHALL reject absent/unspecified visibility and retain
catalog requirements, descriptors, fingerprints and digest checks. Faces SHALL
NOT replace visibility.

#### Scenario: Private Operation Refusal

- **WHEN** a plugin makes the same child call to another plugin's Private operation
- **THEN** the host refuses with an error naming the attempted action and does not invoke the provider
- **AND** the declaring plugin and the host retain access to that Private operation

#### Scenario: Client Cannot Invoke a Private Plugin Operation

- **WHEN** a client submits a plugin's Private operation through Invocations.Invoke
- **THEN** the host refuses with an error naming the action without invoking the instance

#### Scenario: Missing Explicit Visibility

- **WHEN** a declaration in any SDK omits Private or Public, or a catalog supplies unspecified visibility
- **THEN** SDK declaration or catalog admission refuses it instead of choosing a default

### Requirement: Host-Mediated Plugin Calls

A plugin SHALL invoke another plugin's Public operation through a declared
requirement and context child call. The host SHALL use its one registry, check
visibility, declared fingerprint and an admitted provider, then call that provider's
Instance.Invoke. Scope, cancellation and failures SHALL propagate across the hop.
Clients SHALL use harmos.v1.Invocations.Invoke with the same checks. Only the host
SHALL connect to an instance.

#### Scenario: Plugin Invokes Another Plugin's Public Operation

- **WHEN** a plugin makes a child call for its declared requirement and another admitted plugin serves that matching Public operation
- **THEN** the host checks visibility and the declared fingerprint in its one registry, invokes the provider's instance and returns the result through the calling plugin's context

#### Scenario: Client Invokes a Public Plugin Operation

- **WHEN** a client calls the host's generic Invocations.Invoke for an admitted plugin's Public operation with the matching fingerprint
- **THEN** the host checks visibility and provider admission, invokes Instance.Invoke and returns the result without a direct client-to-plugin connection

#### Scenario: Inherited Scope Across Plugins

- **WHEN** a scoped plugin invocation calls another plugin's Public operation through the host
- **THEN** the providing instance receives exactly the caller's inherited scope and scope text does not confer authority

#### Scenario: Cancelling the Caller Cancels the Child

- **WHEN** the calling invocation is cancelled while the other plugin's child invocation is running
- **THEN** the host propagates cancellation to that child and returns its cancellation or declared settlement through the calling context under the operation policy

#### Scenario: Child Failure Across Plugins

- **WHEN** the providing instance returns a structured failure
- **THEN** the host returns its classification, code, action, details and cause to the calling plugin without replacing it with a transport fault

#### Scenario: Requirement or Provider Mismatch

- **WHEN** a child call lacks a declared requirement, has a mismatching fingerprint or has no admitted provider
- **THEN** the host refuses before Instance.Invoke with an error naming the attempted action
