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

### Requirement: Explicit Three-Level Operation Visibility

Every SDK operation SHALL explicitly choose Private, Protected or Public with no
default. The same levels SHALL apply to host-declared built-ins, following the
decided mapping in design.md, which closes the earlier built-in mapping question.
Faces SHALL NOT replace visibility. Admission SHALL reject unspecified visibility,
Private catalog entries and Private/Protected client-facing gRPC bindings while
retaining requirement, descriptor, fingerprint and digest validation.

#### Scenario: Private Local Execution

- **WHEN** a plugin invokes its own Private operation
- **THEN** its SDK executes the handler inside that plugin process with the ordinary operation policy, retries, deadlines and cancellation
- **AND** the operation is absent from its catalog, invisible to the host, has no gRPC method and sends no operation declaration or invocation on any wire, including the host/plugin carrier

#### Scenario: Private Is Not Remotely Invocable

- **WHEN** the host, another plugin or a client attempts to invoke a plugin's Private operation
- **THEN** no exported operation or gRPC method exists and the handler is not invoked

#### Scenario: Protected Host and Owner Invocation

- **WHEN** the declaring plugin or its host invokes a Protected operation
- **THEN** invocation is permitted under policy; the catalog declares it and the host invokes it only over that plugin's carrier
- **AND** it cannot be associated with a client-facing gRPC method

#### Scenario: Protected Peer and Client Refusal

- **WHEN** another plugin or a client attempts to invoke a Protected operation
- **THEN** the host refuses with a failure naming the action before invoking its provider

#### Scenario: Public Invocation

- **WHEN** another plugin with a matching declared requirement or an authorized client invokes a Public operation
- **THEN** the host checks admission and routes the invocation to the provider; a client-facing gRPC binding is permitted

#### Scenario: Host-Only Plugin Requests

- **WHEN** a host application declares an operation that only it may request of a plugin
- **THEN** it declares Protected, preserving host-to-plugin carrier access while refusing other plugins and clients

#### Scenario: Missing Explicit Visibility

- **WHEN** an SDK declaration omits Private, Protected or Public, or a catalog supplies unspecified visibility or Private
- **THEN** declaration or admission refuses it instead of choosing a default or exporting a local operation

#### Scenario: Built-In Mapping

- **WHEN** built-in declarations migrate to the three levels
- **THEN** they follow design.md's decided mapping, preserve request grants and owned-invocation checks, and do not export local-only Private operations

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

### Requirement: Unary Plugin Attempts

Each plugin-served operation attempt SHALL have one request and one final
response. Duplex control/child frames SHALL NOT turn the operation into a request
or result stream. Samples and Watch SHALL retain their separate purposes; native
host operations MAY retain explicitly declared streaming contracts.

#### Scenario: Unary Result over a Duplex Carrier

- **WHEN** a host invokes a plugin operation while exchanging child calls or cancellation frames
- **THEN** one attempt still consumes one operation request and settles with one final result or structured failure

#### Scenario: Malformed Unary Input

- **WHEN** one invocation supplies malformed payload bytes
- **THEN** it fails without running its handler and unrelated valid invocations remain usable

### Requirement: Catalog Admission Integrity

Before activation, the host SHALL validate catalog digest, message descriptors,
operation fingerprints, requirements and provider identity. Plugin operation IDs
SHALL be unambiguous in the host registry, including collisions with host entries.
Missing providers MAY arrive later, but a matching ID with a mismatched contract
SHALL refuse admission or invocation, never bind by arrival order.

#### Scenario: Tampered Catalog

- **WHEN** a catalog advertises a wrong digest, invalid fingerprint or unresolved message schema
- **THEN** activation refuses before publishing its operations

#### Scenario: Duplicate Provider

- **WHEN** another plugin or host entry attempts to claim an admitted plugin operation ID
- **THEN** registration refuses with the conflicting identity and leaves the admitted provider intact

### Requirement: Authoritative Host Time

Plugin waits and interval publication SHALL use the authoritative host clock.
Virtual time SHALL NOT advance past active unsettled plugin work. Device-driven
samples SHALL remain independent of host ticks and preserve source time separately
from host evidence time.

#### Scenario: Active Invocation

- **WHEN** a plugin invocation has active work and a virtual host timer is ready to advance
- **THEN** advancement waits for work settlement rather than overtaking that invocation

#### Scenario: Interval and Device Publication

- **WHEN** interval publication and device-driven samples are consumed
- **THEN** host ticks select interval timing while device samples require no tick, with source time separate from host timestamps
