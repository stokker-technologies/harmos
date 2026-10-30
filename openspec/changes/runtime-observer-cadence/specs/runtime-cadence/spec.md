## ADDED Requirements

### Requirement: Producing Observers Supply Runtime Cadence

Observers SHALL resolve a positive interval from execution resources at activation.
Static observer inventory SHALL contain no sampling cadence.

#### Scenario: Configured Cadence Controls Sampling

- **WHEN** an observer activates with a resource-supplied interval
- **THEN** initialization retries and sampling use that interval
- **AND** a controlled clock can advance samples at that cadence

#### Scenario: Invalid Cadence Is Refused

- **WHEN** an observer returns zero duration or cadence resolution fails
- **THEN** activation reports failure without running a timer

### Requirement: SDK Consumers Migrate Together

Native and sidecar observers, macros, examples and templates SHALL use the runtime
cadence API with no compatibility path for static interval metadata.

#### Scenario: Artifact Inventory Is Inspected

- **WHEN** a consumer inspects a new observer artifact
- **THEN** schema, identity, outputs and activation are statically available
- **AND** cadence is chosen only by the producing runtime
