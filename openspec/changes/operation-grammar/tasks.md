# Operation Grammar Tasks

The owner locked the contract on 2026-11-12.
Boundary, managers and nested resources locked by the owner on 2026-11-12
Separation by audience locked by the owner on 2026-11-12

This is a future implementation inventory, not permission to implement. This
documentation task edits only proposal.md, design.md, tasks.md and the five
specs/*/spec.md deltas in this change. No source, generated files, lifecycle
commands, push or product checkout edits are authorized by this task.

The locked source inventory covers library proto/protocol/operations/storage,
runtime managers and projection, all four SDKs and their build generators,
conformance, examples, tests and maintained guides. Before later implementation, record
exact owned paths within these domains. Six open questions remain at their
owning tasks; per-plugin settings/secrets ownership is closed. Preserve data
encodings and required behavior; no compatibility aliases or second authorities.
Every task below is unchecked future work. Order is dependency order: establish
face package layout and move files before primitives, descriptors or consumers
migrate. Each source task migrates its affected producer, generated bindings and
compile-time consumers atomically.

## 1. Face Packages, Primitives, Final Descriptors and Benchmark Cases

- [ ] 1.1 Establish face packages and file moves first: proto/harmos/plugin/v1/instance.proto for Instance; proto/harmos/context/v1/ for context services; proto/harmos/v1/ and parent folders for client services. Move owning payloads/imports and declaration metadata with the packages; remove the retired singular carrier service. Keep native hooks out of wire services. Integrate the existing plugin-manager entry contracts. Everything below depends on this layout.
- [ ] 1.2 Implement shared positions/cuts, bounded paging, Timestamp/Duration, Failure, retry policy and receipts; declare separate face-local service/method request and response wrappers and regenerate IDs/fingerprints in crates/harmos-protocol/. Verify focused schema checks after the file moves; do not reuse another face's wrappers.
- [ ] 1.3 Lock design.md's inventory: protocol 1 service/4 operations/4 gRPC methods/4 request-response pairs; context 6/40/40 descriptor methods (0 standalone)/40 pairs; client 10 (3 nested)/58/58/58 pairs. Retain four native-only storage hooks/pairs separately: 17 services, 106 inventory operations, 102 descriptor methods (62 standalone), 106 requests and 106 responses overall. Verify all 38 shared capabilities have independent context/client declarations and authorization; census shared/domain/imported types separately.
- [ ] 1.4 Replace automatic RunMethod twins with explicit face-filtered projection. Generate the 32 Storage declarations independently on context and client faces from their respective declaration owners, sharing handlers while preserving native-only/grant boundaries. Use nested parent packages, bare part verbs and one proto per service. Move/update protocol behaviour guidance with its face owner; streams remain Watch/plugin carriers or a promised final-result replacement.
- [ ] 1.5 Add reusable Nightly integration/durable_lag with small_high_rate, multi_megabyte and mixed backend loads; report acceptance-to-durability p50/p95/maximum, backlog and synchronous commit contribution per load. Compare each p95 with 250 ms independently of payload size; try smaller/earlier size-triggered flushes first if missed, then remeasure. Accepted-data snapshots remain the named fallback, not built; no timing assertions.
- [ ] 1.6 Add Nightly integration/file_transfer for large unary client/plugin uploads and downloads; record bytes, chunk/range configuration and throughput before deleting streaming transfers, without timing assertions or new consumer budgets.

## 2. Library Owners and Generic Services

- [ ] 2.1 Integrate already-in-progress plugin-manager work and expose Plugins Create/Get/List/Watch/Set/Restart/Delete with managed/remote entries, verified bundles, persistence, state and failures. Migrate the protocol carriers to harmos.plugin.v1.Instance Get/Invoke/Samples/Stop and preserve activation fencing, disconnect isolation and cooperative settlement; migrate affected library consumers in the same compiling slice.
- [ ] 2.2 Implement Access and access.Listener under GrpcManager: CreatePairing, CreateCertificate regeneration/key rotation and Listener.Get/Set with complete writable values/revisions. Move generic listener/pairing/certificate orchestration here; verify focused lifecycle, invalid rotation and conflict behavior.
- [ ] 2.3 Implement Status, status.Logs and status.TraceEvents under ServiceManager: host identity, health, availability, bounded log/trace reads/watches and diagnostics archive references. Read archive bytes through Storage.GetFileBytes, with no second transfer implementation.
- [ ] 2.4 Add narrow product-contribution registration under ServiceManager for product version/identity and component health/availability; define registration lifetime, aggregation and provider failure handling, and prove focused registration/removal/failure behavior without a product-specific dependency.
- [ ] 2.5 Implement generic context-only HostValues.Get and its host resolver registration under OperationManager, plus the plugin catalog's settings-declaration section. The product supplies settings/secrets stores, schemas, UI, archives, credential integration and policy. Verify scoped value delivery/failure behavior; add no library value store or client settings service.
- [ ] 2.6 Retain OperationManager for face-local CatalogEntries/Invocations and client Invocations.Invoke, StreamManager for context Samples/publication, existing host-clock/service lifetime for face-local Time, and StateManager/RecordManager for their existing responsibilities; migrate names and declarations without duplicate managers or schedulers.

## 3. Storage Handlers, Native Providers and Transfers

- [ ] 3.1 Migrate backend.rs, state recovery and provider/manager consumers through the 66-present/60-target hook mapping while preserving encodings, atomic publication and recovery; verify focused backend/restart tests and contract guidance.
- [ ] 3.2 Implement latest-durable resolution without read-triggered flush/wait, issued-position continuation and bounded paging; verify controlled-backend visibility, exact continuation and foreign/expired-position refusal, and update docs/storage.md.
- [ ] 3.3 Implement whole-value revision-guarded Get/Set mirrors, complete-candidate validation, atomic compare/exchange, explicit create-if-absent and conflicts naming “get again and retry”. Preserve immutable schemas; large writable parts use nested resource Get/Set and their own revisions.
- [ ] 3.4 Migrate records, samples, log lines and upload bytes to bounded immutable Append and accepted/durable receipts; verify client-owned resending, original receipts, changed payload/expired-key refusal and immutable prefixes, including identical-byte offset replay and different-byte conflicts regardless of request identity.
- [ ] 3.5 Reuse StorageManager's production owner for CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction and DeleteFileProduction; verify shared client/plugin resume, fencing, integrity, grants, publication and abandonment. Unsupported providers refuse explicitly.
- [ ] 3.6 Implement bounded unary Storage.GetFileBytes with offset, maximum bytes, total length, digest, optional at and resolved_at; preserve WatchFileBytes replacement/truncation semantics and distinct directory/production watches. Migrate affected query/transfer consumers and examples in compiling slices.
- [ ] 3.7 Bound flushes in bytes and split large input; prove committed byte counts with deterministic controlled-backend tests and reuse Nightly cases for lag/throughput verification.
- [ ] 3.8 Settle issued-position/page-token lifetime, provenance, composite retention and restart behavior at this task, record it in design.md, and implement explicit expiry/unavailability without arbitrary earlier reads.
- [ ] 3.9 Settle exact Watch opening/events/progress/gaps/resynchronization, record it in design.md, and implement one WatchRows selector for records/samples/object revisions with deterministic opening-race/filter-progress/gap proof. Keep accepted facts distinct from durable facts.
- [ ] 3.10 Settle retained-reader lifetime/reclamation before DeleteFile changes, record it in design.md and prove supported backends without implicit data loss.
- [ ] 3.11 Enumerate cooperative final-result promises, record them in design.md and project one replacement method per actual promise. Preserve running-only Cancel, queued refusal naming Cancel, distinct Stop and typed settlement.

## 4. Four-Language SDK Roots and Library Consumers

- [ ] 4.1 Produce a common descriptor table with owner/face/version, parent/service/method, wrappers, cardinality and exposure; add face-selecting build generators for Rust, Python, C# and C++. Generate plugin SDKs with protocol serving and context calling only, clients with client API only, and hosts with all three; generate hierarchical roots only for their audience. Compose library/product tables so Plugins and plugins.Settings share a node, preserve qualified message identity and reject actual leaf collisions. Prove generated paths for client.plugins.settings.get, client.jobs.progress.watch and client.jobs.start with no hand-written per-operation layer.
- [ ] 4.2 Migrate retained SDK/carrier contracts, service-qualified wrappers, fingerprints and policy alongside each declaration slice; preserve standalone protocol/SDK/conformance ownership and product SDK composition. Verify focused catalog, admission, cross-plugin Public invocation, Private refusal naming the action, client Public invocation, inherited scope, child cancellation, clock, failure and call-local policy behavior; deny client context/protocol calls and plugin client-only calls. Preserve explicit catalog visibility and one host registry.
- [ ] 4.3 Settle the semantic plugin-parity promise across Rust, Python, C# and C++ and record it in design.md; four-language client-root naming is already locked.
- [ ] 4.4 Settle generic managed-client invocation coverage (Rust-only or more languages) and record it in design.md without reopening the naming hierarchy.
- [ ] 4.5 Require explicit Private or Public visibility in all four SDKs with no default and reject omitted/unspecified visibility at admission, add C# invocation-local resources/reason-bearing cancellation and correct the stale draft proto header; verify focused behavior and affected guides.
- [ ] 4.6 Migrate tools/conformance/, fixtures, native/guest examples, generated clients and affected tests in the owning compiling slices; preserve interval/device publication and update README/topic/language-guide claims with each user-facing change.
- [ ] 4.7 Verify generated service-stub sets in all four languages: plugin = Instance server plus context callers, client = client-face callers, host = all faces. Reject cross-audience root/stub leakage while retaining imported shared payloads. Keep this generation contract separate from the open semantic parity and managed-client language questions.

## 5. Library Handoff and Source Closure

- [ ] 5.1 Prepare the exact library candidate for later authorized publication/pinning; hand off final descriptors, counts and manager/service ownership. Publish/pin only with that later authorization; implement no application resources in this repository.
- [ ] 5.2 After every consumer migrates and Nightly large-file throughput is measured, delete remaining streaming transfers, automatic twins/envelopes, superseded methods/policies/generators/fixtures/adapters and guidance. Retain Watch, plugin carriers and promised final-result replacements; preserve mandatory hooks and existing data.
- [ ] 5.3 Review affected README, docs/product-guide.md, docs/storage.md, language guides, generators and capability contracts with each owning source change; verify no stale boundary or compound-resource naming claims.
- [ ] 5.4 Verify every locked implementation item is closed in the combined candidate, all writers have handed off, and focused tests plus owned format/language checks pass before declaring source closure. Source closure is not full validation or delivery.

## 6. Final Validation

- [ ] 6.1 After implementation source closure, run strict OpenSpec validation and required combined repository/language checks; repair and report actual failures.
- [ ] 6.2 Under later delivery authorization, prove the combined library through configured hosted CI/Nightly on native Windows and Linux, including SDK/conformance, restart, cancellation, paging, unary transfers and both benchmarks; report exact revisions/results/pending verification.
- [ ] 6.3 Reconcile library services (including nested), operations, gRPC methods, method wrappers and full descriptor payload counts with the locked inventory; verify owner acceptance and leave unperformed proof pending.

## Workflow Follow-Up

- This documentation task commits only this change locally on the current branch; no implementation, lifecycle commands, push, review, merge or publication.
- Its strict documentation validation does not close any future implementation or final-proof checkbox.
