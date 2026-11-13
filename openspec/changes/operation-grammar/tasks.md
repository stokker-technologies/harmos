# Operation Grammar Tasks

The owner locked the contract on 2026-11-12.
Boundary, managers and nested resources locked by the owner on 2026-11-12
Separation by audience locked by the owner on 2026-11-12
Three visibility levels locked by the owner on 2026-11-12

Implementation is authorized under the combined `grpc-plugins` marker. Storage ownership:
`proto/harmos/**/storage*.proto`, `crates/harmos-storage/**`,
`crates/harmos-operations/src/storage/**`, storage runtime handlers/tests/benches,
storage benchmark tooling/Nightly wiring, `docs/storage.md`, and storage tasks/design.
Core owns shared protocol primitives, generators, SDKs and non-storage runtime.
The task numbers below supersede the earlier inventory: durable lag is 1.5
(formerly 1.2), file transfer is 1.6 (formerly 1.3), storage declarations are 1.4.

The locked source inventory covers library proto/protocol/operations/storage,
runtime managers and projection, all four SDKs and their build generators,
conformance, examples, tests and maintained guides. Before later implementation, record
exact owned paths within these domains. Three open questions remain at their
owning tasks; per-plugin settings/secrets ownership is closed. Preserve data
encodings and required behavior; no compatibility aliases or second authorities.
Order is dependency order: establish
face package layout and move files before primitives, descriptors or consumers
migrate. Each source task migrates its affected producer, generated bindings and
compile-time consumers atomically.

## 0. Visibility Guards and SDK Dispatch

- [x] 0.1 Add the deterministic story guarding **Three-Level Visibility** and **Host-Mediated Public Calls** first: plugin to host to another plugin's Public operation succeeds; Private runs only in its declaring process, is absent from the catalog and every wire, and is unreachable from the host, another plugin or a client. Cover both call directions, returned failures, inherited scope, requirements/fingerprints and child cancellation.
- [x] 0.2 Add the third visibility enum value (Protected), explicit SDK selection with no default, and admission rules rejecting unspecified values, Private catalog entries, and Private/Protected client gRPC bindings. Preserve Public routing through the host.
- [x] 0.3 Implement SDK-local Private dispatch with ordinary policy, retry, deadline and cancellation semantics in Rust (sdk/rust/src/context.rs): Proof: conformance cases 24 and 26–28 and policy_clock_spans_keep_large_durations_and_fractional_seconds; the Rust guest component story also checks catalog omission and zero private import calls.
- [x] 0.4 Implement the same SDK-local Private dispatch in Python (sdk/python/src/harmos/invocation.py): Proof: conformance cases 24 and 26–28 exercise local handlers and host-clock policy.
- [x] 0.5 Implement the same SDK-local Private dispatch in C# (sdk/csharp/Harmos/Context.cs): Proof: conformance cases 24 and 26–28 exercise local handlers, resources and reason-bearing cancellation.
- [x] 0.6 Implement the same SDK-local Private dispatch in C++ (sdk/cpp/include/harmos/harmos.hpp): Proof: conformance cases 24 and 26–28 exercise local handlers, host-clock retries, deadline and child cleanup.
- [x] 0.7 Add conformance cases in each SDK: Private absent from catalogs and callable only locally; Protected invocable by host and declaring plugin but refused to another plugin and a client; Public invocable by another plugin with a requirement and by a client through the host. Verify no Private bytes appear even on the host/plugin carrier.
- [ ] 0.8 Apply design.md's decided built-in mapping, including reserved controls and native hooks; this records the resolution of operation-visibility task 1.7 without editing that earlier change. Keep product host-only requests of a plugin declared Protected and prove their refusal to other plugins/clients.

- [x] 0.9 Guard the identified host-built-in admission gap with a deterministic migrated Protected built-in story: a plugin with a matching declared requirement is still refused, as is a client, while the declaring host retains access. Fix submit_grpc_child's plugin-binding-only visibility gate; do not lock its bypass in as compatibility.

## 1. Face Packages, Primitives, Final Descriptors and Benchmark Cases

