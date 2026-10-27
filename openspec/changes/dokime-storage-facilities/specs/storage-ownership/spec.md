## ADDED Requirements

### Requirement: Compose Retained File Operations In One Session

Native file and production following SHALL support shared admission with dependent metadata and projections through the existing ReadSession ownership model.

#### Scenario: Dependent Read At Capacity

- **WHEN** a consumer retains metadata and follows the related file in the same session
- **THEN** new allocation SHALL be admitted or explicitly refused without waiting on an unrelated group that depends on release of that metadata
- **AND** source claims SHALL survive through final consumption

### Requirement: Preserve Opened Source And Producer Identity

Native capture SHALL support the original opened source and a retained producer when the consumer requires those identities.

#### Scenario: Retry After Partial Capture

- **WHEN** capture partially acknowledges data and a caller retries
- **THEN** the same source/target identities and acknowledged prefix SHALL remain authoritative
- **AND** differing overlap SHALL be rejected without appending a conflicting suffix

### Requirement: Admit Backend And Request Memory Before Allocation

Backend scratch and accepted request payloads SHALL carry native ownership through their actual lifetime, including blocking work and caller cancellation.

#### Scenario: Caller Cancels After Acceptance

- **WHEN** accepted backend or append work outlives its initiating caller
- **THEN** its retained payload and scratch claims SHALL remain live until the actual data is released
- **AND** refusal SHALL occur before additional allocation without changing the durable prefix
