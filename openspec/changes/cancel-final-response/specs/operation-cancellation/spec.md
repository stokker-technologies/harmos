## ADDED Requirements

### Requirement: Typed Cancellation Completion
Started operation handles SHALL expose consuming asynchronous cancel returning the same typed response as join. Signal-only controls SHALL use request_cancel. Authored operation contexts SHALL expose a portable cancellation signal.

#### Scenario: Cooperative Summary
- **WHEN** a started operation observes cancellation and produces its normal response within policy deadlines
- **THEN** cancel resolves to that response after child settlement and cleanup

#### Scenario: Cancellation Deadline
- **WHEN** no response is produced within the cancellation timeout bounded by remaining attempt and overall deadlines
- **THEN** execution aborts and cancel returns typed Cancelled without retry

#### Scenario: Existing Operation
- **WHEN** an existing zero-grace operation ignores cancellation
- **THEN** it aborts as before and returns typed Cancelled

### Requirement: Carrier Parity
In-process, sidecar, guest and managed gRPC execution SHALL carry cooperative cancellation without abandoning the response channel. Ordinary transport abandonment SHALL retain abort semantics. Exported protobuf descriptors SHALL expose typed managed calls usable by external TypeScript and generated Python clients.

#### Scenario: Remote Cancellation
- **WHEN** a managed remote caller sends cancellation after the opening request
- **THEN** it receives the final typed response or typed Cancelled on the original response channel

### Requirement: Portable Integer Requests
Portable message integer fields SHALL support u32, u64, i32 and i64. u16 SHALL remain unsupported; byte buffers SHALL use Vec<u8>.

#### Scenario: Port Field
- **WHEN** a caller declares a portable port field
- **THEN** u32 works across carriers and u16 is rejected by message derivation
