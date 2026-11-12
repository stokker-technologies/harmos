# Storage Visibility Delta

## ADDED Requirements

### Requirement: One Position-Resolved Read Path

Positioned reads SHALL share one method per data kind, with optional at and
resolved_at. Absent at SHALL resolve once to latest durable without waiting;
the durable-lag requirement applies. Composite cuts SHALL retain all required
positions/revisions. Products SHALL own object-specific latest/terminal selection,
schemas, indexes and projections; the library SHALL own shared storage mechanisms.

#### Scenario: Accepted Data Ahead of Saved Data

- **WHEN** a controlled backend has accepted a write but has not completed its durable commit and a latest read opens
- **THEN** the read resolves to the existing saved position without requesting a flush or waiting for that write
- **AND** the response reports that resolved_at and excludes the pending data

#### Scenario: Durable Receipt Selection

- **WHEN** a caller reads with at taken from its completed write's durable receipt
- **THEN** the retained exact selection includes that write and reports its resolved cut without advancing silently

#### Scenario: Composite Selection

- **WHEN** a selection depends on several histories or resource revisions
- **THEN** the response identifies all components needed to repeat that selection atomically

### Requirement: Payload-Size-Independent Durable Lag

The newest durable position SHALL trail the newest accepted data by no more than
250 ms at the 95th percentile, independent of payload size. If the bound is missed,
smaller flush pieces and size-triggered flushing SHALL be tried first; accepted-
data snapshot reads SHALL be the named fallback if insufficient, not built here.

#### Scenario: Lag Bound Review

- **WHEN** evidence for the latest-durable decision is reviewed
- **THEN** the 100 ms cadence alone is insufficient proof: flush duration adds to it and grows with written data, with synchronous batches targeting 1 MB and capped at 16 MB
- **AND** because nothing measures durable lag today, the three-load benchmark must demonstrate each load's p95 meets the owner's bound

#### Scenario: Bound Missed

- **WHEN** any load's measured p95 exceeds the owner's stated bound
- **THEN** review requires smaller flush pieces and flushing as soon as a size threshold is reached, alongside the cadence, to be tried first and all three loads remeasured
- **AND** if those are insufficient, reads resolving against a snapshot of accepted data remain the named fallback, without claiming it is built or the latest-durable decision is proven

### Requirement: Reusable Durable-Lag Benchmark

The Nightly benchmark set SHALL include `integration/durable_lag` using storage
benches' stable case naming, with `small_high_rate`, `multi_megabyte` and `mixed`
loads. It SHALL report acceptance-to-durability lag p50, p95 and maximum per load,
including backlog and synchronous commit contribution. Consumers SHALL be able to
reuse the case against their own backend. Measurements SHALL NOT become
wall-clock assertions in tests.

#### Scenario: Nightly Lag Evidence

- **WHEN** the Nightly benchmark set runs `integration/durable_lag` against a backend
- **THEN** its stable load cases cover small samples at a high rate, multi-megabyte batches and a mix, reporting p50, p95 and maximum durable lag for each
- **AND** review compares each load's p95 with the owner's stated bound independently of payload size, rather than accepting a pooled percentile

#### Scenario: Consumer Backend Measurement

- **WHEN** a consumer supplies its own backend to the reusable durable-lag case
- **THEN** the same three loads and lag statistics are available without product vocabulary or wall-clock test assertions

### Requirement: Byte-Bounded Flush Mechanism

Flushes SHALL be bounded in bytes, and large input SHALL be split so no single
commit exceeds the configured size. Deterministic tests SHALL prove this
mechanism using committed byte counts rather than wall-clock assertions.

#### Scenario: Large Input Splitting

- **WHEN** a deterministic controlled backend records commits for input larger than the configured flush size
- **THEN** the input is split across byte-bounded flushes and no single commit exceeds that size
- **AND** the test checks committed byte counts rather than elapsed time

### Requirement: Issued Positions Only

Public at SHALL accept only positions or cuts previously returned for consistent
continuation, including the product's terminal selection. Unissued arbitrary
earlier positions, foreign histories, expired selections and unavailable cuts
SHALL fail explicitly. Internal provider recovery and authorization lookups SHALL
retain their distinct contracts.

#### Scenario: Arbitrary Earlier Sequence

- **WHEN** a caller constructs an earlier sequence that no prior reply issued for the selection
- **THEN** the read refuses it rather than offering arbitrary historical reads

#### Scenario: Lost Retention

- **WHEN** a previously returned selection has expired or its retained revision is unavailable
- **THEN** the read reports the explicit expiry or availability failure without substituting latest

#### Scenario: Foreign History

- **WHEN** at identifies a different store identity or epoch
- **THEN** the system rejects it without comparing sequence numbers across histories

### Requirement: One Bounded Paging Shape

Lists SHALL use page_size, max_bytes and page_token; responses SHALL use
next_page_token and resolved_at. Opaque tokens SHALL bind selection, stable
ordering, resolved cut and schema/projection revision. Continuation SHALL be
exclusive and enforce both row and byte limits. Physical work limits SHALL remain
separate from response limits.

#### Scenario: Mutation Between Pages

- **WHEN** new durable rows arrive after a list's first page
- **THEN** continuation retains the first resolved selection, stable order and tie-breaker without skipping or duplicating its rows

#### Scenario: Changed Selector

- **WHEN** a page token is reused with a different filter, order or revision
- **THEN** continuation fails explicitly rather than silently starting another selection

#### Scenario: Filtered Page

- **WHEN** bounded physical work finds no matching rows but more source remains
- **THEN** an empty bounded page can carry a continuation and does not falsely declare end-of-source

### Requirement: Position-Resolved Watch Boundary

Watch SHALL use the corresponding read selector and optional at, resolving absent
at to latest durable without waiting and reporting resolved_at. Exact opening,
event/progress, gap and resynchronization semantics remain open for the Watch
contract task; the candidate details in design.md do not close that question.

#### Scenario: Watch Selection

- **WHEN** a watch opens without at
- **THEN** it resolves to latest durable without waiting and reports resolved_at

#### Scenario: Issued Watch Position

- **WHEN** a watch supplies at
- **THEN** it accepts only a previously returned position or the product terminal boundary, never an arbitrary earlier position
