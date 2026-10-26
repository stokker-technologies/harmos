# Storage Transfers

## ADDED Requirements

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

Storage and carrier admission SHALL consume owned payloads without deep copying merely to cross a task or layer boundary. Inspection MAY borrow and retained retries/readers MAY share immutable storage. Memory charges SHALL survive the last retained owner.

#### Scenario: Retained Attempt

- **WHEN** an admitted write is attempted again or its caller is cancelled
- **THEN** the pending owner retains the same immutable payload and required byte charge until terminal settlement

#### Scenario: Shared Read View

- **WHEN** a read result or Arrow slice shares a backing allocation
- **THEN** allocation ownership and its memory charge remain valid until the final view is released

### Requirement: Charged Owned Projection

ReadBatch SHALL support consuming projection of one or two unique owners while transferring all existing memory claims. It SHALL admit caller-bounded additional peak allocation before invoking conversion, account private owner and guard-vector growth, refuse aliases and avoid waiting while retaining the sources. It SHALL NOT automatically shrink independently shared claims or permit payload escape through callback errors or captures.

#### Scenario: Saturated Projection

- **WHEN** a unique batch is projected while its read budget is saturated
- **THEN** an allocation-free move with no owner growth SHALL proceed, while a conversion requiring unavailable additional bytes SHALL fail before invoking its callback

#### Scenario: Projection Lifetime

- **WHEN** a successful projection moves payload allocations into a new retained result
- **THEN** existing and additional charges SHALL remain held through its last alias, and nested retained pages SHALL keep their own guards
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

### Requirement: Composed Read Admission

Composed typed/Arrow queries, object/file reads, mutable metadata claims and retained collections SHALL share an explicit caller admission group without changing cursor or version semantics. Metadata growth SHALL be nonblocking and preserve existing ownership on refusal; Arrow/file DTO projections SHALL retain source claims, and IPC handoff SHALL preserve them until destination admission.

#### Scenario: Retained Results Before Another Read

- **WHEN** a session retains prior results and requests another page or point read
- **THEN** admission includes all retained claims in that group across retries and refuses impossible growth instead of waiting for those same results to be dropped

#### Scenario: Active Reader And Collection Lifetimes

- **WHEN** a grouped reader cannot acquire an active query slot
- **THEN** it fails immediately, and retained collection slots and child pages remain charged until their respective last owners drop