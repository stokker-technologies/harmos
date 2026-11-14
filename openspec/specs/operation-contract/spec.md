# Operation Contract Specification

## Purpose

Define shared stream access, runtime-computed operation IDs and host owner admission.

## Requirements

### Requirement: Shared Stream Access
Context SHALL expose the derived stream endpoint set through a shared reference. Unary operations SHALL use `()`. Stream input receives
SHALL serialize without changing cancellation behavior.

#### Scenario: Duplex Operation
- **WHEN** an operation declares input and output items
- **THEN** its derived endpoints are a pair of typed input and output endpoints

### Requirement: Runtime-Computed Operation IDs
Registration SHALL bind an existing executable operation type under a runtime-computed local ID through the ordinary batch mechanism. Schema, policy, resources and implementation SHALL come from the type. The selected ID SHALL participate in the fingerprint. No arbitrary runtime schemas or handlers SHALL be accepted.

#### Scenario: Named Typed Operation
- **WHEN** an application or embedded plugin registers a typed operation under a configured ID
- **THEN** catalog metadata, fingerprint, typed dispatch and encoded dispatch agree on that ID and retain the type's contract

#### Scenario: Conflicting Registration
- **WHEN** a batch repeats an ID or collides with an existing catalog entry from the same owner
- **THEN** registration refuses atomically and leaves the original binding intact

### Requirement: Ordinary Host Plugin Owner
The plugin owner `host` SHALL have ordinary admission and grant checks. Its name SHALL neither be reserved nor confer application caller authority. Runtime authority SHALL come from the caller context; actual qualified identity collisions SHALL still be refused.

#### Scenario: Host Owner Admission
- **WHEN** a plugin named `host` registers and invokes a declared shared application operation
- **THEN** it receives the same admission, duplicate and scope-grant checks as any other plugin owner

#### Scenario: Owner Name Confers No Authority
- **WHEN** a plugin named `host` declares a private or Host-only operation from another provider
- **THEN** activation refuses it without granting authority from the name
