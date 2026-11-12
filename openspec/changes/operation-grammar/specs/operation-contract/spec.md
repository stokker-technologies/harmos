# Operation Contract Delta

## ADDED Requirements

### Requirement: Audience-Separated Service Packages

Separation by audience is locked on 2026-11-12. Each service SHALL have exactly
one face: harmos.plugin.v1 (plugin serves, only host calls), harmos.context.v1
(host serves plugin invocation context child calls), or harmos.v1 and nested
packages (host serves clients). Instance SHALL be the protocol service and
Plugins the client management service. Faces SHALL NOT replace catalog visibility.

#### Scenario: Client Cannot Call Context

- **WHEN** a client attempts harmos.context.v1.HostValues.Get or a context Storage operation
- **THEN** the host refuses the call naming the action and exposes no context service on the client endpoint

#### Scenario: Client Cannot Call an Instance

- **WHEN** a client attempts to call harmos.plugin.v1.Instance directly
- **THEN** it is refused naming the action because only the host may open an instance call

#### Scenario: Plugin Cannot Call a Client-Only Operation

- **WHEN** a plugin attempts client-only Plugins.Restart through its context
- **THEN** the host refuses naming Restart and does not route it to the client service

#### Scenario: Host Calls an Instance

- **WHEN** the host connects to a running instance
- **THEN** it calls harmos.plugin.v1.Instance.Get for identity/catalog and opens Invoke, Samples or Stop only as required by their contracts
- **AND** another plugin or client cannot open these calls directly

### Requirement: Separate Declarations for Shared Capabilities

Each capability needed on two faces SHALL have its own declaration, request,
response, fingerprint and authorization per face, even when handlers are shared.
Faces SHALL evolve independently. The exhaustive mapping in design.md SHALL
preserve the existing callable capabilities without exposing context to clients.

#### Scenario: Shared Time and Storage Capability

- **WHEN** a plugin and a client read the host clock or append/read produced data
- **THEN** each uses its own context or client package's declarations and wrappers from design.md's exhaustive mapping
- **AND** handlers may be shared while authorization and future contract changes remain independent

### Requirement: Product Declarations Follow Audience Packages

Product declarations SHALL use `<product>.plugin.v1`, `<product>.context.v1` or
`<product>.v1` by audience. Host-requested plugin lifecycle operations SHALL be Protected
catalog operations carried by Instance.Invoke, never client services. Private
implementation operations SHALL remain local with no face declaration. This library
SHALL declare no product resources or operations. Nested resources SHALL retain
their parent sub-packages and bare verbs within their face.

#### Scenario: Private Implementation Operation

- **WHEN** a plugin declares a Private implementation operation
- **THEN** it is dispatched locally without a face service, wire declaration or catalog entry

#### Scenario: Product Plugin Contract

- **WHEN** a product declares an exported Protected or Public operation its plugin performs
- **THEN** the operation uses its product plugin-face package and catalog visibility, is carried by Instance.Invoke and is not declared as a client service

### Requirement: Generate Only Required Faces

All four SDK build generators SHALL select faces by audience: a plugin SDK SHALL
generate protocol serving and context calling; a client SHALL generate the client
API; a host SHALL generate all three. Imported shared payloads SHALL NOT cause
another face's service stubs or roots to be generated. This SHALL NOT settle the
open semantic plugin-parity or generic managed-client language scope questions.

#### Scenario: Plugin Bindings

- **WHEN** Rust, Python, C# or C++ plugin bindings are generated
- **THEN** they serve Instance and call context methods, with no client API service stubs

#### Scenario: Client Bindings

- **WHEN** client bindings are generated
- **THEN** they contain only client API service stubs and roots, with no context callers or Instance server

#### Scenario: Host Bindings

- **WHEN** host bindings are generated
- **THEN** they contain all three faces, with each service registered only for its declared audience

### Requirement: Self-Sufficient Library SDK Ownership

The library SHALL retain self-sufficient language SDKs, protocol and conformance
without product vocabulary. Product SDKs SHALL build on them. Ownership is locked
on 2026-11-12. Plugin semantic parity and generic managed-client invocation language
scope remain open; four-language descriptor-derived naming is separately locked.

#### Scenario: Additional Plugin-Hosting Product

- **WHEN** another product hosts plugins using the library
- **THEN** its product SDKs build on the retained library language SDKs rather than reimplementing the plugin protocol in every language

#### Scenario: Independent Conformance Review

