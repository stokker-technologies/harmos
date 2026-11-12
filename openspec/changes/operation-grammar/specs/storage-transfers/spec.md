# Storage Transfers Delta

## ADDED Requirements

### Requirement: Shared Resumable Unary Transfers

Clients and plugins SHALL share the library StorageManager's unary resumable
sessions and ranged content reads through Storage. Uploads SHALL Create, Append
ordered bytes at offsets, Get status, Publish or Delete to abandon. Configuration
SHALL bound chunks/ranges. Products SHALL compose Storage directly and retain
object metadata, removing duplicate upload engines. Plugin operations SHALL
remain logically unary within their carriers.

#### Scenario: Client and Plugin Upload

- **WHEN** a client and an authorized plugin upload content
- **THEN** both call the library Storage session methods on the host endpoint, with the same bounded unary chunk, receipt and integrity rules
- **AND** no second Content/Uploads service or generic transfer implementation is introduced

#### Scenario: Unsupported Provider

- **WHEN** a selected provider does not support resumable sessions
- **THEN** upload fails explicitly instead of selecting a streaming fallback

### Requirement: Resumable Fenced Chunk Admission

Session status SHALL expose accepted and durable progress. Resume SHALL fence the
old writer. A chunk SHALL bind session, writer, offset, data and mutation identity.
Identical bytes replayed at an accepted offset SHALL succeed; different bytes at
that offset SHALL conflict even if a new request identity is supplied.
Noncontiguous new offsets and stale writers SHALL refuse without corrupting the
retained prefix.

#### Scenario: Lost Reply and Resume

- **WHEN** a writer loses a chunk reply, reads status and resumes under a new writer generation
- **THEN** retained progress permits continuation, the previous writer is fenced and identical replay cannot duplicate bytes

#### Scenario: Conflicting Offset

- **WHEN** a chunk supplies different bytes at an already accepted offset
- **THEN** it conflicts regardless of request identity and the original retained bytes remain unchanged

### Requirement: Explicit Upload Terminal Outcomes

PublishFileProduction SHALL validate expected length and digest before making
content visible and returning a durable immutable reference. DeleteFileProduction
SHALL abandon the unpublished session without publishing it. DeleteFile SHALL
remain the separate action for published content; retained-reader reclamation is
an open question to settle at its implementation task. Observation loss SHALL NOT
publish or delete a session. Grant checks SHALL apply to every session action.

#### Scenario: Integrity Mismatch

- **WHEN** PublishFileProduction receives a length or digest inconsistent with uploaded content
- **THEN** it refuses without exposing content or returning a successful immutable reference

#### Scenario: Successful Publication

- **WHEN** publication validates content and required persistence completes
- **THEN** content becomes visible and its reference and durable receipt match its length and digest

#### Scenario: Abandon After Disconnect

- **WHEN** an observer disconnects and a later authorized caller deletes the unpublished session
- **THEN** disconnect did not publish or delete it and deletion abandons it without publication

### Requirement: Bounded Resumable Unary Downloads

GetFileBytes SHALL accept offset, maximum bytes and optional at, returning a
bounded byte range, total content length, content digest and resolved_at. Clients
and plugins SHALL repeat that same unary Get to continue or resume. There SHALL
be no streaming download operation or automatically generated twin.

#### Scenario: Interrupted Download

- **WHEN** a caller resumes with an offset into the same retained content selection
- **THEN** Get returns the next bounded range and the same total length and digest

#### Scenario: Configured Range Bound

- **WHEN** a caller requests a range larger than the configured maximum
- **THEN** the operation enforces that maximum and never returns an unbounded payload

### Requirement: Nightly Transfer Throughput Evidence

Before streaming transfers are deleted, the Nightly benchmark set SHALL measure
large-file upload and download throughput with `integration/file_transfer` for
client and plugin carriers, recording bytes, chunk/range configuration and
throughput. This SHALL be a benchmark measurement, never a test assertion.

#### Scenario: Transfer Removal Review

- **WHEN** superseded streaming transfers are ready for deletion
- **THEN** review includes measured large-file throughput from the Nightly benchmark case rather than a timing assertion in a test
