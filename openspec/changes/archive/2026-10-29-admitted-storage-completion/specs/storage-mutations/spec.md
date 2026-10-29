# Storage Mutations

## ADDED Requirements

### Requirement: Admitted Completion Ownership

A storage call admitted under a valid mutation identity SHALL retain access to its eventual outcome independently of expiry of the identity's replay window. Completion state SHALL remain bounded by owned admission/storage capacity and SHALL be reclaimed after its owners release it.

#### Scenario: Backend Completes After Retry Expiry

- **WHEN** a valid file creation is admitted and its backend completes after the retry ledger deadline
- **THEN** the original waiting call receives the result of that one creation without reissuing the mutation

#### Scenario: Attached Duplicate And Cancelled Caller

- **WHEN** a duplicate attaches before expiry and one waiting caller is cancelled
- **THEN** the remaining admitted caller receives the same operation outcome and completion ownership is reclaimed without an unbounded retained entry

### Requirement: Expired Retry Refusal

Newly submitted or retried expired identities SHALL remain rejected. This repair SHALL NOT extend receipt retention, producer leases or production deadlines, nor change fingerprint validation or persistence failure visibility.

#### Scenario: Late Retry After Admitted Completion

- **WHEN** a new call submits the expired identity of an operation whose original caller completed
- **THEN** the retry remains refused and no duplicate backend mutation is admitted