- **WHEN** the library protocol, SDKs and conformance checks are reviewed together
- **THEN** the library can be proven end to end by itself without product vocabulary or a product-owned protocol implementation
- **AND** SDK retention does not imply a settled four-language plugin-parity promise or generic managed-client scope

### Requirement: Predictable Public Method Grammar

Services SHALL name their resource with its plural noun, or singular noun for
one thing such as Instance. Methods acting on that resource SHALL be bare verbs
and SHALL NOT repeat its noun. Ordinary verbs SHALL be limited to Get, List,
Watch, Set, Create, Delete and Append. Every method SHALL have service-qualified
request and response wrappers under design.md's grammar, including empty replies.

#### Scenario: Descriptor Review

- **WHEN** a public service descriptor is reviewed after migration
- **THEN** each method has its own named request and response wrapper, including methods returning empty results
- **AND** no ordinary method uses Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan, Inspect or Register

#### Scenario: Derived Resource

- **WHEN** a getter exposes a runtime-owned fact such as current time or projection status
- **THEN** the grammar does not create a setter for that fact merely to make a name pair

### Requirement: Nested Resources and Single-Mode Parts

A part read in exactly one way SHALL use its noun alone on its parent, including
paged Jobs.Steps. A part with several read/change modes SHALL be a
nested resource service with its own bare verbs, replacing verb-plus-part methods
and flat compound services. Mixed-resource Storage SHALL retain names identifying
independent storage resources, including ListRows/WatchRows and GetObject/SetObject;
its single-mode whole parts SHALL be Info and Capabilities.

#### Scenario: Job Parts

- **WHEN** job steps are read as one bounded page and progress is read or watched
- **THEN** the names are Jobs.Steps and Jobs.Progress.Get/Watch, with no Jobs.GetProgress or flat JobProgress service

#### Scenario: Writable Nested Parts

- **WHEN** application-owned plugin settings or document settings have matching Get and Set
- **THEN** their names are Plugins.Settings.Get/Set and Documents.Settings.Get/Set, with no flat compound settings service

### Requirement: Nested Wire Packages and Wrappers

A nested service SHALL use package `<face package>.<lowercase parent>` (with face
package `<owner>.plugin.v1`, `<owner>.context.v1` or `<owner>.v1`) and the part's
service name, with one proto file per service in a folder per parent. Request and
response wrappers SHALL be `<Service><Method>Request/Response` within that package.
Nested resources MAY have a different library owner from their parent.

#### Scenario: Product Settings Beside Library Plugins

- **WHEN** a product serves plugin settings beside the library's Plugins service
- **THEN** it declares application.v1.plugins.Settings in application/v1/plugins/settings.proto and calls use application.v1.plugins.Settings/Get with SettingsGetRequest/SettingsGetResponse
- **AND** the host exposes both owners' services directly without redeclaring Plugins

### Requirement: Descriptor-Derived Hierarchical Client Roots

Rust, Python, C# and C++ SDK builds SHALL generate the same resource hierarchy
from a common descriptor-derived table, using normal language casing. Face packages,
service names, wrappers, cardinality and exposure SHALL determine the root under
design.md. Library/product tables SHALL compose and reject leaf collisions.
No hand-written per-operation naming/forwarding layer SHALL exist. This naming
requirement SHALL NOT settle managed-invocation coverage.

#### Scenario: Composed Client Root

- **WHEN** a root is generated from library Plugins and application plugins.Settings/jobs.Progress descriptors
- **THEN** calls read client.plugins.settings.get(request), client.jobs.progress.watch(request) and client.jobs.start(request)
- **AND** the plugins node also exposes library plugin entry methods

### Requirement: Explicit Domain Action Meanings

Each domain verb SHALL have the single meaning in design.md's shared vocabulary and library action definitions
and SHALL be used only when an ordinary verb would hide a side effect. Sleep
SHALL retain the authoritative host-clock wait. Application lifecycle verbs SHALL
NOT introduce generic library RPCs without a library-owned resource contract.

#### Scenario: Clock Wait

- **WHEN** a caller invokes Sleep with a protobuf Duration under a virtual host clock
- **THEN** completion follows that clock's advancement and no independent SDK wall-clock wait is introduced

#### Scenario: Side-Effect Review

- **WHEN** a method is proposed as AdvanceProjection or CaptureFile
- **THEN** review verifies materialization or independent content capture rather than treating the action as a writable status field

### Requirement: One Failure and Retry Vocabulary

