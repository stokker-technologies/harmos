# Operation Contract Delta

## ADDED Requirements

### Requirement: Predictable Public Method Grammar

Public library methods SHALL use VerbResource names and method-specific
MethodRequest and MethodResponse messages. Every reply SHALL use the response
wrapper, including empty results. Ordinary verbs SHALL be limited to Get, List,
Watch, Set, Create, Delete and Append. Singular and plural resources SHALL denote
one resource and a collection respectively.

#### Scenario: Descriptor Review

- **WHEN** a public service descriptor is reviewed after migration
- **THEN** each method has its own named request and response wrapper, including methods returning empty results
- **AND** no ordinary method uses Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan, Inspect or Register

#### Scenario: Derived Resource

- **WHEN** a getter exposes a runtime-owned fact such as current time or projection status
- **THEN** the grammar does not create a setter for that fact merely to make a name pair

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
