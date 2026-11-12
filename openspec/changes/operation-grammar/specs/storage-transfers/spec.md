# Storage Transfers Delta

## ADDED Requirements

### Requirement: Shared Resumable Unary Uploads

Clients and plugins SHALL use the same resumable unary upload-session mechanism
instead of streaming uploads. Session creation, status, bounded chunk admission,
writer resume, finish and abandon SHALL use the file-production lifecycle.
Plugin operations SHALL remain logically unary even when carried by duplex
invocation-control frames.

#### Scenario: Client and Plugin Upload

- **WHEN** a client and an authorized plugin each upload the same kind of content
- **THEN** both use the same session implementation and receipt/integrity rules with bounded unary chunks, not separate client-stream and plugin paths

#### Scenario: No Streaming Fallback

- **WHEN** the selected provider does not support resumable productions
- **THEN** upload fails with explicit capability refusal instead of silently choosing the old streaming route

### Requirement: Resumable Fenced Chunk Admission

Session status SHALL expose accepted and durable progress. Resume SHALL fence the
old writer. A chunk SHALL bind session, writer, offset, data and mutation identity;
identical retries SHALL replay the original outcome, while changed retries,
noncontiguous offsets and stale writers SHALL refuse without corrupting the
retained prefix.

#### Scenario: Lost Reply and Resume

- **WHEN** a writer loses an acknowledged chunk reply, reads session status and resumes under a new writer generation
- **THEN** retained progress is available, the previous writer is fenced and replay cannot duplicate bytes

#### Scenario: Conflicting Chunk

- **WHEN** a chunk reuses the identity of an admitted chunk with different bytes or an invalid offset
- **THEN** the operation refuses and the original retained bytes remain unchanged

### Requirement: Explicit Upload Terminal Outcomes

Finish SHALL validate expected length and integrity before returning a durable
immutable file reference. Abandon SHALL end writing under the production's
partial-data retention guarantee and SHALL remain distinct from DeleteFile.
Observation loss SHALL NOT implicitly finish or delete a session. Admission and
grant checks SHALL apply to each session action.

#### Scenario: Integrity Mismatch

- **WHEN** Finish receives a length or digest that disagrees with uploaded content
- **THEN** it refuses without publishing a successful immutable reference

#### Scenario: Successful Finish

- **WHEN** Finish validates all content and required persistence completes
- **THEN** it returns the final reference and durable receipt with matching length and digest

#### Scenario: Abandon and Disconnect

- **WHEN** an observer disconnects and a later authorized caller abandons the session
- **THEN** disconnect did not publish or delete it, and Abandon ends writing while preserving its declared retained partial content