- [ ] 1.1 Establish face packages and file moves first: proto/harmos/plugin/v1/instance.proto for Instance; proto/harmos/context/v1/ for context services; proto/harmos/v1/ and parent folders for client services. Move owning payloads/imports and declaration metadata with the packages; remove the retired singular carrier service. Keep native hooks out of wire services. Integrate the existing plugin-manager entry contracts. Everything below depends on this layout.
- [x] 1.1.1 Move the non-storage carrier and resource services into per-face, per-service files; regenerate Rust/Python and migrate native SDK/host consumers. Focused proof: generated_code_matches_proto; focused SDK/runtime builds. Storage, generic management services and runtime face admission remain with their owning tasks.
- [ ] 1.2 Implement shared positions/cuts, bounded paging, Timestamp/Duration, Failure, retry policy and receipts; declare separate face-local service/method request and response wrappers and regenerate IDs/fingerprints in crates/harmos-protocol/. Verify focused schema checks after the file moves; do not reuse another face's wrappers.
- [ ] 1.3 Lock design.md's inventory: protocol 1 service/4 operations/4 gRPC methods/4 request-response pairs; context 6/40/40 descriptor methods (0 standalone)/40 pairs; client 10 (3 nested)/58/58/58 pairs. Retain four native-only storage hooks/pairs separately: 17 services, 106 inventory operations, 102 descriptor methods (62 standalone), 106 requests and 106 responses overall. Verify all 38 shared capabilities have independent context/client declarations and authorization; census shared/domain/imported types separately.
- [ ] 1.4 Replace automatic RunMethod twins with explicit face-filtered projection. Generate the 32 Storage declarations independently on context and client faces from their respective declaration owners, sharing handlers while preserving native-only/grant boundaries. Use nested parent packages, bare part verbs and one proto per service. Move/update protocol behaviour guidance with its face owner; streams remain Watch/plugin carriers or a promised final-result replacement.
- [x] 1.5 Add reusable Nightly integration/durable_lag with small_high_rate, multi_megabyte and mixed backend loads; report acceptance-to-durability p50/p95/maximum, backlog and synchronous commit contribution per load. Compare each p95 with 250 ms independently of payload size; try smaller/earlier size-triggered flushes first if missed, then remeasure. Accepted-data snapshots remain the named fallback, not built; no timing assertions.
- [x] 1.6 Add Nightly integration/file_transfer for large unary client/plugin uploads and downloads; record bytes, chunk/range configuration and throughput before deleting streaming transfers, without timing assertions or new consumer budgets.

## 2. Library Owners and Generic Services

- [ ] 2.1 Integrate already-in-progress plugin-manager work and expose Plugins Create/Get/List/Watch/Set/Restart/Delete with managed/remote entries, verified bundles, persistence, state and failures. Migrate the protocol carriers to harmos.plugin.v1.Instance Get/Invoke/Samples/Stop and preserve activation fencing, disconnect isolation and cooperative settlement; migrate affected library consumers in the same compiling slice.
- [ ] 2.2 Implement Access and access.Listener under GrpcManager: CreatePairing, CreateCertificate regeneration/key rotation and Listener.Get/Set with complete writable values/revisions. Move generic listener/pairing/certificate orchestration here; verify focused lifecycle, invalid rotation and conflict behavior.
- [ ] 2.3 Implement Status, status.Logs and status.TraceEvents under ServiceManager: host identity, health, availability, bounded log/trace reads/watches and diagnostics archive references. Read archive bytes through Storage.GetFileBytes, with no second transfer implementation.
- [ ] 2.4 Add narrow product-contribution registration under ServiceManager for product version/identity and component health/availability; define registration lifetime, aggregation and provider failure handling, and prove focused registration/removal/failure behavior without a product-specific dependency.
- [x] 2.5 Implement generic context-only HostValues.Get and its host resolver registration under OperationManager, plus the plugin catalog's settings-declaration section. The product supplies settings/secrets stores, schemas, UI, archives, credential integration and policy. Verify scoped value delivery/failure behavior; add no library value store or client settings service.
- [x] 2.6 Retain OperationManager for face-local CatalogEntries/Invocations and client Invocations.Invoke, StreamManager for context Samples/publication, existing host-clock/service lifetime for face-local Time, and StateManager/RecordManager for their existing responsibilities; migrate names and declarations without duplicate managers or schedulers.

