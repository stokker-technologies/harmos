# storage-transfers Specification

## Purpose

Define independent typed persistence, owned bounded payload handoffs and projection, ordered file progress and bounded query admission.

## Requirements

### Requirement: Independent Typed Persistence

Backends SHALL receive owned homogeneous schema, object, record, stream and file-reference batches. Each call SHALL complete durably and idempotently at its assigned positions; the manager SHALL preserve contiguous ordering without promising atomicity across different typed calls.

#### Scenario: Partial Mixed Progress

- **WHEN** a schema write succeeds and its following object write fails
- **THEN** the schema may remain durable, the cursor cannot claim the missing object, and retrying the admitted operation does not duplicate the schema

### Requirement: Owned Bounded Payload Handoffs

Storage and carrier admission SHALL consume owned payloads without deep copying merely to cross a task or layer boundary. Inspection MAY borrow and retained retries/readers MAY share immutable storage. Storage allocations SHALL remain valid through their last retained owner; storage SHALL NOT recreate the superseded allocation-claim mechanism.

#### Scenario: Retained Attempt

- **WHEN** an admitted write is attempted again or its caller is cancelled
- **THEN** the pending owner retains the same immutable payload until terminal settlement

#### Scenario: Shared Read View

- **WHEN** a read result or Arrow slice shares a backing allocation
- **THEN** allocation ownership remains valid until the final view is released

### Requirement: Owned Projection

ReadBatch SHALL support consuming projection of one or two unique owners and refuse retained aliases before invoking conversion. Consumers SHALL own their output and scratch allocation budgets.

#### Scenario: Aliased Projection

- **WHEN** a typed or file batch has a retained alias and a caller requests consuming projection
- **THEN** the move SHALL fail before invoking its conversion callback and preserve retained data

#### Scenario: Projection Lifetime

- **WHEN** a successful projection moves payload allocations into a new retained result
- **THEN** its ordinary shared owner SHALL keep those allocations valid through its last alias

### Requirement: Ordered Independent File Progress

Storage SHALL support explicitly bounded independent file I/O while preserving per-file ordering, publication cursors and durability barriers.

#### Scenario: Distinct Files

- **WHEN** two independent files have admitted chunks and the configured limit permits both
- **THEN** their backend writes may progress concurrently while a subsequent chunk of either file waits for that file's preceding completion

#### Scenario: Shutdown and Failure

- **WHEN** an action fails or shutdown begins with file actions dispatched
- **THEN** dispatched actions settle before backend teardown and required pending data is neither silently discarded nor duplicated

### Requirement: Bounded Query Admission

Typed/Arrow queries, object/file reads and retained collections SHALL compose through StorageManager with ordinary shared ownership, explicit cursors and unchanged version semantics. Query concurrency and serialized queue/page limits SHALL remain consumer-configured; consumers SHALL bound their retained results and conversion scratch.

#### Scenario: Retained Results Before Another Read

- **WHEN** a consumer retains prior results and requests another page or point read
- **THEN** prior data SHALL remain valid independently of the next read without a shared read-pool reservation

#### Scenario: Active Reader and Collection Lifetimes

- **WHEN** a reader waits for an active query slot
- **THEN** the configured pending-query limit SHALL apply, cancellation SHALL release its slot, and previously returned pages SHALL remain valid through their last owners
