# Storage Visibility Delta

## ADDED Requirements

### Requirement: One Position-Resolved Read Path

Positioned data reads SHALL share one method per kind of data, accepting optional
at and returning resolved_at. Absent at SHALL resolve once to latest durable data
without waiting for accepted pending writes or building an accepted-data snapshot.
A cut SHALL retain all required positions and revisions rather than silently
reducing a composite selection to one sequence number.

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

### Requirement: Snapshot Then Durable Watch

Watch SHALL use the corresponding read selector and optional at, open with its
resolved boundary/snapshot, then emit changes strictly after it. Saved-data
watches SHALL distinguish durable progress from accepted data and include progress
past filtered-out positions. A gap or expired retained start SHALL require explicit
resynchronization rather than silent loss.

#### Scenario: Writes During Opening

- **WHEN** data becomes durable while the watch's initial snapshot is being delivered
- **THEN** the caller receives the snapshot boundary and subsequent changes once in position order without an opening gap

#### Scenario: Explicit Start

- **WHEN** a watch starts at a retained cut returned by a previous reply
- **THEN** it opens with that boundary/snapshot and follows changes strictly after it instead of remaining frozen

#### Scenario: Pending Data and Filter Progress

- **WHEN** pending data is not durable and later committed positions are filtered out
- **THEN** pending data is not labeled durable and the watch still reports the examined durable progress

#### Scenario: Retention Gap

- **WHEN** a consumer resumes from an issued position whose necessary changes have been reclaimed
- **THEN** the watch returns explicit resynchronization information instead of skipping to the current frontier
