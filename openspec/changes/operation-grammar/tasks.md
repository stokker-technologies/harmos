# Operation Grammar Tasks

The owner locked the contract on 2026-11-12.
Boundary, managers and nested resources locked by the owner on 2026-11-12

This is a future implementation inventory, not permission to implement. This
documentation task edits only proposal.md, design.md, tasks.md and the five
specs/*/spec.md deltas in this change. No source, generated files, lifecycle
commands, push or product checkout edits are authorized by this task.

The locked source inventory covers library proto/protocol/operations/storage,
runtime managers and projection, all four SDKs and their build generators,
conformance, examples, tests and maintained guides; the product handoff covers
storage definitions, product operations/managers, runtime composition, SDKs, CLI,
desktop and removal of duplicate machinery. Before later implementation, record
exact owned paths within these domains. Six open questions remain at their
owning tasks; per-plugin settings/secrets ownership is closed. Preserve data
encodings and required behavior; no compatibility aliases or second authorities.
Every task below is unchecked future work. Order is dependency order: finalize
service names before the desktop moves once. Each source task migrates its
affected producer, generated bindings and compile-time consumers atomically.

## 1. Primitives, Final Descriptors and Benchmark Cases

- [ ] 1.1 Implement shared positions/cuts, bounded paging, Timestamp/Duration, wrappers, Failure, retry policy and accepted/durable receipts in proto/harmos/v1/ and crates/harmos-protocol/; verify focused schema/fingerprint checks.
- [ ] 1.2 Lock the complete declaration inventory to design.md: library 67 logical operations, 13 logical services (3 nested), 61 gRPC methods in 11 exposed services, 67 requests and 67 responses; Storage has 36 declarations (32 exposed, 4 native-only). Product handoff has 110 logical templates in 39 logical services (22 nested), including 92 client methods in 32 services (20 nested), 110 requests and 110 responses. Census domain/shared/imported payloads separately; no automatic managed-envelope additions.
- [ ] 1.3 Declare final nested packages, one proto per service in a parent folder, package-qualified wrappers and exposure metadata. Replace verb-plus-part/flat compound parts with nested resources; retain the established mixed-resource Storage contract. Integrate the plugin-manager branch's final entry contracts rather than create a second implementation.
- [ ] 1.4 Replace automatic RunMethod twins in runtime projection and packaging/client generators with explicit methods; streams serve Watch/plugin carriers or an explicitly promised final-result replacement. Generate all 32 exposed Storage methods from one authoritative owner, preserving native-only/grant boundaries, and update proto/harmos/v1/behaviour.md.
- [ ] 1.5 Add reusable Nightly integration/durable_lag with small_high_rate, multi_megabyte and mixed backend loads; report acceptance-to-durability p50/p95/maximum, backlog and synchronous commit contribution per load. Compare each p95 with 250 ms independently of payload size; try smaller/earlier size-triggered flushes first if missed, then remeasure. Accepted-data snapshots remain the named fallback, not built; no timing assertions.
- [ ] 1.6 Add Nightly integration/file_transfer for large unary client/plugin uploads and downloads; record bytes, chunk/range configuration and throughput before deleting streaming transfers, without timing assertions or new consumer budgets.

## 2. Library Owners and Generic Services

- [ ] 2.1 Integrate already-in-progress plugin-manager work and expose Plugins Create/Get/List/Watch/Set/Restart/Delete with managed/remote entries, verified bundles, persistence, state and failures. Preserve Plugin protocol carriers, activation fencing, disconnect isolation and cooperative settlement; migrate affected library consumers in the same compiling slice.
- [ ] 2.2 Implement Access and access.Listener under GrpcManager: CreatePairing, CreateCertificate regeneration/key rotation and Listener.Get/Set with complete writable values/revisions. Move generic listener/pairing/certificate orchestration here; verify focused lifecycle, invalid rotation and conflict behavior.
- [ ] 2.3 Implement Status, status.Logs and status.TraceEvents under ServiceManager: host identity, health, availability, bounded log/trace reads/watches and diagnostics archive references. Read archive bytes through Storage.GetFileBytes, with no second transfer implementation.
- [ ] 2.4 Add narrow product-contribution registration under ServiceManager for product version/identity and component health/availability; define registration lifetime, aggregation and provider failure handling, and prove focused registration/removal/failure behavior without a product-specific dependency.
- [ ] 2.5 Implement generic host-only HostValues.Get and its host resolver registration under OperationManager, plus the plugin catalog's settings-declaration section. The product supplies settings/secrets stores, schemas, UI, archives, credential integration and policy. Verify scoped value delivery/failure behavior; add no library value store or client settings service.
- [ ] 2.6 Retain OperationManager for CatalogEntries/Invocations, StreamManager for Samples/publication, existing host-clock/service lifetime for Time, and StateManager/RecordManager for their existing responsibilities; migrate names and declarations without duplicate managers or schedulers.

## 3. Storage Handlers, Native Providers and Transfers

- [ ] 3.1 Migrate backend.rs, state recovery and provider/manager consumers through the 66-present/60-target hook mapping while preserving encodings, atomic publication and recovery; verify focused backend/restart tests and contract guidance.
- [ ] 3.2 Implement latest-durable resolution without read-triggered flush/wait, issued-position continuation and bounded paging; verify controlled-backend visibility, exact continuation and foreign/expired-position refusal, and update docs/storage.md.
- [ ] 3.3 Implement whole-value revision-guarded Get/Set mirrors, complete-candidate validation, atomic compare/exchange, explicit create-if-absent and conflicts naming “get again and retry”. Preserve immutable schemas; large writable parts use nested resource Get/Set and their own revisions.
- [ ] 3.4 Migrate records, samples, log lines and upload bytes to bounded immutable Append and accepted/durable receipts; verify client-owned resending, original receipts, changed payload/expired-key refusal and immutable prefixes, including identical-byte offset replay and different-byte conflicts regardless of request identity.
- [ ] 3.5 Reuse StorageManager's production owner for CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction and DeleteFileProduction; verify shared client/plugin resume, fencing, integrity, grants, publication and abandonment. Unsupported providers refuse explicitly.
- [ ] 3.6 Implement bounded unary Storage.GetFileBytes with offset, maximum bytes, total length, digest, optional at and resolved_at; preserve WatchFileBytes replacement/truncation semantics and distinct directory/production watches. Migrate affected query/transfer consumers and examples in compiling slices.
- [ ] 3.7 Bound flushes in bytes and split large input; prove committed byte counts with deterministic controlled-backend tests and reuse Nightly cases for lag/throughput evidence.
- [ ] 3.8 Settle issued-position/page-token lifetime, provenance, composite retention and restart behavior at this task, record it in design.md, and implement explicit expiry/unavailability without arbitrary earlier reads.
- [ ] 3.9 Settle exact Watch opening/events/progress/gaps/resynchronization, record it in design.md, and implement one WatchRows selector for records/samples/object revisions with deterministic opening-race/filter-progress/gap proof. Keep accepted facts distinct from durable facts.
- [ ] 3.10 Settle retained-reader lifetime/reclamation before DeleteFile changes, record it in design.md and prove supported backends without implicit data loss.
- [ ] 3.11 Enumerate cooperative final-result promises, record them in design.md and project one replacement method per actual promise. Preserve running-only Cancel, queued refusal naming Cancel, distinct Stop and typed settlement.

## 4. Four-Language SDK Roots and Library Consumers

- [ ] 4.1 Produce a common descriptor table with owner/version, parent/service/method, wrappers, cardinality and exposure; add small build generators for Rust, Python, C# and C++ hierarchical roots. Compose library/product tables so Plugins and plugins.Settings share a node, preserve qualified message identity and reject actual leaf collisions. Prove generated paths for client.plugins.settings.get, client.executions.progress.watch and client.executions.start with no hand-written per-operation layer.
- [ ] 4.2 Migrate retained SDK/carrier contracts, service-qualified wrappers, fingerprints and policy alongside each declaration slice; preserve standalone protocol/SDK/conformance ownership and product SDK composition. Verify focused catalog, child-call, scope, clock, failure and call-local policy behavior.
- [ ] 4.3 Settle the semantic plugin-parity promise across Rust, Python, C# and C++ and record it in design.md; four-language client-root naming is already locked.
- [ ] 4.4 Settle generic managed-client execution coverage (Rust-only or more languages) and record it in design.md without reopening the naming hierarchy.
- [ ] 4.5 Align Python's default to Private with explicit Public intent, add C# invocation-local resources/reason-bearing cancellation and correct the stale draft proto header; verify focused behavior and affected guides.
- [ ] 4.6 Migrate tools/conformance/, fixtures, native/guest examples, generated clients and affected tests in the owning compiling slices; preserve interval/device publication and update README/topic/language-guide claims with each user-facing change.

## 5. Product Handoff, Composition and One Desktop Migration

- [ ] 5.1 Prepare the exact library candidate for later authorized publication/pinning; hand off final descriptors, counts and manager/service ownership. Publish/pin only with that later authorization; implement no product resources in this repository.
- [ ] 5.2 In the product repository, adopt the pinned runtime and define its own schemas/indexes/projections/latest selection, objects and authorization policy. Serve library Plugins, Access/access.Listener, Storage transfers and Status/status parts directly on its endpoint by composition, with no redeclarations or wrappers; register product facts with ServiceManager.
- [ ] 5.3 Split the product's single host/shared state into execution (queueing/lifecycle/retry/Stop/Cancel/recovery), prompt and report-publication managers; decide cycles by the simplest complete option and add a capture manager only if recordings remain. Keep rendering pure and evidence/catalog/notes/aggregates/plots/documents as operations over storage; map every service method to one operation file.
- [ ] 5.4 Declare/migrate all final product nested resource services and wrappers from the 132-row mapping, and the 24-row plugin mapping plus Effects.Get companion. Keep Plugins.Settings, Settings, Secrets and Storage.Sources product-owned. Use generic HostValues.Get for plugin settings/secrets delivery. Preserve checkpoint workflows at issued cuts without aggregate compatibility RPCs.
- [ ] 5.5 Expose Executions.Progress.Get/Watch, include the same current progress in Executions.Get and register producer Executions.Progress.Set only as a scoped host operation a plugin calls. Verify no client Set projection and preserve whole small execution Get/Set for rename/archive/pause without sibling mutation.
- [ ] 5.6 In each compiling adoption slice, delete product copies of plugin connection, dispatch, streams, process launch, entry management, listener/pairing, upload handling and generic host status, including superseded adapters/tests/configuration/guidance. Preserve product file/artifact metadata; use library bytes/range/watch/session methods.
- [ ] 5.7 Generate/migrate product SDK roots from the combined descriptors, then migrate its CLI to final services, unary transfers, receipts and issued-position paging. Verify focused product behavior.
- [ ] 5.8 After every service has its final name and SDK/CLI migration is complete, migrate the product desktop host and regenerate its client root exactly once against the same pinned library revision; verify focused interactions and delete the superseded desktop host paths in that slice.

## 6. Superseded Code and Source Closure

- [ ] 6.1 After every consumer migrates and Nightly large-file throughput is measured, delete remaining streaming transfers, automatic twins/envelopes, superseded methods/policies/generators/fixtures/adapters and guidance. Retain Watch, plugin carriers and promised final-result replacements; preserve mandatory hooks and existing data.
- [ ] 6.2 Review affected README, docs/product-guide.md, docs/storage.md, language guides, generators and capability contracts with each owning source change; verify no stale boundary or compound-resource naming claims.
- [ ] 6.3 Verify every locked implementation item is closed in the combined candidate, all writers have handed off, and focused tests plus owned format/language checks pass before declaring source closure. Source closure is not full validation or delivery.

## 7. Final Validation

- [ ] 7.1 After implementation source closure, run strict OpenSpec validation and required combined repository/language checks; repair and report actual failures.
- [ ] 7.2 Under later delivery authorization, prove the combined library and pinned product through configured hosted CI/Nightly on native Windows and Linux, including SDK/conformance, restart, cancellation, paging, unary transfers and both benchmarks; report exact revisions/results/pending evidence.
- [ ] 7.3 Reconcile owner-qualified services (including nested), operations, gRPC methods, method wrappers and full descriptor payload counts with the locked inventory; verify owner acceptance and leave unperformed proof pending.

## Workflow Follow-Up

- This documentation task commits only this change locally on the current branch; no implementation, lifecycle commands, push, review, merge or publication.
- Its strict documentation validation does not close any future implementation or final-proof checkbox.
