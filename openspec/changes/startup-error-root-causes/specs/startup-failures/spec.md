## ADDED Requirements

### Requirement: Classified Startup Diagnostics

Startup errors SHALL distinguish root failures from dependent missing resources within each startup/prepare pass, render readable root messages, and retain all structured failure details.

#### Scenario: Dependent Resources
- **WHEN** one prepare pass contains a construction failure and missing-resource errors
- **THEN** the report contains one root and each missing-resource item as a consequence regardless of reporting order

#### Scenario: Independent Failures
- **WHEN** unrelated resources fail
- **THEN** each independent failure remains a root

#### Scenario: Refusal Transport
- **WHEN** a root carries an application refusal code, details and diagnostics
- **THEN** startup transport retains their content without Rust Debug in user-facing prose

#### Scenario: Missing Resources Alone
- **WHEN** a pass has no preparation/construction failure
- **THEN** its missing-resource errors remain roots