## 3. Storage Handlers, Native Providers and Transfers

- [x] 3.1 Migrate backend.rs, state recovery and provider/manager consumers through the 66-present/60-target hook mapping while preserving encodings, atomic publication and recovery; verify focused backend/restart tests and contract guidance.
- [x] 3.2 Implement latest-durable resolution without read-triggered flush/wait, issued-position continuation and bounded paging; verify controlled-backend visibility, exact continuation and foreign/expired-position refusal, and update docs/storage.md.
- [x] 3.3 Implement whole-value revision-guarded Get/Set mirrors, complete-candidate validation, atomic compare/exchange, explicit create-if-absent and conflicts naming “get again and retry”. Preserve immutable schemas; large writable parts use nested resource Get/Set and their own revisions.
- [ ] 3.4 Migrate records, samples, log lines and upload bytes to bounded immutable Append and accepted/durable receipts; verify client-owned resending, original receipts, changed payload/expired-key refusal and immutable prefixes, including identical-byte offset replay and different-byte conflicts regardless of request identity.
- [x] 3.5 Reuse StorageManager's production owner for CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction and DeleteFileProduction; verify shared client/plugin resume, fencing, integrity, grants, publication and abandonment. Unsupported providers refuse explicitly.
- [x] 3.6 Implement bounded unary Storage.GetFileBytes with offset, maximum bytes, total length, digest, optional at and resolved_at; preserve WatchFileBytes replacement/truncation semantics and distinct directory/production watches. Migrate affected query/transfer consumers and examples in compiling slices.
- [x] 3.7 Bound flushes in bytes and split large input; prove committed byte counts with deterministic controlled-backend tests and reuse Nightly cases for lag/throughput verification.
- [x] 3.8 Settle issued-position/page-token lifetime, provenance, composite retention and restart behavior at this task, record it in design.md, and implement explicit expiry/unavailability without arbitrary earlier reads.
- [x] 3.9 Settle exact Watch opening/events/progress/gaps/resynchronization, record it in design.md, and implement one WatchRows selector for records/samples/object revisions with deterministic opening-race/filter-progress/gap proof. Keep accepted facts distinct from durable facts.
- [ ] 3.10 Settle retained-reader lifetime/reclamation before DeleteFile changes, record it in design.md and prove supported backends without implicit data loss.
- [ ] 3.11 Enumerate cooperative final-result promises, record them in design.md and project one replacement method per actual promise. Preserve running-only Cancel, queued refusal naming Cancel, distinct Stop and typed settlement.

## 4. Four-Language SDK Roots and Library Consumers

- [x] 4.1 Produce a common descriptor table with owner/face/version, parent/service/method, wrappers, cardinality and exposure; add face-selecting build generators for Rust, Python, C# and C++. Generate plugin SDKs with protocol serving and context calling only, clients with client API only, and hosts with all three; generate hierarchical roots only for their audience. Compose library/product tables so Plugins and plugins.Settings share a node, preserve qualified message identity and reject actual leaf collisions. Prove generated paths for client.plugins.settings.get, client.jobs.progress.watch and client.jobs.start with no hand-written per-operation layer.
- [x] 4.2 Migrate retained SDK/carrier contracts, service-qualified wrappers, fingerprints and policy alongside each declaration slice; preserve standalone protocol/SDK/conformance ownership and product SDK composition. Verify focused catalog, admission, cross-plugin Public invocation, Protected refusal naming the action and local-only Private execution, client Public invocation, inherited scope, child cancellation, clock, failure and call-local policy behavior; deny client context/protocol calls and plugin client-only calls. Preserve explicit catalog visibility and one host registry.
- [x] 4.3 Settle the semantic plugin-parity promise across Rust, Python, C# and C++ and record it in design.md; four-language client-root naming is already locked.
- [x] 4.4 Settle generic managed-client invocation coverage (Rust-only or more languages) and record it in design.md without reopening the naming hierarchy.
- [x] 4.5 Require explicit Private, Protected or Public visibility in all four SDKs with no default and reject omitted/unspecified visibility at admission, add C# invocation-local resources/reason-bearing cancellation and correct the stale draft proto header; verify focused behavior and affected guides.
- [x] 4.6 Migrate tools/conformance/, fixtures, native/guest examples, generated clients and affected tests in the owning compiling slices; preserve interval/device publication and update README/topic/language-guide claims with each user-facing change.
- [x] 4.7 Verify generated service-stub sets in all four languages: plugin = Instance server plus context callers, client = client-face callers, host = all faces. Reject cross-audience root/stub leakage while retaining imported shared payloads. Keep this generation contract separate from the open semantic parity and managed-client language questions.

