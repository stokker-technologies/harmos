# Tasks

## 1. Contract

- [ ] 1.1 Lock the source inventory per macro: uses in Harmos source, examples and tests, and in Dokime, with the typed projections each use depends on.

## 2. Simple Declarations

- [ ] 2.1 Delete the unused `domain_` and `runtime_` variants.
- [ ] 2.2 Replace `listener`, `streams` and `lanes` with values checked when built.
- [ ] 2.3 Replace `record` with a typed entry and split `stream` into a row derive and a stream value.

## 3. Operations and Messages

- [ ] 3.1 Replace `operation` with a request derive and an operation value, moving the declared-call and declared-resource checks to run time.
- [ ] 3.2 Replace the typed `catalog` enums with catalog values, giving storage and projections checked handles.
- [ ] 3.3 Replace `records` with typed entries in an owned registry.
- [ ] 3.4 Turn `message` into a derive.

## 4. State

- [ ] 4.1 Add the state catalog and typed transaction entries with explicit reversible and barrier declarations, and run the existing replay, recovery and inverse-law tests against it unchanged.
- [ ] 4.2 Replace `transaction` and `transactions`, then delete their generators.

## 5. Final Validation

- [ ] 5.1 Run the operation, state, storage and example tests on Linux and Windows.
- [ ] 5.2 Update the product guide and authoring guidance.

## Workflow Follow-Up

- Dokime migrates its operations, catalogs, transactions and messages when it adopts the revision.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
