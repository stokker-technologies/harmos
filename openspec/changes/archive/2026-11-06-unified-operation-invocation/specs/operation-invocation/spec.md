## ADDED Requirements

### Requirement: One Authorized Invocation Surface

Every runtime, handler, template and observer SHALL expose `operation(request)` returning the SDK `Invocation<H: OperationHost, O, E = ()>`. Await SHALL return `Response<O::Response>`; start SHALL return the host instance. Retry and endpoints SHALL be per-call options. Only runtime hosts SHALL expose lane through an extension trait. The no-argument handler call metadata accessor SHALL remain.

#### Scenario: Authorized Handler and Template
- **WHEN** a handler declares Invokes or a template declares Called for an operation
- **THEN** its operation method creates the same SDK builder and awaits a response retaining its payload owner.

#### Scenario: Undeclared Operation
- **WHEN** a handler or template requests an undeclared operation other than an existing built-in exemption
- **THEN** compilation fails at the operation method authorization bound.

### Requirement: Clean API Retirement

Request-taking call, builder run, template host, Invocation id and the routing enum SHALL be removed without aliases. SDK Host and Dokime StepHost SHALL become OperationHost and StepOperationHost.

#### Scenario: Migrated Consumer
- **WHEN** a consumer uses the unified invocation
- **THEN** no superseded builder or trait alias remains.

### Requirement: Host-Owned Execution

OperationHost implementations SHALL preserve routing, stage/harness selection, admission, cancellation and child-call metadata. Handler adapters SHALL capture the current attempt's authority and metadata. Errors SHALL use H::Error and convert at surface Result boundaries.

#### Scenario: Child Cancellation and Metadata
- **WHEN** a handler starts a child with portable call values and is cancelled
- **THEN** the child sees the calling attempt's metadata and settles through existing scoped cancellation and admission.

#### Scenario: Existing Built-In Access
- **WHEN** a template invokes an existing exempt built-in such as WatchFile
- **THEN** operation routes it without a requires entry and preserves stage ownership.

### Requirement: Declared Defaults and Safe Overrides

The operation macro SHALL own default policy once. A call with no builder options SHALL use that declaration unchanged. A retry override SHALL affect only its call, cross sidecar and guest wire boundaries, never raise attempt deadlines, and refuse enabling retries with unknown replay safety. Every actual attempt SHALL remain visible in evidence.

#### Scenario: Default and Override Isolation
- **WHEN** a call overrides retry and a later call omits it
- **THEN** only the first uses the override, and both retain declared attempt deadlines.

#### Scenario: Unsafe Override
- **WHEN** a caller enables retries for an operation with unknown replay safety
- **THEN** admission refuses before the action executes; macro-declared retry semantics remain unchanged.

#### Scenario: Carrier Retry Evidence
- **WHEN** a replay-safe sidecar or guest child retries a transient failure
- **THEN** the host applies the transported policy and records every attempt with its original child metadata.