## Architecture Guards

These are future deterministic behavior/structure guards for the named invariants
in docs/architecture.md, not tests of that document's wording. Partial existing
proof does not close them; no checkbox is completed by this documentation task.

- [ ] A.1 Add **Three Audience Faces** descriptor/generation and admission cases for each SDK, rejecting cross-audience calls and stubs (tasks 1.1, 4.7).
- [ ] A.2 Add **Explicit Streaming and Cooperative Control** stories proving no generated twins, the single declared final-result replacement, running cancellation/child settlement, distinct Stop and preservation of queued action refusals (tasks 1.4, 3.11).
- [ ] A.3 Add **Manager Ownership** lifecycle stories for entry activation/retirement, listener rotation, product-fact registration/removal and resolver failure/cleanup, showing each service uses its one manager (tasks 2.1–2.6).
- [ ] A.4 Add **Library and Product Composition** independent-host stories composing library services/resolvers without redeclarations, duplicate hosting or product settings/secrets stores in the library (tasks 2.1–2.5).
- [ ] A.5 Add **Resource Operation Grammar** descriptor and four-language generated-call stories for bare verbs, nested packages, service-qualified wrappers, stale whole-value Set and immutable Append (tasks 1.2–1.4, 3.3–3.4, 4.1).
- [ ] A.6 Add **Position-Resolved Reads** controlled-backend stories for optional at, immediate latest-durable selection, resolved_at in every reply, issued-position paging and durable watch boundaries; keep lag evidence in Nightly benchmarks, not timing assertions (tasks 1.5, 3.2, 3.8–3.9).
- [ ] A.7 Add **Stable Receipts After Lost Replies** client stories across the migrated mutation surface proving same normalized request returns its original receipt, changed payload conflicts, expiry refuses and admitted completion survives caller loss (task 3.4).
- [x] A.8 Add **Callee-Owned Policy and Consumer Budgets** controlled-clock/backend stories for call-local overrides and configured backend I/O, retries and session lifetimes; migrate hardcoded operational policy in crates/harmos-storage/src/config.rs to its owning policy/configuration without weakening limits. Preserve consumer admission/concurrency/response sizing and replay-safety checks.
- [x] A.9 Add a deterministic production dependency-graph check for **Crate Dependency Direction**, rejecting upward runtime/product dependencies from lower library crates or SDKs while permitting declared derive/build/dev roles.
- [x] A.10 Add **Generic Library SDKs** independent-host conformance and generated-root cases in all four languages using only shared generic concepts; do not close unresolved semantic parity/client-coverage questions by assumption (tasks 4.1, 4.3–4.4).
- [ ] A.11 Audit **Deterministic Stories** repository-wide: replace timing/load/sampled-memory assertions with controlled-clock/event journeys, move measurements to benchmarks, and remove prose/constant/tautological assertions. Add any missing deterministic deadline/cancellation stories; never test authored wording.

**Three-Level Visibility** and the combined **Host-Mediated Public Calls** guard
are deliberately first in tasks 0.1–0.8, including all four SDK-local dispatches
and the complete built-in mapping.


### Owned Guards and Integration Boundary

