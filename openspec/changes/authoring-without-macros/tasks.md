# Tasks

## 1. Contract

- [x] 1.1 After authoritative host protobuf adoption under `grpc-plugins`, lock the source inventory per macro: uses in Harmos source, examples and tests, and in Dokime, with the typed projections each use depends on.

The current [source inventory](proposal.md#source-inventory) records every textual
attribute site, dependent projection, live alias export and excluded-path dependency.
The host protobuf cutover is complete. This worktree owns all Rust consumers,
manifests, fixture tooling and guides; external application adoption remains the
follow-up below. Replacements follow the proposal's eleven-step order, with a
local commit and focused proof for each complete macro family.

## 2. Simple Declarations

- [x] 2.1 Inventory live `domain_catalog`, `runtime_catalog`, `domain_operation` and `runtime_operation` consumers and re-exports in domain, runtime, operations, examples and fixtures. Keep aliases until their families migrate in 3.1/3.2; only genuinely unreferenced variants may be deleted now.
- [x] 2.2 Replace `listener`, `streams` and `lanes` with values checked when built.
- [x] 2.2.1 Replace `listener`, migrate deliveries and guest constant descriptions, and delete its generator and exports.
- [x] 2.2.2 Replace `streams` and its membership projections with stream value lists.
- [x] 2.2.3 Replace `lanes` and its typed catalogs with validated lane values.
- [x] 2.3 Replace `record` with a typed entry and split `stream` into a row derive and a stream value.
- [x] 2.3.1 Replace `record` and its generator with typed record entries, preserving stored identity, revision and restoration.
- [x] 2.3.2 Replace `stream` with a field-reading row derive and a stream value, including guest constant descriptions.

## 3. Operations and Messages

- [x] 3.1 Replace `operation` with a request derive and an operation value, moving the declared-call and declared-resource checks to run time; migrate live `domain_operation`/`runtime_operation` aliases and consumers before removing their exports and generators.
- [x] 3.2 Replace the typed `catalog` enums with catalog values, giving storage and projections checked handles; migrate live `domain_catalog`/`runtime_catalog` aliases and consumers before removing their exports and generators.
- [x] 3.3 Replace `records` with typed entries in an owned registry.
- [x] 3.4 Turn `message` into a derive.

## 4. State

- [x] 4.1 Add the state catalog and typed transaction entries with explicit reversible and barrier declarations, and run the existing replay, recovery and inverse-law tests against it unchanged.
- [x] 4.2 Replace `transaction` and `transactions`, then delete their generators.

- [ ] 4.3 After all ordered declaration replacements, consolidate surviving field-reading derives in `sdk/rust/derive` (`harmos-sdk-derive`) and delete `harmos-macros`, coordinated with `grpc-plugins` 6.10. Preserve distinct stored-schema `Described`, protobuf Message and row semantics; migrate manifests, re-exports, consumers and derive coverage without SDK host dependencies. Domain reduction follows this consolidation; guest registration and `harmos-guest-macros` deletion remain with deferred `grpc-plugins` 7.1.

## 5. Final Validation

- [ ] 5.1 Run the operation, state, storage and example tests on Linux and Windows.
- [ ] 5.2 Update the product guide and authoring guidance.

## Workflow Follow-Up

- Dokime migrates its operations, catalogs, transactions and messages when it adopts the revision.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
