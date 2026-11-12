# Operation Grammar Tasks

The owner locked the contract on 2026-11-12. This is the future implementation
inventory, not implementation authorization. Settle the six open questions at
their owning tasks; they do not block independent locked work. Before source
edits, record exact owned paths within the listed domains and migrate all coupled
consumers without compatibility aliases or stored-data changes.

## 1. Shared Primitives and Nightly Benchmark Cases

- [ ] 1.1 Implement shared positions `(history, sequence)`, composite cuts, one bounded paging shape, protobuf Timestamp/Duration, response wrappers, Failure, retry policy and accepted/durable receipts in `proto/harmos/v1/` and `crates/harmos-protocol/`; verify focused schema, encoding and fingerprint checks.
- [ ] 1.2 Add reusable `integration/durable_lag` to the Nightly benchmark set with `small_high_rate`, `multi_megabyte` and `mixed` backend loads; report acceptance-to-durability p50, p95 and maximum per load, backlog and synchronous commit contribution, comparing each p95 with 250 ms independently of payload size. Use benchmarks, never wall-clock test assertions; if missed, try smaller and earlier size-triggered flushes first and remeasure. Accepted-data snapshots remain the named fallback, not built.
- [ ] 1.3 Add Nightly `integration/file_transfer` benchmark cases for large-file unary upload/download through client and plugin carriers, reporting bytes, configured chunk/range sizes and throughput; measure before deleting streaming transfers, without timing assertions or new consumer budgets.

## 2. Complete Proto Inventory Without Automatic Twins

- [ ] 2.1 Enumerate the complete proto inventory from the mapping: 47 logical operations, 36 storage declarations, 42 exposed gRPC methods in 3 services and 94 method request/response messages (84 exposed, 10 native-only); enumerate shared/domain/nested/imported payloads separately and verify every declaration and consumer is accounted for before handlers. Include GetIndexDefinition, GetProjectionDefinition, GetObject and native GetSchema as exact writable-value/revision mirrors of Set, plus GetFileProduction status.
- [ ] 2.2 Replace unconditional twins in runtime gRPC projection and packaging/client generators with explicit canonical gRPC methods; Get/List are unary, streams serve Watch and plugin carriers, and a managed final-result gRPC method replaces its ordinary gRPC method only where promised. Verify focused descriptor and generation checks, with no unreachable automatic envelopes in the final inventory.
- [ ] 2.3 Generate the 32 exposed Storage methods and SDK bindings from one authoritative declaration owner, with matching request/response wrappers; preserve grants and the five native-only operations in design.md. Update `proto/harmos/v1/behaviour.md` with the locked grammar and policy.

## 3. Handlers, Native Storage and Transfers

- [ ] 3.1 Migrate `crates/harmos-storage/src/backend.rs`, state recovery hooks and provider/manager consumers to the 66-present/60-target native-hook mapping while preserving encodings, atomic publication and recovery; verify focused backend/restart tests and update contract guidance.
- [ ] 3.2 Implement durable latest resolution and issued-position continuation in snapshots/query and handlers; verify deterministic no-read-triggered-flush, exact continuation, foreign/expired-position refusal and bounded paging tests, and update `docs/storage.md`.
- [ ] 3.3 Implement matching Get/Set pairs: Get returns the whole writable value and revision; Set atomically replaces it using that expected revision. Verify complete-candidate validation, stale conflicts naming “get again and retry”, create-if-absent and no partial mutation. Preserve immutable schema rules; map oversized writable parts to separately settable sub-resources, each with its own Get/Set and revision.
- [ ] 3.4 Migrate records, samples, log lines and upload bytes to bounded immutable Append and accepted/durable receipts; verify lost replies, changed replay payloads, expired keys and immutable prefixes, including successful identical-byte replay at accepted offsets and conflicting different bytes at the same offset regardless of request identity.
- [ ] 3.5 Bind the existing production owner to CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction and DeleteFileProduction; implement one client/plugin upload mechanism with ordered offsets, status/resume, fencing, integrity and grants. Verify deterministic reconnect, stale-writer, publication and abandonment cases and document capability refusal.
- [ ] 3.6 Implement shared unary GetFileBytes downloads with offset, configured maximum bytes, total length, digest, optional at and resolved_at; migrate bounded query/list delivery to unary paging. Verify exact-range, continuation, resume and content-integrity behavior and update transfer examples.
- [ ] 3.7 Bound flushes in bytes and split large input; prove committed byte counts with deterministic controlled-backend tests and document the mechanism. Reuse the benchmark cases to assess the locked durable-lag bound.
- [ ] 3.8 Settle issued-position and page-token validity, provenance, composite selection and restart behavior when implementing retention; record the decision in design.md and implement explicit expiry/unavailability without arbitrary earlier reads.
- [ ] 3.9 Settle exact Watch opening, events, progress, gaps and resynchronization at the Watch task; record the decision in design.md, implement one WatchRows selector for records/samples/object revisions, and preserve distinct file/directory/production watches with durable versus accepted facts. Verify deterministic opening-race, filter-progress and gap behavior.
- [ ] 3.10 Settle deletion while readers retain content before changing DeleteFile or reclamation; record the decision in design.md and verify it across supported backends without implicit data-loss migrations.
- [ ] 3.11 Enumerate operations promising a cooperative final result after cancellation at the managed-gRPC method task; record the decision in design.md and implement one replacement gRPC method per such promise. Preserve running-only Cancel, queued refusal naming Cancel, distinct Stop and typed settlement with focused tests.