- [x] 0.8.1 Migrate native reserved lifecycle/control declarations to Protected and context/client built-ins to their decided faces; prove the built-in refusal and owner access. Storage's built-in mapping remains task 0.8 with its owning worktree.
- [x] A.1.1 Guard non-storage face descriptors, separate wrappers, client listener admission and plugin client-face refusal; select audience stubs in all four SDKs. The complete descriptor census remains 1.3 after the other service owners merge.
- [x] A.2.1 Guard explicit non-storage streams and managed replacement, queued product Cancel refusal/cause, separate safe-boundary Stop, peer cancellation and local child cleanup. Proof: audience_faces, generated_sdk_client_invokes_streams_and_settles_cancellation, client_preserves_queued_cancel_refusal_and_stop_waits_for_the_product_boundary, peer_journey and conformance 25–28.
- [x] A.5.1 Prove face-local wrappers and four-language library/product root composition, including Plugins with plugins.Settings and Jobs with jobs.Progress. Whole-value Set/Append and position semantics remain with storage.
- [x] A.8.1 Prove SDK-local call policy, replay-safe retries, host-clock deadlines and cooperative grace without raising budgets. Proof: common conformance 26–28 in all four languages.

Owned source: `proto/harmos/{plugin,context}/v1`, non-storage client proto files,
`crates/harmos-protocol`, non-storage `crates/harmos-operations`, runtime operations
and gRPC instance execution outside management/access/status, `sdk`, conformance,
examples and their build generators. The shared Identifier alphabet accepts the
canonical service/method separator. Storage, Plugins, Access and Status inventory,
benchmark and complete-service guards remain open for their declared owners;
combined source closure and hosted validation are not claimed here.

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

- Storage implementation commits remain local on `feat/gp-grammar-storage`; no push or specification lifecycle commands.
- Transfer source/proof: `storage::unary_transfers::resume_fence_publish_and_range_across_faces` exercises the socket client and plugin child carrier. Remaining read-provenance, paging and Watch work stays in 3.2, 3.8 and 3.9.

- Storage generation join: the core branch now supplies face-aware generation. Storage uses `crates/harmos-protocol/codegen.rs`
  and its generate example for both storage faces and reflection. The merged generator
  retains these inputs, `context` Rust module, shared `storage_types.proto`
  imports and independently owned context/client operation declarations when joined.

- Storage descriptor inventory: both face-local Storage services now declare all 32
  locked method pairs; the shared payload file declares no service. Rust/Python bindings
  regenerate through the joined core generator. Focused proof:
  `generated_code_matches_proto::generated_code_matches_proto` (1 passed). Task 1.4
  storage source is complete: every declaration is bound to its migrated shared handler.

- Task 3.3 partial: index/projection Get/Set pairs are served independently on both
  faces with shared handlers and atomic expected-revision checks in SQLite and the
  reference catalogs. Explicit create-if-absent and stale unchanged values are covered.
  Focused proofs: `index_seek::seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart`,
  `projection_replay::projection_frontiers_survive_interrupted_publication_rebuild_and_restart`,
  and `storage::generated_native_journey::saved_queries_and_conditional_updates_cross_the_protocol`
  (one passed each). Schema/object mirrors and issued definition cuts are implemented below.

- Tasks 3.2/3.8 partial: issued saved cuts, HMAC-bound ListRows pages and retained
  ranged-file reads share the existing saved-snapshot owner on both faces.
  Capabilities/CreateSnapshot/DeleteSnapshot/ListColumnRows/ListIndexEntries replace
  their retired declarations; the streaming Query operation and consumers are removed.
  Focused proofs (one passed each):
  `storage::issued_pages::issued_pages_keep_their_cut_while_newer_writes_are_blocked`,
  `storage::issued_cut_lifetime::cached_cuts_expire_while_explicit_leases_pin_their_image`,
  `storage::operations::queries::batches::query_operation_batches`,
  `storage::operations::plugins::query_grants_enforced`,
  `storage::operations::schemas::schema_registry_roundtrip`,
  plus the existing unary-transfer and native-journey stories. Projection/status,
  object/directory reads and Watch joins are implemented below.
  Coupling: one native-only operations dependency on the already locked ring version
  provides HMAC; the lockfile gains only that dependency edge. Retired Rust SDK
  request bindings are deleted with their operation owners.

- Projection read migration: independent AdvanceProjection/GetProjectionStatus/
  ListProjectionRows declarations share revision/source/cut validation. Omitted
  frontiers select the newest readable materialization; explicit frontiers require
  an issued composite, without flush or latest substitution. Retired declarations,
  request types and SDK bindings are removed. Focused proof:
  `storage::generated_native_journey::saved_queries_and_conditional_updates_cross_the_protocol`
  (one passed). Complete unary reply envelopes enforce the storage response ceiling.

