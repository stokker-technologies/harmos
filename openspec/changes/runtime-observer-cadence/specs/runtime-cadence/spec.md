## ADDED Requirements

### Requirement: Producing Observers Supply Runtime Cadence

Macro observers SHALL require interval_ms of at least 1 ms as their declared initial
cadence. Observers MAY override that cadence through a synchronous interval method
using execution resources at activation; otherwise the declared interval SHALL apply.
Streams SHALL remain cadence-less.
Static observer inventory SHALL contain no sampling cadence.

#### Scenario: Declared Cadence Controls Sampling

- **WHEN** an observer without an interval method activates
- **THEN** initialization retries and sampling use its declared interval_ms

#### Scenario: Invalid Declaration Is Refused

- **WHEN** interval_ms is absent or less than 1 ms, including with an override
- **THEN** compilation refuses the observer

#### Scenario: Configured Cadence Controls Sampling

- **WHEN** an observer activates with a resource-supplied interval
- **THEN** initialization retries and sampling use that interval
- **AND** a controlled clock can advance samples at that cadence

#### Scenario: Invalid Cadence Is Refused

- **WHEN** an observer returns zero duration or cadence resolution fails
- **THEN** activation reports failure without running a timer

### Requirement: SDK Consumers Migrate Together

Native and sidecar observers, macros, examples and templates SHALL use the runtime
cadence API with required defaults and optional overrides, without a compatibility shim.

#### Scenario: Artifact Inventory Is Inspected

- **WHEN** a consumer inspects a new observer artifact
- **THEN** schema, identity, outputs and activation are statically available
- **AND** cadence is chosen only by the producing runtime