Host and plugin contracts SHALL share one structured Failure and retry-policy
vocabulary. Failure propagation SHALL preserve classification, stable code,
action, details and causes with separate diagnostics. Policy SHALL express retry
count, backoff, jitter and Duration budgets consistently; only explicitly
replay-safe operations SHALL retry transient failures.

#### Scenario: Child Failure

- **WHEN** a child operation returns a structured application failure
- **THEN** its parent preserves every failure field and cause, and the boundary maps canonical gRPC status/details without replacing the application cause with a transport fault

#### Scenario: Retry Override Isolation

- **WHEN** one replay-safe call overrides the total-attempt budget and the next omits an override
- **THEN** only the first call uses the override, one total attempt means no retry, and both retain callee-owned deadlines

#### Scenario: Unsafe Retry

- **WHEN** a caller requests retry for an operation with unknown replay safety
- **THEN** admission refuses before invoking the operation

### Requirement: Opaque Scope and Canonical Identity

Operation IDs SHALL derive from the declared namespace and method without an
independent alias. Scope SHALL remain opaque caller-bound context inherited by
children; it SHALL NOT confer authority. Renamed contracts SHALL carry regenerated
fingerprints and requirements before admission.

#### Scenario: Old Fingerprint

- **WHEN** a caller supplies the old fingerprint for a renamed operation
- **THEN** admission refuses the mismatch instead of routing through a hidden compatibility alias

#### Scenario: Scope Inheritance

- **WHEN** an authorized parent calls a child using an inherited scope
- **THEN** the library preserves it unchanged and does not derive grants from its text

### Requirement: Client-Owned Resending After Failed Calls

When a call fails on the client, resending SHALL be the client's responsibility.
The same request identifier and normalized payload SHALL return the original
receipt; a changed payload under that identifier SHALL conflict. This does not
change replay-safety admission, retention or expired-identifier refusal.

#### Scenario: Lost Call Result

- **WHEN** the client fails to receive a mutation result and resends the same request identifier and normalized payload
- **THEN** the original receipt is returned without repeating the mutation

#### Scenario: Changed Payload Under an Identifier

- **WHEN** the client resends the same request identifier with a changed payload
- **THEN** the call conflicts without replacing the original outcome

### Requirement: Generic Machinery and Product Composition

The library SHALL own generic machinery through design.md's manager/service
inventory: plugins, access, transfers, host status, operations/invocations, time,
storage, streams, protocol and SDKs. A product SHALL serve these services directly
by composition, without redeclarations or wrappers, and SHALL NOT rebuild the
runtime using only the protocol crate.

#### Scenario: Product Endpoint

- **WHEN** a client manages a plugin entry, listener or transfer, or reads host status on a product endpoint
- **THEN** it calls the library-owned services on that endpoint and the library manager performs the generic work
- **AND** Plugins covers managed/remote entry Create/Get/List/Watch/Set/Restart/Delete, verified bundles, persistence, state and failures

#### Scenario: Duplicate Machinery Removal

- **WHEN** a product adopts the composed runtime
- **THEN** its duplicate connection, dispatch, stream, launch, entry, listener/pairing, upload and generic status implementations are removed with their consumers
- **AND** the earlier duplication caused by protocol-only hosting is not retained as an adapter

### Requirement: Product Definitions, Policy and Plugin Values

A product SHALL own storage definitions, schemas, indexes, projections, latest
selection, its objects and operations, and authorization policy. It SHALL retain
settings/secrets stores, schemas, UI declarations, archives and operating-system
credential integration. The library SHALL provide only a plugin catalog's settings
declarations and generic context-only harmos.context.v1.HostValues.Get with a registered product
resolver. This SHALL close the per-plugin settings/secrets ownership question.

#### Scenario: Plugin Requests a Value

- **WHEN** a plugin calls the generic host-value operation for a declared setting or secret
- **THEN** the product resolver applies its storage and policy while the library supplies invocation, scope and failure delivery
- **AND** no library settings store, secret store, credential adapter or client settings service is introduced

### Requirement: Manager Ownership and Thin Service Methods

Managers SHALL follow design.md's single live-ownership rule and library
responsibility inventory. Each service method SHALL map to one operation file
calling a manager or storage.

#### Scenario: Service Dispatch Review

- **WHEN** a library service method is reviewed
- **THEN** orchestration resides in the owning manager and plain storage operations call storage through their operation file

### Requirement: Library Access and Host Status Owners

