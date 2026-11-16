## MODIFIED Requirements

### Requirement: Service Restart Ownership

ServicePolicy SHALL own stop_timeout and restart independently of request retry policy. Every service execution error SHALL be eligible for restart under RestartPolicy without author classification or recovery predicates. Restart policy SHALL use failure-based terminology, preserve credit/health replenishment, and wait for required finalize. Explicit stop and cancellation SHALL not restart services.

#### Scenario: Service Execution Error

- **WHEN** service execution returns an error
- **THEN** restart eligibility is governed by its restart policy rather than the error variant, with finalize settled before another instance executes.

#### Scenario: Intentional Shutdown

- **WHEN** a service is explicitly stopped or unregistered
- **THEN** stop follows its stop deadline and finalize obligations without triggering restart.
