# operation-deadlines Specification

## Purpose
TBD - created by archiving change idle-stream-deadlines. Update Purpose after archive.

## Requirements

### Requirement: Stream Progress Rearms The Deadline
Streaming operations SHALL interpret policy timeout as maximum silence and unary operations SHALL retain total deadlines.

#### Scenario: Continuous Progress
- **WHEN** stream items move more frequently than timeout for a total duration greater than timeout
- **THEN** the operation succeeds

#### Scenario: Stalled Stream
- **WHEN** no item moves for longer than timeout, including while waiting for completion
- **THEN** the operation fails with DeadlineExceeded naming its identity over gRPC

#### Scenario: Unary Total Deadline
- **WHEN** a unary operation takes longer than timeout
- **THEN** it expires despite any unrelated activity
