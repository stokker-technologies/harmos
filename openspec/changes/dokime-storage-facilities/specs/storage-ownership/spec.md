## ADDED Requirements

### Requirement: Compose Retained File Operations

Native file and production following SHALL compose with retained metadata and projections through StorageManager and ordinary shared batch ownership. Serialized queue/page bounds and query concurrency SHALL remain explicit consumer settings; consumer retained-result and scratch budgets SHALL remain consumer-owned.

#### Scenario: Retained Metadata And Data

- **WHEN** a consumer retains metadata and follows the related file through the same storage manager
- **THEN** following SHALL preserve ordinary source and target ownership without introducing a read-memory pool
- **AND** source allocations SHALL survive through final consumption

### Requirement: Preserve Opened Source And Producer Identity

Native capture SHALL support the original opened source and a retained producer when the consumer requires those identities.

#### Scenario: Retry After Partial Capture

- **WHEN** capture partially acknowledges data and a caller retries
- **THEN** the same source/target identities and acknowledged prefix SHALL remain authoritative
- **AND** differing overlap SHALL be rejected without appending a conflicting suffix

### Requirement: Retain Backend And Request Owners

Backend work and accepted request payloads SHALL retain their actual data owners through blocking work and caller cancellation. Storage SHALL NOT expose the superseded read-session or backend scratch-admission carriers.

#### Scenario: Caller Cancels After Acceptance

- **WHEN** accepted backend or append work outlives its initiating caller
- **THEN** its retained payload and output owners SHALL remain live until the actual data is released
- **AND** queue/page-limit refusal SHALL preserve the durable prefix
