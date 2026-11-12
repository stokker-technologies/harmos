# Storage Mutations Delta

## ADDED Requirements

### Requirement: Masked Atomic Set

Set SHALL accept an optional field mask; absence SHALL replace the whole writable
value. Revisioned resources SHALL enforce an expected revision. The complete
candidate SHALL be validated before atomic publication. Invalid masks, read-only
fields and revision conflicts SHALL refuse without partial mutation. Atomic
object replacement SHALL retain compare/exchange semantics.

#### Scenario: Absent Mask

- **WHEN** Set supplies a complete writable value with a matching expected revision and no mask
- **THEN** the whole writable value is replaced atomically and the response returns the result and receipt

#### Scenario: Selected Fields

- **WHEN** Set supplies a valid mask selecting one writable field
- **THEN** only that field changes and unselected writable fields retain their prior values

#### Scenario: Conflict or Invalid Mask

- **WHEN** expected revision is stale or the mask names an unknown or read-only field
- **THEN** Set refuses without any visible partial mutation

#### Scenario: Create If Absent

- **WHEN** a conditional object Set requests create-if-absent and another writer has already created the object
- **THEN** the backend returns the conflict outcome rather than performing an unconditional overwrite

### Requirement: Append Ordered Immutable Data

Records, samples and other ordered immutable data SHALL use Append rather than
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