## 4. Library SDKs, Plugin Hosts and Conformance

- [ ] 4.1 Settle four-language plugin semantic parity when SDK migration reaches it; record the promise in design.md and inventory Rust, Python, C# and C++ peers without moving library ownership.
- [ ] 4.2 Settle generic managed-client language coverage at its SDK task; record whether coverage is Rust-only or includes other languages, without treating current scope as a parity promise.
- [ ] 4.3 Migrate retained SDKs and host plugin carriers to names, wrappers, fingerprints and one policy vocabulary; preserve independent protocol/SDK/conformance ownership without product vocabulary, with product SDKs building on them. Verify focused catalog, child-call, failure, scope, clock and call-local policy tests and update language guides.
- [ ] 4.4 Align Python's default to Private with explicit Public intent, add C# invocation-local resources and reason-bearing cancellation, and correct the stale draft proto header; verify focused behavioral checks and update affected guides.
- [ ] 4.5 Preserve interval/device publication, activation fencing, disconnect isolation and cooperative settlement; migrate `tools/conformance/`, SDK fixtures, native/guest examples and affected tests, verifying each with focused project tasks.

## 5. Product Handoff, Command-Line Tool and Desktop

- [ ] 5.1 Prepare the exact library candidate for later authorized publication/pinning; hand off the product-owned host gaps and SDK migration without implementing product resources here. Product GetExecution/SetExecution replaces rename/archive setters with the whole small writable part and expected revision; rename affects one identified resource. Large writable resources split into named Get/Set sub-resources.
- [ ] 5.2 After product SDK migration, migrate the product command-line tool to the pinned grammar, unary transfers, receipts and issued-position paging; verify focused command behavior in the product repository.
- [ ] 5.3 After the command-line migration, migrate the product desktop host and regenerate its client against the same library revision; verify focused product interactions without adding product vocabulary to this library.
- [ ] 5.4 Review README, `docs/product-guide.md`, `docs/storage.md`, language guides, generator examples and capability contracts; update affected claims and links with each owning change, leaving unrelated migrations alone.

## 6. Delete Superseded Code Last and Close Source

- [ ] 6.1 After every affected consumer migrates and the Nightly large-file throughput case has been measured, delete streaming uploads/downloads, superseded gRPC methods, policies, adapters, generators, envelopes, fixtures and guidance; verify no automatic twins or streaming Get/List remain, retaining Watch, plugin carriers and explicitly promised managed replacements.
- [ ] 6.2 Verify the complete inventory is closed in one combined candidate with all writers handed off, focused tests and owned format/language checks complete; only then declare source closure. Final validation remains separate.

## 7. Final Validation

- [ ] 7.1 After implementation source closure, run strict OpenSpec validation and required repository/language checks for the combined candidate; report actual results and repair every failure.
- [ ] 7.2 Under later delivery authorization, validate the combined library and pinned product candidate through configured hosted CI and Nightly on native Windows and Linux, including SDK/conformance, restart, cancellation, paging, unary transfers and both benchmark cases; report exact revisions, results and pending evidence.
- [ ] 7.3 Review final operation/gRPC method/service/message counts and capability coverage against the locked contract and full descriptor payload inventory; verify owner acceptance and leave unperformed proof pending.

## Workflow Follow-Up

- This documentation task commits only this locked change locally; implementation, push, review creation, publication and specification activation or retirement are not authorized.
- Keep final validation unchecked until the future implementation proof is actually performed; this documentation task's strict validation does not close it.
