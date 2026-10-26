## ADDED Requirements

### Requirement: Consistent Registration Verbs

Runtime and owning managers SHALL expose register and unregister for supported contributions. Managers SHALL own authoritative registry/lifecycle state. Unregistration SHALL support individual identity-based removal without a retained handle or removal of a whole catalog. This change SHALL not introduce inspect.

#### Scenario: Removal Without A Handle

- **WHEN** an authorized caller unregisters an individual contribution by its qualified identity
- **THEN** the owning manager withdraws and settles that contribution without requiring its original registration handle or removing unrelated catalog members.

### Requirement: Qualified Contribution Identity

The host SHALL prefix plugin and guest contributions once with their owner identity. Cross-plugin references SHALL use the qualified identity, while reload generations SHALL remain separate from the stable public ID.

#### Scenario: Plugin Operation

- **WHEN** plugin bench registers its local operation run_bench
- **THEN** other plugins address bench.run_bench and transport forwarding does not prefix the ID again.

### Requirement: Typed Invocation Lanes

The application runtime SHALL own a closed lane enum and lane capacities. Invocation builders SHALL accept typed lane overrides, validate them before admission and preserve the selected lane throughout execution. Portable contracts SHALL not depend on consumer runtime enum types.

#### Scenario: Lane Override

- **WHEN** an invocation selects an admitted host lane using lane(Lane::Interactive)
- **THEN** that invocation uses the selected lane without changing the operation structural fingerprint or other invocations' defaults.

### Requirement: Service Restart Ownership

ServicePolicy SHALL own stop_timeout and restart independently of request retry policy. Every service execution error SHALL be eligible for restart under RestartPolicy without author classification or recovery predicates. Restart policy SHALL use failure-based terminology, preserve credit/health replenishment, and wait for required cleanup. Explicit shutdown and cancellation SHALL not restart services.

#### Scenario: Service Execution Error

- **WHEN** service execution returns an error
- **THEN** restart eligibility is governed by its restart policy rather than the error variant, with cleanup settled before another instance executes.

#### Scenario: Intentional Shutdown

- **WHEN** a service is explicitly stopped or unregistered
- **THEN** shutdown follows its stop deadline and cleanup obligations without triggering restart.

### Requirement: Dynamic Plugin Contributions

Plugin scopes SHALL support dynamic service and operation registration through the same owning managers as runtime startup. Each contribution SHALL retain plugin ownership and replacement-generation identity for safe withdrawal and subsequent explicit registration. Automatic reload, readiness handover and rollback are deferred; this delivery SHALL retain the provider/code owner until active work and cleanup settle.

#### Scenario: Independent Plugin Contribution

- **WHEN** a running plugin registers a service contribution
- **THEN** the contribution can be individually withdrawn and is also associated with that plugin for coordinated unload.

### Requirement: Catalog Batch Registration

Enum catalogs SHALL organize large operation inventories and register their members as a batch through register. Entries SHALL remain individually addressable and removable. Batch validation SHALL precede publication; an invalid member SHALL not leave a partially registered batch.

#### Scenario: Individual Removal From A Catalog

- **WHEN** a catalog registers Read and Calibrate and an authorized caller unregisters Read by qualified identity
- **THEN** Calibrate remains registered and no catalog handle is required.

#### Scenario: Invalid Batch

- **WHEN** a catalog contains a conflicting or invalid registration
- **THEN** registration fails without publishing a subset of its contributions.
