# Operation Contract Delta

## ADDED Requirements

### Requirement: Self-Sufficient Library SDK Ownership

The library SHALL retain self-sufficient language SDKs, protocol and conformance
without product vocabulary. Product SDKs SHALL build on them. Ownership is locked
on 2026-11-12. Plugin semantic parity and generic managed-client execution language
scope remain open; four-language descriptor-derived naming is separately locked.

#### Scenario: Additional Plugin-Hosting Product

- **WHEN** another product hosts plugins using the library
- **THEN** its product SDKs build on the retained library language SDKs rather than reimplementing the plugin protocol in every language

#### Scenario: Independent Conformance Review

- **WHEN** the library protocol, SDKs and conformance suite are reviewed together
- **THEN** the library can be proven end to end by itself without product vocabulary or a product-owned protocol implementation
- **AND** SDK retention does not imply a settled four-language plugin-parity promise or generic managed-client scope

### Requirement: Predictable Public Method Grammar

Services SHALL name their resource with its plural noun, or singular noun for
one thing such as Plugin. Methods acting on that resource SHALL be bare verbs
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
paged Steps and Parameters. A part with several read/change modes SHALL be a
nested resource service with its own bare verbs, replacing verb-plus-part methods
and flat compound services. Mixed-resource Storage SHALL retain names identifying
independent storage resources, including ListRows/WatchRows and GetObject/SetObject;
its single-mode whole parts SHALL be Info and Capabilities.

#### Scenario: Execution Parts

- **WHEN** execution steps are read as one bounded page and progress is read or watched
- **THEN** the names are Executions.Steps and Executions.Progress.Get/Watch, with no Executions.GetProgress or flat ExecutionProgress service

#### Scenario: Writable Nested Parts

- **WHEN** plugin settings or plot settings have matching Get and Set
- **THEN** their names are Plugins.Settings.Get/Set and Plots.Settings.Get/Set, with no flat compound settings service

### Requirement: Nested Wire Packages and Wrappers

A nested service SHALL use package `<owner>.v1.<lowercase parent>` and the part's
service name, with one proto file per service in a folder per parent. Request and
response wrappers SHALL be `<Service><Method>Request/Response` within that package.
Nested resources MAY have a different library owner from their parent.

#### Scenario: Product Settings Beside Library Plugins

- **WHEN** a product serves plugin settings beside the library's Plugins service
- **THEN** it declares product.v1.plugins.Settings in product/v1/plugins/settings.proto and calls use product.v1.plugins.Settings/Get with SettingsGetRequest/SettingsGetResponse
- **AND** the host exposes both owners' services directly without redeclaring Plugins

### Requirement: Descriptor-Derived Hierarchical Client Roots

Rust, Python, C# and C++ SDK builds SHALL generate the same resource hierarchy
from a common descriptor-derived table, using normal language casing. Packages,
service names, wrappers, cardinality and exposure SHALL determine the root under
design.md. Library/product tables SHALL compose and reject leaf collisions.
No hand-written per-operation naming/forwarding layer SHALL exist. This naming
requirement SHALL NOT settle managed-execution coverage.

#### Scenario: Composed Client Root

- **WHEN** a root is generated from library Plugins and product plugins.Settings/executions.Progress descriptors
- **THEN** calls read client.plugins.settings.get(request), client.executions.progress.watch(request) and client.executions.start(request)
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
declarations and generic host-only HostValues.Get with a registered product
resolver. This SHALL close the per-plugin settings/secrets ownership question.

#### Scenario: Plugin Requests a Value

- **WHEN** a plugin calls the generic host-value operation for a declared setting or secret
- **THEN** the product resolver applies its storage and policy while the library supplies invocation, scope and failure delivery
- **AND** no library settings store, secret store, credential adapter or client settings service is introduced

### Requirement: Manager Ownership and Thin Service Methods

Managers SHALL follow design.md's single live-ownership rule and responsibility
inventory. The product SHALL split its host/shared state into execution, prompt
and report managers, with a capture manager only if recordings remain. Rendering
SHALL stay pure; evidence, catalog, notes, aggregates, plots and documents SHALL
remain operations over storage. Each service method SHALL map to one operation
file calling a manager or storage.

#### Scenario: Service Dispatch Review

- **WHEN** a product service method is reviewed after the split
- **THEN** orchestration resides in the owning manager and plain storage operations call storage through their operation file
- **AND** the execution manager owns queueing, lifecycle, retries, Stop/Cancel and recovery; cycle ownership is chosen at the split using the simplest complete option

#### Scenario: Report Preview

- **WHEN** a report preview is rendered without publication
- **THEN** pure rendering does not allocate a manager or publish content; the report manager owns explicit atomic publication only

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
