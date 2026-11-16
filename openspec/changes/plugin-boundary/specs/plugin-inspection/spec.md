## Purpose

Allow products to inspect frozen plugin metadata through the same bounded process and transport ownership used by runtime loading.

## ADDED Requirements

### Requirement: Metadata-Only Inspection

Harmos SHALL provide bounded inspection of frozen plugin metadata without initialize, sampling, operational handler execution, installation or registry writes. Inspection SHALL perform generic descriptor validation and return opaque product sections for product interpretation.

#### Scenario: Inspect a Candidate

- **WHEN** a product inspects a managed candidate for its bundle metadata
- **THEN** it receives generic-validated frozen declarations and no plugin resource acquisition hook or sample demand runs

### Requirement: Inspection Settles Its Resources

Inspection SHALL use the runtime's process and transport owners with operation-owned deadlines, metadata/message bounds and cancellation. Every exit path SHALL settle owned resources under existing teardown policies. Failures SHALL identify the action and corrective step.

#### Scenario: Cancellation Before Metadata Arrives

- **WHEN** an inspection is cancelled after it spawns a child but before metadata arrives
- **THEN** its retained process owner signals, kills if needed and reaps that child, and no installed state is created

#### Scenario: Remote Inspection

- **WHEN** inspection of a product-selected remote endpoint completes or fails
- **THEN** Harmos detaches its inspection connection and does not terminate the remote process

#### Scenario: Metadata Exceeds Admission Bounds

- **WHEN** a candidate reports metadata beyond the configured inspection bound
- **THEN** inspection refuses with an action to supply metadata within the bound and settles its owned resources
