## MODIFIED Requirements

### Requirement: Consistent Registration Verbs

The runtime builder and the running runtime SHALL expose register and unregister for supported contributions and route them to the internal owning manager. Managers SHALL own authoritative registry/lifecycle state and SHALL NOT be public registration surfaces. Unregistration SHALL support individual identity-based removal without a retained handle or removal of a whole catalog. This change SHALL not introduce inspect.

#### Scenario: Removal Without a Handle

- **WHEN** an authorized caller unregisters an individual contribution by its qualified identity
- **THEN** the owning manager withdraws and settles that contribution without requiring its original registration handle or removing unrelated catalog members.

#### Scenario: Registration Through the Runtime

- **WHEN** a product registers an operation catalog after start
- **THEN** it calls the runtime's register verb and never obtains the operation manager