- Bounded query progress now retains QueryReader state instead of rescanning prefixes.
  Source/output steps return bounded data or empty progress pages; latest-token replay,
  capacity eviction, current invocation binding and active-slot release are implemented.
  Durable receipt positions can open retained exact reads after saved-frontier proof.
  Focused proof: `storage::filtered_pages::filtered_physical_pages_return_progress_without_rescanning_the_prefix`
  plus both issued-cut stories, query batches/grants/schema, unary transfer and native
  journey stories (eight named focused tests passed). The controlled provider records
  source work; replay does not rescan it. Receipt-position proof uses the publication
  receipt directly on the first ranged read. The remaining file/directory methods and Watch still keep 3.2/3.8 open;
  common native-list paging and metadata/object mirrors are implemented below.

- Definition Set receipts now commit atomically with the complete index/projection
  candidate or refusal. The manager owns admitted work through caller loss; retries
  preserve the original outcome after later revisions, and source replacement checks
  both sources. Capabilities exposes the shared mutation window. Focused proofs:
  `storage::definition_receipts::definition_receipts_survive_caller_loss_and_preserve_source_authority`
  and `index_seek::seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart`
  (one passed each). Tasks 3.3/3.4 remain open for the other mutation families.

- Object mirrors now use face-local GetObject/SetObject wrappers and one atomic
  handler. Set returns a typed applied/conflict outcome, opaque revisions and
  original durable receipts; Get returns existence and an issued exact cut.
  The old CompareExchangeObject wire declaration/bindings are removed. Focused
  proof: `storage::generated_native_journey::saved_queries_and_conditional_updates_cross_the_protocol`
  (one passed), including socket/client and plugin-child replay and old-cut reads.
  The schema mirror and common native-list paging are implemented below.

- Storage Append migration: separate client/context AppendRecords, AppendSamples
  and AppendObjectRevisions declarations share bounded unary handlers. Superseded
  streaming declarations, exchange implementation, DTOs and consumers are removed.
  Durable completion includes acknowledged replay-ledger persistence; admitted
  data/receipts survive lost delivery. Capacity retirement refuses removed IDs.
  Focused proofs (one each):
  `storage::operations::writes::records::record_append_receipts`,
  `storage::operations::writes::cancellation::admitted_samples_survive_lost_delivery`,
  `storage::operations::objects::exchange::object_append_saves`,
  `storage::operations::plugins::write_grants_enforced`, and
  `receipts::tests::admission_evicts_completed_receipts_without_losing_active_owners`.
  Log-line Append belongs to its core service owner; task 3.4 stays open for that join.

- Native lists now share PageRequest/optional-at/next-page-token wrappers on both
  faces, with HMAC-bound native continuations and full-envelope byte bounds.
  Column pages preserve short/filtered physical progress; IPC packing and numeric
  integer predicates retain exactness. Index revision checks run inside provider
  scans, and definition/status getters honor issued participating revisions.
  Focused proofs (one each):
  `storage::native_pages::native_lists_continue_filtered_pages_and_refuse_changed_revisions`,
  `storage::generated_native_journey::saved_queries_and_conditional_updates_cross_the_protocol`,
  `storage::definition_receipts::definition_receipts_survive_caller_loss_and_preserve_source_authority`,
  `index_seek::seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart`,
  `query::columnar::tests::numeric_column_bounds_preserve_integer_endpoints`, and
  `saved_preview_survives_atomic_replacement` in the Schematic storage target.
  The example uses face-local messages and its existing runtime operation API;
  coupling adds a protocol dev dependency and the matching lockfile edges.

- Storage policy proof: `manager::policy::configured_policy_owns_io_retries_and_retirement`
  and `storage::callee_policy::a_child_deadline_override_does_not_change_the_next_storage_call`
  pass with controlled time/provider events. Backend defaults and durability are
  preserved; retained-production/receipt capacities are now explicit consumer inputs.

