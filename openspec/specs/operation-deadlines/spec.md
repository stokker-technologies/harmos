# Operation Deadlines Specification

## Purpose

Bound stalled streaming work without imposing a total transfer duration, while
preserving total deadlines for unary operations and bounded response draining.

## Requirements

### Requirement: Stream Progress Rearms the Deadline
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

#### Scenario: Buffered Output Draining
- **WHEN** a handler has settled but buffered gRPC output stops moving beyond policy timeout
- **THEN** the listener ends the stream with DeadlineExceeded naming the operation