GrpcManager SHALL own Access/Access.Listener for listener Get/Set, pairing creation
and certificate regeneration/key rotation. ServiceManager SHALL own Status,
Status.Logs and Status.TraceEvents for host identity, health, availability, bounded
logs/traces and a diagnostics archive. A narrow product-fact registration SHALL
contribute product version/identity and component health/availability while the
library retains aggregation and lifecycle. Product-only reads SHALL remain in
product services.

#### Scenario: Product Health Contribution

- **WHEN** a product registers a component health provider and version facts
- **THEN** the library's Status includes those contributions under the service manager's lifetime and failure handling without a second product status service

#### Scenario: Access Rotation

- **WHEN** authorized access operations replace listener settings or regenerate a certificate
- **THEN** GrpcManager owns the live listener/certificate lifecycle and validation rather than a product copy

### Requirement: Library Dependency Direction

Production dependencies SHALL remain acyclic: runtime composes operations,
storage, domain and protocol; operations and storage depend on domain/protocol;
domain depends on protocol; the standalone Rust SDK depends on protocol/derive.
Protocol, domain, storage, operations and language SDKs SHALL NOT depend on the
host runtime or consuming products. Runtime SHALL own sandbox authority.
Build, derive and development dependencies SHALL NOT be mistaken for ownership
of a production runtime resource.

#### Scenario: Standalone Contract Consumer

- **WHEN** a plugin or generated-contract client uses the standalone SDK
- **THEN** its production dependency graph contains no host runtime or consuming product

#### Scenario: Forbidden Dependency Edge

- **WHEN** a lower library crate adds an upward runtime or consuming-product production dependency
- **THEN** dependency validation rejects that edge rather than allowing a second host authority

### Requirement: Policy Ownership and Honest Budgets

Operation deadlines, delays and retries SHALL have one owner in operation policy
or owning subsystem configuration; a caller override SHALL affect one invocation
only. Consumers SHALL own storage budgets, admission, concurrency and response
batch sizes; the library SHALL own protocol constants and correctness constraints.
Budgets, limits, deadlines and required checks SHALL NOT be weakened to pass proof.

#### Scenario: Call-Local Override

- **WHEN** a caller overrides retry policy for one admitted operation
- **THEN** later invocations retain the callee policy and replay-safety admission remains enforced

#### Scenario: Backend Policy Migration

- **WHEN** backend I/O, retry or session lifetimes currently use private constants
- **THEN** implementation moves operational policy to its owning policy/configuration rather than claiming those constants are consumer configuration

#### Scenario: Consumer Resource Selection

- **WHEN** a consumer selects storage response sizing, concurrency or admission limits
- **THEN** construction validates those settings without inventing product policy or overriding correctness constraints

#### Scenario: Failed Budget Proof

- **WHEN** behavior misses a configured deadline, budget or limit
- **THEN** proof reports the failure and repairs the implementation without enlarging the value to obtain a pass

### Requirement: Deterministic Behavioral Proof

Correctness tests SHALL be deterministic, story-shaped at integration level and
above, and based on observable behavior. Deadline/delay stories SHALL use a
controlled clock or event; real elapsed time, machine load and sampled memory
SHALL be benchmark measurements, never pass/fail assertions. Authored prose,
tautologies and a single repository constant SHALL NOT be asserted in tests.
A hang guard MAY stop stuck work without making elapsed time a behavioral claim.

#### Scenario: Deadline Story

- **WHEN** a story proves cancellation or deadline expiry
- **THEN** it controls clock advancement and handler events and checks settlement independently of machine speed

#### Scenario: Performance Evidence

- **WHEN** a workload measures lag, throughput or memory
- **THEN** it records benchmark evidence rather than asserting sampled resource use in a correctness test

#### Scenario: Contract Review

- **WHEN** a test is proposed to protect an invariant
- **THEN** it exercises behavior and cannot pass merely by quoting maintained prose or comparing a repository constant with itself

### Requirement: Consistent Generic SDK Vocabulary

Library protocol, SDKs, conformance and architecture contracts SHALL use generic
vocabulary without product objects or private identities. Rust, Python, C# and
C++ SDKs SHALL remain library-owned and name the same concepts consistently in
normal language casing. Product SDKs SHALL compose those library SDKs. This
requirement SHALL NOT imply that unresolved semantic plugin parity or generic
managed-client language coverage is already implemented or decided.

#### Scenario: Independent Host

- **WHEN** an independent host consumes any library SDK and generated resource roots
- **THEN** it receives the same library concepts without requiring another product's model or runtime
