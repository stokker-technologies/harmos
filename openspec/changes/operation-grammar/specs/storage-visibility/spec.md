# Storage Visibility Delta

## ADDED Requirements

### Requirement: Face-Local Read Contracts

The mapped storage contracts SHALL apply independently to `harmos.context.v1.Storage` child
calls and `harmos.v1.Storage` client methods where design.md maps the capability
to both faces. Each face SHALL own its request/response and authorization; shared
handlers SHALL preserve the same storage guarantees. Native-only hooks SHALL stay native.

#### Scenario: Storage Call by Audience

- **WHEN** a client or an authorized plugin uses a capability mapped to both Storage faces
- **THEN** the client calls harmos.v1.Storage and the plugin uses harmos.context.v1.Storage through its context child calls, with separate wrappers and authorization and the same storage guarantees

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

### Requirement: Accepted Measured Durable Lag

Amended by the owner on 2026-11-13: the measured p95 lag SHALL be accepted:
small about 99 ms; large median 391 ms (178–570); mixed median 346 ms (173–493).
Latest SHALL select saved progress without waiting for newer accepted data. No
accepted-data snapshot SHALL be built. Nightly SHALL keep reporting p50, p95 and
maximum per load as measurements with no failing threshold.

#### Scenario: Measured Lag Review

- **WHEN** Nightly reports lag for small, large and mixed loads
- **THEN** p50, p95 and maximum remain recorded measurements and no lag threshold fails the run
- **AND** a p95 above the former limit does not require an accepted-data snapshot

#### Scenario: Latest During Pending Durability

- **WHEN** a caller omits at while newer accepted data awaits persistence
- **THEN** the read immediately selects latest durable data and reports that selection without exposing accepted data as durable

### Requirement: Reusable Durable-Lag Benchmark

The Nightly benchmark set SHALL include `integration/durable_lag` using storage
benches' stable case naming, with `small_high_rate`, `multi_megabyte` and `mixed`
loads. It SHALL report acceptance-to-durability lag p50, p95 and maximum per load,
including backlog and synchronous commit contribution. Consumers SHALL be able to
reuse the case against their own backend. Measurements SHALL NOT become
wall-clock assertions in tests.

#### Scenario: Nightly Lag Verification

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

Only lists of data rows that can grow without bound SHALL use page_size, max_bytes and page_token; responses SHALL use
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

Watch SHALL use the corresponding read selector and optional at, resolving absent at to
latest durable without waiting and reporting resolved_at. WatchRows SHALL first report
its selected boundary, then exclusive committed events in storage-position order and
progress through filtered data. Initial state SHALL use ListRows. Accepted facts SHALL
remain distinct from durable observation.

#### Scenario: Watch Selection

- **WHEN** a watch opens without at
- **THEN** it resolves to latest durable without waiting and reports resolved_at

#### Scenario: Issued Watch Position

- **WHEN** a watch supplies at
- **THEN** it accepts only a previously returned position or the product terminal boundary, never an arbitrary earlier position

### Requirement: Watch Retention and Resynchronization

The same retention owner SHALL cover opening races and the tail. Retention loss SHALL
emit resync_required and end observation; the caller SHALL list a fresh image and reopen
from its issued cut.

#### Scenario: Lost Watch Retention

- **WHEN** retention covering a watch's opening race or tail is lost
- **THEN** observation emits resync_required and ends
- **AND** the caller lists a fresh image and reopens from its issued cut

### Requirement: External Watch Guarantees

External file/directory watches SHALL preserve their own replacement, truncation,
production and scan guarantees.

#### Scenario: External File or Directory Watch

- **WHEN** an external file/directory watch observes replacement, truncation, production or a scan
- **THEN** its own replacement, truncation, production and scan guarantees remain in force
