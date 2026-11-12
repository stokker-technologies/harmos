# Storage Mutations Delta

## ADDED Requirements

### Requirement: Face-Local Mutation Contracts

The mapped storage contracts SHALL apply independently to `harmos.context.v1.Storage` child
calls and `harmos.v1.Storage` client methods where design.md maps the capability
to both faces. Each face SHALL own its request/response and authorization; shared
handlers SHALL preserve the same storage guarantees. Native-only hooks SHALL stay native.

#### Scenario: Storage Call by Audience

- **WHEN** a client or an authorized plugin uses a capability mapped to both Storage faces
- **THEN** the client calls harmos.v1.Storage and the plugin uses harmos.context.v1.Storage through its context child calls, with separate wrappers and authorization and the same storage guarantees

### Requirement: Whole-Value Revision-Guarded Atomic Set

Get SHALL return the whole writable value and its revision. The matching Set
SHALL send that whole writable value and the expected revision from Get. Set
SHALL validate the complete candidate before atomic publication; stale revisions
SHALL conflict without partial mutation, naming the action “get again and retry”.
Atomic object replacement SHALL retain compare/exchange semantics.

#### Scenario: Whole Replacement

- **WHEN** Set supplies the whole writable value with the revision returned by its matching Get
- **THEN** that whole writable value is replaced atomically and the response returns the result and receipt

#### Scenario: Concurrent Writer

- **WHEN** another writer changes the resource after Get and Set sends the stale revision
- **THEN** Set refuses without partial mutation and its conflict names the action “get again and retry”

#### Scenario: Combined Small Writable Part

- **WHEN** an application replaces a small writable document value through Documents.Set
- **THEN** Documents.Get returns the whole small writable part and revision and Documents.Set replaces that part for only the identified document

#### Scenario: Create If Absent

- **WHEN** a conditional object Set requests create-if-absent and another writer has already created the object
- **THEN** the backend returns the conflict outcome rather than performing an unconditional overwrite

### Requirement: Separately Settable Large Writable Parts

Writable parts that are unreasonably large for whole replacement SHALL be split
into separately settable sub-resources, each with its own Get, Set and revision.
The mapping SHALL identify these nested resource services and their parent
sub-packages under the grammar; it SHALL NOT introduce flat compound services.

#### Scenario: Large Writable Part

- **WHEN** whole replacement of a writable part would be unreasonable
- **THEN** its mapping identifies nested resource services with bare Get/Set, parent sub-packages and their own revisions

### Requirement: Append Ordered Immutable Data

Records, samples, log lines, upload bytes and other ordered immutable data SHALL use Append rather than
Create or Set. Append SHALL assign ordered positions without modifying earlier
entries, accept bounded batches, and preserve original outcomes under mutation
replay. Bulk object-revision ingestion SHALL remain distinct from atomic SetObject.

#### Scenario: Replayed Batch

- **WHEN** the same mutation identity and normalized Append payload are retried after reply loss
- **THEN** the original receipt and assigned positions are replayed without adding duplicate entries

#### Scenario: Changed Replay Payload

- **WHEN** an Append retry reuses a mutation identity with different data
- **THEN** the system reports a conflict without appending that data

#### Scenario: Immutable Prefix

- **WHEN** another bounded sample batch is appended
- **THEN** earlier samples retain their values and positions and the new samples follow them in history order

### Requirement: Distinct Durability Receipts

Mutation receipts SHALL distinguish admission, completion and durability and
identify the semantic mutation. accepted_at SHALL NOT imply durable_at. Durable
positions SHALL be issued only after the backend's durability condition is met.
Callers SHALL be able to obtain the write's durable position for subsequent
position-resolved reads without making all latest reads request persistence.

#### Scenario: Accepted Receipt

- **WHEN** a controlled backend has admitted data but its durable commit is pending
- **THEN** the receipt reports acceptance without falsely populating durable completion

#### Scenario: Commit and Read

- **WHEN** the backend commits the admitted write and its completion/status is observed
- **THEN** the receipt reports its durable position, which can be supplied as at to a retained exact read

#### Scenario: Expired Key

- **WHEN** a newly submitted retry uses an expired mutation identity
- **THEN** it fails explicitly instead of repeating the mutation, while already admitted owners retain their original eventual outcome

### Requirement: Distinct Storage Guarantees

Saved reads SHALL pin durable data; follows that include accepted data SHALL NOT
label it durable. Bounded physical scans, ordered indexes and exact-revision and
frontier projections SHALL preserve their distinct semantics. Bulk ingestion
SHALL NOT substitute for atomic conditional object replacement. Backends SHALL
preserve their declared durability settings and acknowledge persistence only after
the corresponding barrier; changing batching SHALL NOT weaken that guarantee.

#### Scenario: Saved Read and Live Follow

- **WHEN** accepted rows arrive after a retained saved read boundary
- **THEN** the saved read remains at its cut while a follow may report accepted progress without calling it saved

#### Scenario: Index and Projection Continuation

- **WHEN** a bounded scan, ordered index seek or exact-frontier projection continues across writes or restart
- **THEN** it retains its own ordering, selection and revision/frontier guarantees rather than substituting a live bulk read

#### Scenario: Conditional Update and Durable Acknowledgement

- **WHEN** a conditional object replacement has a stale revision or persistence is incomplete
- **THEN** a stale replacement conflicts atomically and incomplete persistence is not acknowledged as durable

### Requirement: Safe Root Opening

Incompatible host definitions or invalid recovery snapshots SHALL refuse root
opening and preserve data. Opening SHALL NOT silently reset, migrate or discard
history. Guidance SHALL require a backup before a deliberate reset or fresh-root
selection under application ownership. Retained plugin-owned history SHALL keep
its stored definitions independently of current plugin presence and shape.

#### Scenario: Incompatible Host Shape

- **WHEN** a stored host record or transaction definition differs from the current compatible inventory
- **THEN** opening refuses with the affected identity and fresh-root guidance, and existing history remains intact

#### Scenario: Deliberate Reset

- **WHEN** an operator chooses to reset an incompatible root
- **THEN** backup and reset are explicit application-owned actions rather than side effects of opening the library

#### Scenario: Corrupt Recovery Snapshot

- **WHEN** recovery detects a malformed or inconsistent snapshot
- **THEN** startup refuses before exposing invalid state and does not clear recovery evidence
