# Operation Contract Delta

## ADDED Requirements

### Requirement: Self-Sufficient Library SDK Ownership

The library SHALL retain its language SDKs, protocol and conformance suite as a
self-sufficient boundary without product vocabulary. Product SDKs SHALL keep
building on the library SDKs, since other products will also host plugins on the
library. Ownership is decided by the owner on 2026-11-12. Only plugin semantic
parity across Rust, Python, C# and C++, and whether the generic managed client
exists in all four languages or in Rust only, remain open under SDKs.

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

### Requirement: Part Names and Mixed-Resource Services

A part read in exactly one mode SHALL use its noun alone, whether streamed,
paged or whole. Only parts with multiple read or change modes SHALL use verb
plus part; prefer a small resource service where clearer, retaining exact
Get/Set pairs. Mixed-resource services SHALL retain method resource nouns
without repeating their own service name and SHALL NOT be split merely to
shorten names.

#### Scenario: Resource and Single-Mode Parts

- **WHEN** the execution resource is described by Executions
- **THEN** its actions are Start, Cancel and Get, its streamed progress is Progress, and its paged steps and parameters are Steps and Parameters

#### Scenario: Multiple Modes and Mixed Resources

- **WHEN** a part has List and Watch modes, or Get and Set modes
- **THEN** its methods retain verb plus part or use bare verbs in its own resource service
- **AND** Storage retains ListRows/WatchRows and GetObject/SetObject while its single-mode whole parts are Info and Capabilities

### Requirement: Proto-Derived Short Client Names

Generated clients SHALL derive short names directly from proto without a
handwritten naming layer.

#### Scenario: Generated Client Names

- **WHEN** clients are generated from the execution descriptor
- **THEN** they expose executions.start(request), executions.cancel(request), executions.get(request) and executions.progress(request) directly from proto

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
