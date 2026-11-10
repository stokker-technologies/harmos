## ADDED Requirements

### Requirement: Values Instead of Attribute Macros
An application SHALL declare operations, catalogs, lanes, listeners, streams, records and
transactions as ordinary values, with derives used only on data types whose fields must be read.

#### Scenario: Declaring an Operation
- **WHEN** an author derives the message trait on a request type and builds an operation value
  from an identifier and a function
- **THEN** the operation is served with the same contract, schema and fingerprint as before

#### Scenario: Mistake Found at Start
- **WHEN** two operations share an identifier, a lane set has an invalid capacity or a listener
  names a callback that does not exist
- **THEN** the application refuses to start with a message naming the item

#### Scenario: Undeclared Call
- **WHEN** an operation calls an operation it did not declare
- **THEN** the call is refused with a message naming both operations

### Requirement: State Guarantees Survive
State management SHALL keep stable identities, schema-derived revisions, restoration, replay and
undo when transactions are declared as values.

#### Scenario: Replay
- **WHEN** a journal written before the change is replayed through the new state catalog
- **THEN** the recovered state equals the state recovered before the change

#### Scenario: Undo
- **WHEN** a reversible transaction is applied and undone
- **THEN** the state equals the state before it was applied

#### Scenario: Barrier
- **WHEN** a transaction is declared without an inverse
- **THEN** undo stops at it, as declared
