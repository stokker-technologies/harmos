# filesystem-operations Specification

## Purpose

Define composed filesystem resource policy, retained ownership and measurement.

## Requirements

### Requirement: Composed Resource Policy and Measurement
The system SHALL compose filesystem and capture/replay/state resource settings under their existing owners, retain ordinary shared data ownership without restoring the superseded storage read pool, and measure applicable hash/capture/read/follow/directory workloads after the combined implementation. New workloads SHALL be identified as having no historical baseline.

#### Scenario: Retained Buffers
- **WHEN** a consumer retains a file slice or typed-record page after the producer advances or closes
- **THEN** its backing allocation SHALL remain valid until the final owning alias is released
