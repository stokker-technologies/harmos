# storage-transfers Specification

## Purpose
Defines independent typed persistence, the shipped JSON backend, owned bounded payload handoffs and projection, ordered file progress, comparable performance workloads and bounded query admission.

## Requirements

### Requirement: Independent Typed Persistence

Backends SHALL receive owned homogeneous schema, object, record, stream and file-reference batches. Each call SHALL complete durably and idempotently at its assigned positions; the manager SHALL preserve contiguous ordering without promising atomicity across different typed calls.

#### Scenario: Partial Mixed Progress

- **WHEN** a schema write succeeds and its following object write fails
- **THEN** the schema may remain durable, the cursor cannot claim the missing object, and retrying the admitted operation does not duplicate the schema

### Requirement: Shipped JSON Backend

Harmos SHALL expose a filesystem JSON storage backend using explicit settings, bounded disk reads and raw file payloads, reusable by consumers, persistence tests and benchmarks. It SHALL preserve historical reads and required restart, retention, session and mutation semantics without adding manager scheduling policy.

#### Scenario: Reopen Persistent Data

- **WHEN** the JSON backend is closed and reopened with the same storage history
- **THEN** acknowledged durable typed entries and completed files remain readable at the correct cursor and interrupted writes do not appear as completed data

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

#### Scenario: Shutdown And Failure

- **WHEN** an action fails or shutdown begins with file actions dispatched
- **THEN** dispatched actions settle before backend teardown and required pending data is neither silently discarded nor duplicated

### Requirement: Comparable Performance Workloads

Benchmarks SHALL separately identify storage, real sidecar, real guest and external gRPC paths with explicit settings, deterministic inputs and identical before/after measurement boundaries.

#### Scenario: Storage Scale

- **WHEN** the storage suite runs
- **THEN** it measures 30 MiB file upload and download, and symmetric writes and reads for 10,000 records and 1,000,000 samples, with bounded generation/consumption and verified results
- **AND** writes include durable completion while read timing excludes setup writes and includes consuming the complete saved dataset

#### Scenario: Carrier Boundaries

- **WHEN** carrier benchmarks report command or transfer latency
- **THEN** the selected real process, component or network boundary is exercised and startup is distinguished from steady-state work

#### Scenario: Symmetric Carrier Datasets

- **WHEN** the sidecar or guest transfer suite runs
- **THEN** it transfers 10,000 records, 30 MiB of file bytes and 1,000,000 samples in both directions through the real carrier
- **AND** it compares unary batches with streams using identical batch sizes and verifies ordering, counts and contents outside timing
- **AND** the three datasets run concurrently on a reused host/plugin with isolated transfer IDs, using benchmark-feature endpoints in the shipped Schematic examples

### Requirement: Bounded Query Admission

Typed/Arrow queries, object/file reads and retained collections SHALL compose through StorageManager with ordinary shared ownership, explicit cursors and unchanged version semantics. Query concurrency and serialized queue/page limits SHALL remain consumer-configured; consumers SHALL bound their retained results and conversion scratch.

#### Scenario: Retained Results Before Another Read

- **WHEN** a consumer retains prior results and requests another page or point read
- **THEN** prior data SHALL remain valid independently of the next read without a shared read-pool reservation

#### Scenario: Active Reader And Collection Lifetimes

- **WHEN** a reader waits for an active query slot
- **THEN** the configured pending-query limit SHALL apply, cancellation SHALL release its slot, and previously returned pages SHALL remain valid through their last owners
