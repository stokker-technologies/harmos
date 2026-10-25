## ADDED Requirements

### Requirement: Operation And Storage Coverage

Harmos SHALL emit structured, runtime-filterable spans at all operation execution and storage interface boundaries without a new public profiling API or feature. Instrumentation SHALL preserve existing results, cancellation, resource accounting and durability semantics and exclude payloads and secrets.

#### Scenario: Operation Reaches Storage

- **WHEN** a caller submits an operation that queues a state commit and awaits storage
- **THEN** the trace distinguishes admission, queue residence, execution, preparation, publication and durability waits with existing operation and storage identities

#### Scenario: Reader Continues After Creation

- **WHEN** a storage query, file reader or follower delivers subsequent batches
- **THEN** each batch boundary remains observable under its request context without per-element instrumentation

#### Scenario: Instrumentation Is Filtered Out

- **WHEN** the application disables the instrumentation target or level
- **THEN** Harmos preserves behavior without collecting payloads or eagerly formatting diagnostic fields

### Requirement: Causal Context

Harmos SHALL preserve caller context through asynchronous handoffs and represent fan-in as causal links rather than attributing shared work to an arbitrary caller. Trace context SHALL remain separate from idempotency and durable domain identity.

#### Scenario: Shared Persistence Batch

- **WHEN** one backend write or flush serves multiple admitted requests
- **THEN** each request has a causal contribution span under its own subscriber, sharing batch identity with the single physical-work span and exposing receipt/frontier fields for durability joins

#### Scenario: Contributors Use Distinct Subscribers

- **WHEN** a shared batch receives requests captured by different scoped subscribers
- **THEN** queue and contribution spans close under their owning subscribers, no foreign span IDs become causal links, and batch identity supports correlation across the retained traces

#### Scenario: Cancellation Settles

- **WHEN** an operation fails or is cancelled while work is settling
- **THEN** the trace closes request and physical-work lifetimes at their actual boundaries and records a structured outcome

### Requirement: Native Performance Artifacts

System tests SHALL retain Perfetto-readable span lifetimes, fields, parent relationships and causal flows with explicitly flushed normal completion. Benchmarks SHALL retain Criterion-native reports and baseline comparison without duplicate custom CSV/Markdown summary generation. Native test verdicts SHALL remain available.

#### Scenario: Agent Reads A Completed Trace

- **WHEN** a system story completes with queued and shared storage work
- **THEN** the retained trace exposes the original relationships for structured analysis and distinguishes elapsed waits from backend execution

#### Scenario: Benchmark Results Are Published

- **WHEN** a benchmark leaf completes
- **THEN** native Criterion results and readable reports are retained with the applicable baseline comparison and missing-baseline or regression findings preserved