- Core generation/visibility join: merged `43843f60`; storage operation IDs now use
  the descriptor-qualified service/method identity emitted by all SDK generators.
  Explicitly declared context factories suppress duplicate automatic projections
  in native registration. Rust/Python generation and descriptor drift proof pass;
  `storage::unary_transfers::resume_fence_publish_and_range_across_faces` passes
  after the join. The reserved storage validator remains its declaration owner.
  Coupling changes native registration's projection deduplication and removes one
  stale native SDK import; SDK wrapper/resource outputs are generator-owned.

- Durable WatchRows now has independent client/context declarations and one shared
  selector handler for records, samples and object revisions. The native follower
  owns opening retention, committed-prefix gating, filtered progress and gaps;
  accepted following remains a distinct native method. Retired FollowRecords,
  FollowStream and FollowObjects declarations, DTOs and adapters are removed.
  Focused proofs (one each):
  `manager::durable_watch::durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps`,
  `storage::durable_row_watch::watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss`,
  `storage::operations::record_follow::record_watch_resumes`,
  `storage::operations::stream_follow::sample_watch_preserves_sequence`,
  `storage::operations::foreign_cursor::foreign_watch_cut_refused`,
  `storage::operations::follow_budget::watch_spends_its_declared_lifetime`,
  `storage::operations::objects::follow::object_watch_resumes`,
  `storage::operations::objects::pruned::retired_object_watch_cut_is_refused`,
  plus catalog and call-local-policy regressions. The socket/child journey covers
  a commit during opening, blocked accepted data, progress, explicit resync and a
  fresh-list boundary. External file/directory watches retain their separate join.

- File/directory observation migration: WatchFileBytes, WatchFileProduction and
  WatchDirectoryEntries now have independent per-face declarations with shared
  native handlers. Superseded FollowFile/FollowProduction/WatchDirectory adapters
  and request DTOs are removed. File frames include all nested framing; finite
  byte-count completion and terminal production progress are explicit stream
  responses. Existing offset, observation cancellation and directory boundaries
  are preserved. Focused proofs: the cross-face
  `storage::file_observation::distinct_file_observations_preserve_bounded_frames_and_terminal_progress`,
  captured-file/directory journeys, production reconnect/future-offset refusal,
  retained streaming read and durable-row-watch regression (one passed each).
  All 32 Storage methods now have independent declarations and shared handlers;
  streaming upload/download remain solely for the owner's throughput comparison.

- Unary filesystem metadata migration closes the 32-handler storage face join.
  `filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations`
  passes across socket/child calls, mutation, eviction, explicit leases and a
  manager restart on the same persisted providers. Production status repeats its
  issued metadata in `resume_fence_publish_and_range_across_faces`. Generated
  hash/capture/directory, issued-cut/native-page regressions and native/guest
  Schematic checks pass. The four Protected native wrappers/schema mirror still
  keep inventory/3.3 open. Coupling updates the Schematic hash example/proto,
  generator roots, generated outputs, SDK guest bindings, gRPC hash fixture and
  one guest benchmark operation identifier; no second declaration owner is added.

- Native hooks and schema mirrors close storage 3.3: Protected Info/GetSchema/
  SetSchema/DeleteFile have four native-only wrapper pairs. Atomic SetSchema uses
  the existing schema append hook, preserving the 60-method backend inventory and
  stored schema encoding. Proofs: `schema_registry_roundtrip` (whole values, old
  cuts, stale unchanged values, new-version keys, malformed graphs, lost caller),
  `schema_exchange_records_original_outcomes_and_revisions_across_restart`
  (competing creates, atomic conflicts, original outcomes after restart), and
  `reports_persisted_frontiers` (host access, all four matching plugin requirements
  refused, no client gRPC method). Storage 3.2/3.8 are closed by these and the
  named saved-page, native-list, observation, eviction and restart journeys above;
  the 250 ms lag investigation remains separately open under 1.5/A.6. Global 1.4
  and inventory reconciliation await the combined service owners. Coupling is
  the one-line domain StorageInfo conversion target and generated protocol/SDK
  outputs. DeleteFile reclamation semantics are unchanged; 3.10 remains open.
