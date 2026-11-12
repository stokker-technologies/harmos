# Storage Mutations Delta

## ADDED Requirements

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

- **WHEN** product rename and archive setters are merged into SetExecution
- **THEN** GetExecution returns the whole small writable part and revision and SetExecution replaces that part for only the identified execution

#### Scenario: Create If Absent

- **WHEN** a conditional object Set requests create-if-absent and another writer has already created the object
- **THEN** the backend returns the conflict outcome rather than performing an unconditional overwrite

### Requirement: Separately Settable Large Writable Parts

Writable parts that are unreasonably large for whole replacement SHALL be split
into separately settable sub-resources, each with its own Get, Set and revision.
The mapping SHALL identify these sub-resources.

#### Scenario: Large Writable Part

- **WHEN** whole replacement of a writable part would be unreasonable
- **THEN** its mapping identifies separately settable sub-resources with matching Get/Set pairs and their own revisions

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
