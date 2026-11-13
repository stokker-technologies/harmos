# Operation Grammar Tasks

The owner locked the contract on 2026-11-12.
Boundary, managers and nested resources locked by the owner on 2026-11-12
Separation by audience locked by the owner on 2026-11-12
Three visibility levels locked by the owner on 2026-11-12

The owner authorizes this checkout's sole writer to implement all four amendments
of 2026-11-13 across the combined candidate under the existing `grpc-plugins` marker.
Ownership covers protocol, runtime, operations, storage, all SDKs, generators, tooling,
consumers, examples, stories and guides. No other writer is active in this repository.
Commits stay local; no push or specification lifecycle command is authorized.

The task numbers below supersede the earlier inventory: durable lag is 1.5
(formerly 1.2), file transfer is 1.6 (formerly 1.3), storage declarations are 1.4.

The locked source inventory covers library proto/protocol/operations/storage,
runtime managers and projection, all four SDKs and their build generators,
conformance, examples, tests and maintained guides. Exact integration ownership is
recorded above and in Integration Candidate. Three open questions remain at their
owning tasks; per-plugin settings/secrets ownership is closed. Preserve data
encodings and required behavior; no compatibility aliases or second authorities.
Unchecked tasks remain work. Order is dependency order: establish
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
- [x] 1.3 Lock design.md's inventory: protocol 1 service/4 operations/4 gRPC methods/4 request-response pairs; context 6/40/40 descriptor methods (0 standalone)/40 pairs; client 12 (6 nested)/59/59/59 pairs. Retain four native-only storage hooks/pairs separately: 19 services, 107 inventory operations, 103 descriptor methods (63 standalone), 107 requests and 107 responses overall. Verify all 38 shared capabilities have independent context/client declarations and authorization; census shared/domain/imported types separately.
- [ ] 1.4 Replace automatic RunMethod twins with explicit face-filtered projection. Generate the 32 Storage declarations independently on context and client faces from their respective declaration owners, sharing handlers while preserving native-only/grant boundaries. Use nested parent packages, bare part verbs and one proto per service. Move/update protocol behaviour guidance with its face owner; streams remain Watch/plugin carriers or a promised final-result replacement.
- [x] 1.5 Add reusable Nightly integration/durable_lag with small_high_rate, multi_megabyte and mixed backend loads; report acceptance-to-durability p50/p95/maximum, backlog and synchronous commit contribution per load. Amended by the owner on 2026-11-13: measured lag is accepted; no accepted-data snapshot or failing threshold, and no timing assertions.
- [x] 1.5.1 Repeat each fixed load at least five times, attribute cadence/build/write/FULL/publication and ordered blocking, preserve raw contention evidence, repair measured batching/provider costs and record the remaining floor without an accepted-data snapshot. The owner accepted this measured floor on 2026-11-13, closing the lag acceptance guard.
- [x] 1.6 Add Nightly integration/file_transfer for large unary client/plugin uploads and downloads; record bytes, chunk/range configuration and throughput before deleting streaming transfers, without timing assertions or new consumer budgets.

## 2. Library Owners and Generic Services

- [x] 2.1 Integrate already-in-progress plugin-manager work and expose Plugins Create/Get/List/Watch/Set/Restart/Delete with managed/remote entries, verified bundles, persistence, state and failures. Migrate the protocol carriers to harmos.plugin.v1.Instance Get/Invoke/Samples/Stop and preserve activation fencing, disconnect isolation and cooperative settlement; migrate affected library consumers in the same compiling slice.
- [x] 2.2 Implement access.Listener Get/Set, durable access.Pairings Create/List/Delete and live access.Certificates.Create under GrpcManager with complete writable values/revisions. Move generic listener/pairing/certificate orchestration here; verify focused lifecycle, invalid rotation and conflict behavior.
- [x] 2.3 Implement Status.Get/Watch, status.Logs/TraceEvents List/Watch and status.Diagnostics.Get under ServiceManager: host identity, health, availability, bounded log/trace reads/watches and bounded diagnostics archive ranges through the narrow ContentSource join to shared storage ranges, with no second transfer engine.
- [x] 2.4 Add narrow product-contribution registration under ServiceManager for product version/identity and component health/availability; define registration lifetime, aggregation and provider failure handling, and prove focused registration/removal/failure behavior without a product-specific dependency.
- [x] 2.5 Implement generic context-only HostValues.Get and its host resolver registration under OperationManager, plus the plugin catalog's settings-declaration section. The product supplies settings/secrets stores, schemas, UI, archives, credential integration and policy. Verify scoped value delivery/failure behavior; add no library value store or client settings service.
- [x] 2.6 Retain OperationManager for face-local CatalogEntries/Invocations and client Invocations.Invoke, StreamManager for context Samples/publication, existing host-clock/service lifetime for face-local Time, and StateManager/RecordManager for their existing responsibilities; migrate names and declarations without duplicate managers or schedulers.

## 3. Storage Handlers, Native Providers and Transfers

- [x] 3.1 Migrate backend.rs, state recovery and provider/manager consumers through the 66-present/60-target hook mapping while preserving encodings, atomic publication and recovery; verify focused backend/restart tests and contract guidance.
- [x] 3.2 Implement latest-durable resolution without read-triggered flush/wait, issued-position continuation and bounded paging; verify controlled-backend visibility, exact continuation and foreign/expired-position refusal, and update docs/storage.md.
- [x] 3.3 Implement whole-value revision-guarded Get/Set mirrors, complete-candidate validation, atomic compare/exchange, explicit create-if-absent and conflicts naming “get again and retry”. Preserve immutable schemas; large writable parts use nested resource Get/Set and their own revisions.
- [ ] 3.4 Migrate records, samples, log lines and upload bytes to bounded immutable Append and accepted/durable receipts; verify client-owned resending, original receipts, changed payload/expired-key refusal and immutable prefixes, including identical-byte offset replay and different-byte conflicts regardless of request identity.
- [x] 3.5 Reuse StorageManager's production owner for CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction and DeleteFileProduction; verify shared client/plugin resume, fencing, integrity, grants, publication and abandonment. Unsupported providers refuse explicitly.
- [x] 3.6 Implement bounded unary Storage.GetFileBytes with offset, maximum bytes, total length, digest, optional at and resolved_at; preserve WatchFileBytes replacement/truncation semantics and distinct directory/production watches. Migrate affected query/transfer consumers and examples in compiling slices.
- [x] 3.7 Bound admissions and commits in bytes, splitting logical input before admission; prove committed byte counts and durable-prefix recovery with deterministic controlled-backend tests. Repeated stage measurements supersede the costly 1 MiB per-commit split; the 1 MiB early-flush trigger and existing atomic admission ceiling remain unchanged.
- [x] 3.8 Settle issued-position/page-token lifetime, provenance, composite retention and restart behavior at this task, record it in design.md, and implement explicit expiry/unavailability without arbitrary earlier reads.
- [x] 3.9 Settle exact Watch opening/events/progress/gaps/resynchronization, record it in design.md, and implement one WatchRows selector for records/samples/object revisions with deterministic opening-race/filter-progress/gap proof. Keep accepted facts distinct from durable facts.
- [ ] 3.10 Settle retained-reader lifetime/reclamation before DeleteFile changes, record it in design.md and prove supported backends without implicit data loss.
- [ ] 3.11 Enumerate cooperative final-result promises, record them in design.md and project one replacement method per actual promise. Preserve running-only Cancel, queued refusal naming Cancel, distinct Stop and typed settlement.

## 4. Four-Language SDK Roots and Library Consumers

- [x] 4.1 Produce a common descriptor table with owner/face/version, parent/service/method, wrappers, cardinality and exposure; generate one complete SDK for Rust, Python, C# and C++, each containing plugin authoring and descriptor-derived client roots. Compose library/product tables so Plugins and plugins.Settings share a node, preserve qualified message identity and reject actual leaf collisions. Prove generated paths for client.plugins.settings.get, client.jobs.progress.watch and client.jobs.start with no hand-written per-operation layer.
- [x] 4.2 Migrate retained SDK/carrier contracts, service-qualified wrappers, fingerprints and policy alongside each declaration slice; preserve standalone protocol/SDK/conformance ownership and product SDK composition. Verify focused catalog, admission, cross-plugin Public invocation, Protected refusal naming the action and local-only Private execution, client Public invocation, inherited scope, child cancellation, clock, failure and call-local policy behavior; deny client context/protocol calls and plugin client-only calls. Preserve explicit catalog visibility and one host registry.
- [x] 4.3 Settle the semantic plugin-parity promise across Rust, Python, C# and C++ and record it in design.md; four-language client-root naming is already locked.
- [x] 4.4 Settle generic managed-client invocation coverage (Rust-only or more languages) and record it in design.md without reopening the naming hierarchy.
- [x] 4.5 Require explicit Private, Protected or Public visibility in all four SDKs with no default and reject omitted/unspecified visibility at admission, add C# invocation-local resources/reason-bearing cancellation and correct the stale draft proto header; verify focused behavior and affected guides.
- [x] 4.6 Migrate tools/conformance/, fixtures, native/guest examples, generated clients and affected tests in the owning compiling slices; preserve interval/device publication and update README/topic/language-guide claims with each user-facing change.

## Architecture Guards

These are future deterministic behavior/structure guards for the named invariants
in docs/architecture.md, not tests of that document's wording. Partial existing
proof does not close them; no checkbox is completed by this documentation task.

- [ ] A.1 Add **Three Audience Faces** descriptor and runtime admission cases for each SDK, rejecting cross-face calls independently of available bindings (task 1.1).
- [ ] A.2 Add **Explicit Streaming and Cooperative Control** stories proving no generated twins, the single declared final-result replacement, running cancellation/child settlement, distinct Stop and preservation of queued action refusals (tasks 1.4, 3.11).
- [x] A.3 Add **Manager Ownership** lifecycle stories for entry activation/retirement, listener rotation, product-fact registration/removal and resolver failure/cleanup, showing each service uses its one manager (tasks 2.1–2.6).
- [x] A.4 Add **Library and Product Composition** independent-host stories composing library services/resolvers without redeclarations, duplicate hosting or product settings/secrets stores in the library (tasks 2.1–2.5).
- [ ] A.5 Add **Resource Operation Grammar** descriptor and four-language generated-call stories for bare verbs, nested packages, service-qualified wrappers, stale whole-value Set and immutable Append (tasks 1.2–1.4, 3.3–3.4, 4.1).
- [x] A.6 Add **Position-Resolved Reads** controlled-backend stories for optional at, immediate latest-durable selection, resolved_at in every reply, issued-position paging and durable watch boundaries; keep lag evidence in Nightly benchmarks, not timing assertions (tasks 1.5, 3.2, 3.8–3.9).
- [ ] A.7 Add **Stable Receipts After Lost Replies** client stories across the migrated mutation surface proving same normalized request returns its original receipt, changed payload conflicts, expiry refuses and admitted completion survives caller loss (task 3.4).
- [x] A.8 Add **Callee-Owned Policy and Consumer Budgets** controlled-clock/backend stories for call-local overrides and configured backend I/O, retries and session lifetimes; migrate hardcoded operational policy in crates/harmos-storage/src/config.rs to its owning policy/configuration without weakening limits. Preserve consumer admission/concurrency/response sizing and replay-safety checks.
- [x] A.9 Add a deterministic production dependency-graph check for **Crate Dependency Direction**, rejecting upward runtime/product dependencies from lower library crates or SDKs while permitting declared derive/build/dev roles.
- [x] A.10 Add **Generic Library SDKs** independent-host conformance and generated-root cases in all four languages using only shared generic concepts; do not close unresolved semantic parity/client-coverage questions by assumption (tasks 4.1, 4.3–4.4).
- [ ] A.11 Audit **Deterministic Stories** repository-wide: replace timing/load/sampled-memory assertions with controlled-clock/event journeys, move measurements to benchmarks, and remove prose/constant/tautological assertions. Add any missing deterministic deadline/cancellation stories; never test authored wording.

**Three-Level Visibility** and the combined **Host-Mediated Public Calls** guard
are deliberately first in tasks 0.1–0.8, including all four SDK-local dispatches
and the complete built-in mapping.


### Owned Guards and Integration Boundary

- [x] 0.8.2 Apply storage's Protected native-only hook mapping and independently Public/grant-checked client/context declarations; prove host access, matching plugin requirement refusal and absent client gRPC projection.
- [x] 1.4.1 Bind all 32 independently owned Storage wrappers on each face to shared handlers; retain four Protected native-only wrapper pairs separately. Global census and non-storage owners remain with their combined tasks.
- [x] 3.4.1 Complete storage AppendRecords/AppendSamples/AppendObjectRevisions/upload-byte receipts and immutable-prefix replay. Log-line Append remains the core service owner's join under 3.4.

- [x] 0.8.1 Migrate native reserved lifecycle/control declarations to Protected and context/client built-ins to their decided faces; prove the built-in refusal and owner access. Storage's mapping is proved separately in 0.8.2.
- [x] A.1.1 Guard non-storage face descriptors, separate wrappers, client listener admission and plugin client-face refusal. All four SDKs include authoring and client bindings; host runtime admission is the guard. The complete descriptor census remains 1.3 after the other service owners merge.
- [x] A.2.1 Guard explicit non-storage streams and managed replacement, queued product Cancel refusal/cause, separate safe-boundary Stop, peer cancellation and local child cleanup. Proof: audience_faces, generated_sdk_client_invokes_streams_and_settles_cancellation, client_preserves_queued_cancel_refusal_and_stop_waits_for_the_product_boundary, peer_journey and conformance 25–28.
- [x] A.5.1 Prove face-local wrappers and four-language library/product root composition, including Plugins with plugins.Settings and Jobs with jobs.Progress. Storage whole-value Set/Append and positions are proved in the storage table below.
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
- [ ] 5.2 Complete the remaining non-transfer cleanup of superseded methods/policies/generators/fixtures/adapters and guidance after every consumer migrates. Retain Watch, plugin carriers and promised final-result replacements; preserve mandatory hooks and existing data.
- [x] 5.2.1 Amended by the owner on 2026-11-13: delete streaming file-transfer handlers, proto requests, bindings, SDK surfaces, streaming-only stories and benchmark cases after the measured comparison; retain unary transfers with the storage-owned 4 MiB default.
- [ ] 5.3 Review affected README, docs/product-guide.md, docs/storage.md, language guides, generators and capability contracts with each owning source change; verify no stale boundary or compound-resource naming claims.
- [ ] 5.4 Verify every locked implementation item is closed in the combined candidate, all writers have handed off, and focused tests plus owned format/language checks pass before declaring source closure. Source closure is not full validation or delivery.

## 6. Final Validation

- [ ] 6.1 After implementation source closure, run strict OpenSpec validation and required combined repository/language checks; repair and report actual failures.
- [ ] 6.2 Under later delivery authorization, prove the combined library through configured hosted CI/Nightly on native Windows and Linux, including SDK/conformance, restart, cancellation, paging, unary transfers and both benchmarks; report exact revisions/results/pending verification.
- [ ] 6.3 Reconcile library services (including nested), operations, gRPC methods, method wrappers and full descriptor payload counts with the locked inventory; verify owner acceptance and leave unperformed proof pending.

## 7. Wire Identity Admission Repair

- [x] 7.1 Restore protocol-owned wire identity validation independently from native dotted names; retain descriptor/fingerprint, visibility, audience and template-provider checks. Own `crates/harmos-protocol/src/schema.rs`, operations protobuf/gRPC contracts, factory registration and assignments, runtime gRPC/guest admission and dispatch, and Rust SDK catalog/client/guest validation.
- [x] 7.2 Prove numeric SDK catalog admission/invocation, canonical Protected host lifecycle access and peer/client refusal, Public peer/client invocation, application face declarations and malformed/reserved refusals. Own runtime plugin stories, operations contract/assignment tests, the shared computation fixture's replay declaration, `sdk/rust/src/bin/invoke-fixture.rs`, all four conformance fixtures and `tools/conformance/`. Preserve exact SDK lifecycle metadata roundtrip and forgery refusal.
- [x] 7.3 Update the identity design and architecture contracts; review README impact. Run format, Rust/Python/repository checks, protocol drift, named stories, four-SDK conformance, manager/access/status, guest System, Tests build and offline locked metadata on the clean committed head; commit locally without pushing.

## 8. Plugin Manager

- [x] 8.1 Record the owner-authorized manager contract, public shapes, ownership, decisions and requirement scenarios before implementation.
- [x] 8.2 Add `Plugins` proto, generated bindings, seven operation files and descriptor-generated SDK root bindings; join shared primitives from the core branch without duplicating them.
- [x] 8.3 Implement the bounded persistent manager registry with conditional objects, whole-value revision guards, replay-safe Create, issued read cuts, complete bounded-set List and explicit Watch resynchronization.
- [x] 8.4 Reuse verified bundle reading and shared published-content uploads; install only verified native executables under the manager root, with no process started on invalid content. Join unary upload sessions from the storage worker.
- [x] 8.5 Compose existing attachments, reconnect and process supervision; expose process identity, hidden Windows startup, unavailable failures, enable/disable, activation replacement, deletion and host restoration.
- [x] 8.6 Add one journey per file: `client_manages_remote_plugins`, `client_manages_bundled_plugins`, `host_restores_plugin_entries`, and `client_refuses_invalid_plugin_bundles`; use the controlled clock, awaited events and existing reference plugins.
- [x] 8.7 Document host adoption, product-owned policy and removal of duplicated management/hosting code; update affected README and guide links.
- [x] 8.8 Run format, Rust/Python/repository checks, all four stories twice, existing plugin-hosting tests, four-SDK conformance and Tests build; repair findings and record only actual proof.
- [x] 8.9 Commit building owned changes locally with Conventional Commits; do not push or run specification lifecycle commands.

## Workflow Follow-Up

- Manager stories upload through unary client file sessions; persistence uses `get_object` and published bundles use bounded `get_file_bytes`. Diagnostics and file snapshots share the storage ranged-content implementation. No transfer join remains deferred; unfinished storage migration tasks remain with their owner.

- The manager implementation and local commits are authorized. Push, review creation, publication and specification lifecycle commands remain outside this task's authorization.
- Keep final validation unchecked until the combined implementation proof is actually performed; documentation validation alone does not close it.

## Integration Candidate

- [x] I.1 Merge the committed core, plugin-manager, access/status and storage branches in order; retain complete SDK generation, runtime face/visibility checks and mechanical roots, regenerate every affected output and prove touched packages.
- [x] I.2 Join manager bundle stories to unary storage upload sessions and diagnostics to the storage ranged reader; record unavailable committed dependencies precisely.
- [x] I.3 Run combined format, Rust/Python/repository checks, protocol drift, four SDK conformance, named runtime/guest/storage stories and the Tests producer; repair failures without changing limits.
- [x] I.3.1 Complete format, Rust/Python/repository checks, protocol drift, four-SDK conformance, manager/access/status journeys, existing hosting/listener/certificate stories, guest System selection, storage unit/integration proof and the Tests producer. Manager/access/status journeys passed repeated runs; the earlier catalog-entry fixture failure is resolved in I.3.2.
- [x] I.3.2 Resolve external_authority_parity by listing the complete catalog once, with no paging loop or application response byte bound (amended by the owner on 2026-11-13).
- [x] I.4 Commit the candidate locally, verify clean status and locked offline metadata; document consumer adoption and pending proof without pushing.

## Access and Status Implementation

- [x] AS.1 Record access/status proto and Rust shapes, decisions, scenarios and exact ownership before implementation.
- [x] AS.2 Add client contracts and one operation per file; regenerate protocol and prove focused builds.
- [x] AS.3 Implement gRPC-owned listener Get/Set, durable pairings, credential-store atomicity and live certificate rotation.
- [x] AS.4 Implement service-owned identity/health, bounded redacted event histories/watches and ranged diagnostics with contributions.
- [x] AS.5 Add five deterministic client journeys, descriptor-generated client bindings and adoption guidance.
- [x] AS.6 Run required format, Rust/Python/repository checks, stories twice, existing listener/certificate tests and Tests build; repair findings.

### Storage Convergence

- [x] I.5 Merge `feat/gp-grammar-storage` through `8babe0a5` into the existing integration branch; retain storage behavior, stories, benchmarks and measurement records, core wire identity validation and descriptor-derived complete SDK generators. Resolve source contracts and regenerate outputs rather than merging generated bytes.
- [x] I.6 Join runtime registration in `crates/harmos-runtime/src/operations/manager/registration.rs`: one typed set of explicit operation identities suppresses inferred context projections, retaining storage's independently authorized context handlers without duplicate registration.
- [x] I.7 Reconcile the combined descriptor census with the locked inventory: 19 services, 103 wire methods, 206 face wrappers and eight native hook wrappers; enumerate shared/domain/imported payloads separately in design.md.
- [ ] I.8 Complete log-line Append under 3.4. `proto/harmos/v1/status/logs.proto` exposes List/Watch; `crates/harmos-runtime/src/services/status/mod.rs::Status::record_log` accepts one in-memory redacted event with no request identity or durable receipt. A bounded immutable Append needs a declared owner/face, mutation identity retention and persisted original outcomes plus lost-reply/restart proof. Adding a method or forwarding to `record_log` would not supply those semantics; this is beyond a mechanical merge join.
  - 2026-11-13 method-only compatibility review: no durable sink is supplied by StatusSettings or configure_status; standalone Status hosts have no required StorageManager. Supplying durable storage/recovery requires an additional consumer configuration change. Preserve the pinned shapes and leave Append unimplemented; the precise boundary and smallest complete follow-up are recorded under design.md's Open Questions.

The storage declaration, paging, Watch, policy and native schema migrations are now
joined. Retained-reader DeleteFile reclamation (3.10), the log Append above,
cooperative final-result inventory (3.11), and whole-change source closure remain
open. Streaming ReadFile/WriteFile were deleted after the owner's throughput review on 2026-11-13.
No accepted-data snapshot is built; A.6 is closed by the owner's accepted measured lag on 2026-11-13 and the controlled-backend stories below.

## Storage Ownership and Proof

Storage commits remain local on `feat/gp-grammar-storage`; no push or specification
lifecycle commands. Owned source is `proto/harmos/**/storage*.proto`,
`crates/harmos-storage/**`, `crates/harmos-operations/src/storage/**`, storage
runtime handlers/tests/benches, benchmark tooling/Nightly wiring and storage guides.
Core generator integration is merged through `43843f60`; there is one declaration
owner per face and one shared native handler per capability. Both faces declare
32 Storage method pairs, with four Protected native-only wrapper pairs separately.
ReadFile/WriteFile, their requests, handlers, bindings and streaming-only stories
were deleted by the owner's 2026-11-13 amendment after the comparison.

| Storage Item | Deterministic Proof |
| --- | --- |
| Resumable upload, fencing, checkpoints, integrity, abandonment, grants and ranged reads (3.5/3.6) | `resume_fence_publish_and_range_across_faces`; `replayed_file_bytes_preserve_the_accepted_prefix` |
| Latest durable reads, issued cuts and retained query progress (3.2) | `issued_pages_keep_their_cut_while_newer_writes_are_blocked`; `filtered_physical_pages_return_progress_without_rescanning_the_prefix`; `native_lists_continue_filtered_pages_and_refuse_changed_revisions` |
| Whole-value definitions/objects/schemas and atomic receipts (3.3) | `definition_receipts_survive_caller_loss_and_preserve_source_authority`; `saved_queries_and_conditional_updates_cross_the_protocol`; `schema_registry_roundtrip`; `schema_exchange_records_original_outcomes_and_revisions_across_restart` |
| Storage immutable Append and original receipts (3.4.1) | `record_append_receipts`; `admitted_samples_survive_lost_delivery`; `object_append_saves`; `mutation_receipts_survive_replays_lost_responses_and_cancellation` |
| Byte bounds, ordered prefixes and native physical chunks (3.7) | `large_batches_preserve_committed_prefixes`; `small_prefixes_publish_before_later_large_admissions`; `coalesced_admissions_keep_physical_chunks_bounded_and_commit_atomically`; `oversized_state_capture_resumes_after_each_bounded_committed_piece` |
| Provenance, leases, eviction and restart (3.8) | `cached_cuts_expire_while_explicit_leases_pin_their_image`; `filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations`; native schema/index/projection/file restart journeys |
| Durable rows and distinct external observations (3.9/3.6) | `durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps`; `watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss`; `distinct_file_observations_preserve_bounded_frames_and_terminal_progress` |
| Protected native hooks and consumer policy (0.8.2/A.8) | `reports_persisted_frontiers`; `configured_policy_owns_io_retries_and_retirement`; `a_child_deadline_override_does_not_change_the_next_storage_call` |
| Authoritative declarations and bindings (1.4.1) | `generated_code_matches_proto`; generated Schematic hash/capture/directory/storage journeys |

Stage measurements and retained raw benchmark evidence belong to 1.5/1.6 in the
design. Timing remains measurement, never a test assertion. No accepted-payload
snapshot, budget increase, weaker synchronization or deadline change is introduced.
Task A.6 is closed: the owner accepted measured p95 values on 2026-11-13. Log-line Append is the
core service owner's join under 3.4; full service census and combined source closure
remain with the integration owner. DeleteFile reclamation (3.10) is unchanged.

Couplings outside the storage paths are the source-matched generator roots and
outputs, generated SDK resources, Schematic hash/proto/preview consumers, the gRPC
hash fixture and guest benchmark identifier, the domain StorageInfo conversion,
and runtime registration's explicit-context projection deduplication. There are
no compatibility aliases or second declaration owners. README claims remain valid;
affected usage is maintained in the product/storage guides.


Final local proof: storage library 51/51 and integration 127/127; runtime's storage
story prefix 178/178, plus the full-candidate schema-replay regression and native
restart proof after its fingerprint repair. Format, Rust/Python/repository checks
and the complete Tests artifact producer pass. Python SDK verification selects the
freshly task-built conformance host through its supported HARMOS_CONFORMANCE_HOST
override because an older exported host was present; all 29 protocol rules pass.
Native storage, file-transfer (seven available carrier/direction cases, three runs)
and durable-lag (three loads, five final runs each) benchmarks complete. This is
local Windows evidence, not a full test lane or hosted/Linux validation. Measured lag is accepted by the owner's 2026-11-13 amendment; no threshold fails
Nightly and A.6 is closed.

## Owner Amendments of 2026-11-13

- [x] O.1 Replace paging for bounded catalogs, plugin entries and pairings in proto, operations, runtime, generated SDKs, consumers, examples, stories and guides; preserve paging and issued positions for unbounded data. Record the bounded/unbounded inventory and scenarios.
- [x] O.2 Ship one SDK package per language; delete audience build selection, generated-stub guards and their tests/tooling; add four copyable self-install client journeys and migrate package consumers/guides. Preserve runtime face/visibility enforcement.
- [x] O.3 Keep unary transfers with one storage-owned 4 MiB chunk default and the owner's later separate 8 MiB message default; derive the minimum ceiling including framing, refuse incompatible startup settings, delete streaming transfer declarations/handlers/SDKs/stories and benchmark cases, record and rerun the default benchmark.
- [x] O.4 Accept measured durable lag without a failing threshold or accepted-data snapshot; replace superseded requirements/guides and close the lag acceptance tasks.
- [x] O.5 Complete final proof and local commits; keep every decision in a separate local building commit and do not push.
- [x] O.5.1 Run format, Rust/Python/repository checks, protocol drift, four-SDK conformance, the named self-install/storage/manager/access/status/wire identity stories, guest System selections and the final Tests artifact; repair findings. Expose the existing focused fixture selection through the public build task.
- [x] O.5.2 Verify locked offline Cargo metadata on the committed clean head with Cargo.lock unchanged.
- [ ] Plugin settings owned by the library, decided by the owner on 2026-11-13, replacing the earlier boundary note for plugin settings: a plugin instance has settings and the library stores them beside its plugin entries. A plugin declares each setting with name, title, description and a typed default; values are typed (`SettingValue`: flag, integer, number, text, duration), never a type name with opaque bytes. Client face `Plugins.Settings.List` (declarations, current values, revision) and `Plugins.Settings.Set` (whole value with expected revision); context face `Settings.List`, which returns only the calling plugin's values. Remove `harmos.context.v1.HostValues`, its resolver registration and its stories. Secrets stay outside the library: a product owns their store and its own context read. A change notification for running plugins is not part of this; add it only for a demonstrated case.
  - [x] D.1 Definition: Declare typed settings, SettingsDeclarations, client Plugins.Settings List/Set and caller-scoped plugin-host Settings.List; remove the HostValues wire service. Consumer persistence, authorization and tests remain open.
- [ ] Product resources under `Plugins`, decided by the owner on 2026-11-13: the plugin entry is the only plugin record; a product attaches its own per-plugin data as nested resources it owns under the library's `Plugins` parent (for example a product's `plugins.Secrets`), composed into one generated client tree, and declares plugin needs in product-defined catalog sections. The library never stores or interprets that data. Add the client fixture case for an application resource under a library parent, in all four SDK languages.
  - [x] D.2 Definition: Preserve the Plugins parent and nested package composition contract; record product-resource fixture work in docs/architecture.md. No product-owned resource is declared by the library.
- [ ] Caller identity for product context operations, decided by the owner on 2026-11-13: publish the calling plugin (entry name and catalog) to product context handlers as a public resource, replacing the crate-private caller that only the removed HostValues resolver exposed, so a product can refuse a read the calling plugin did not declare.
  - [x] D.3 Definition: Keep plugin-host Settings.List free of a plugin selector; record trusted caller-resource implementation for the runtime worker. No new caller identity wire payload is introduced.
- [ ] In-place replacement of a plugin's bundle, decided by the owner on 2026-11-13: an upgrade replaces the placement of an existing entry, keeping its name, settings and product data; the entry name is the plugin's own identity. A failed replacement leaves the previous bundle serving. Delete plus Create remains removal and installation, not upgrade.
  - [x] D.4 Definition: Declare Plugins.Replace with name, managed placement, expected revision, request identity, entry and receipt; preserve existing Plugins calls.
- [ ] Deletion contract, decided by the owner on 2026-11-13: no new hook. Deleting an entry removes its library-stored settings in the same commit and publishes the deletion on the watch; a product removes its own per-plugin data from the in-process watch and reconciles at startup against the listed entries. State in the guide that product cleanup is eventual.
  - [x] D.5 Definition: Retain PluginChange.deleted unchanged and document atomic settings deletion plus eventual product cleanup in the definition migration contract. Runtime behavior and guide migration remain open.
- [ ] Protocol layout by boundary, LOCKED by the owner on 2026-11-13 (further refinement is expected iteratively): the host has two boundaries and each has a package that says who uses it. `harmos.v1` is shared vocabulary only, messages without services (`ReadCut`, `Receipt`, `Failure`, `FileReference`, `SettingValue`, `SettingDeclaration`). `harmos.client.v1` is what a client program calls on the host (today's `harmos.v1` services). `harmos.plugin.v1` is what a plugin implements (`Instance`). `harmos.plugin.host.v1` is what a plugin may call on the host (today's `harmos.context.v1`). Folders follow the packages: `proto/harmos/v1/`, `client/v1/`, `plugin/v1/`, `plugin/host/v1/`. The file path equals the call path, one resource per file (`client/v1/plugins/settings.proto` holds `Plugins.Settings`). `types.proto` is dissolved: each message lives with the concept that owns it or in a small shared file. The generated client tree is unchanged (`client.operations.list`); the SDK handler object keeps the name context. Update the generator's face rule, runtime admission, the four SDKs and the guides, and state the consumer product's change for its next pin.
  - [x] D.6 Definition: Move services by boundary, dissolve shared aggregate files, update generator face/path rules, regenerate bindings and publish the complete consumer mapping. Hand-written consumers remain open by owner authorization.
- [ ] One home per atomic operation, LOCKED by the owner on 2026-11-13: every capability has exactly one wire declaration. Where a product must shape a capability (scope, attribution, typed replies, policy), which is all stored data, the product declares the wire operation and its handler is a call to the library's in-process operation; the library declares no wire twin. Where the library's shape is complete as it is (plugin management, access, status, invocations, time, plugin settings), the library declares it and a product adds no wrapper. Consequently remove `Samples.Append` from the plugin-host boundary: a plugin answers the host's ticks on `Instance.Samples`, the single library home for samples, and a product that wants a plugin to publish a row or a record declares its own operation over the in-process append. Verify before removal that no library story or the schematic example needs a pushed sample the tick model cannot express, and report it if one does. Ship the shared file and upload messages and a small handler helper so a product's pass-through wrapper is a declaration and one call.
  - [x] D.7 Definition: Remove Samples.Append and its wire pair, retain Instance.Samples, and report the guest publication fixture dependency and native helper work for the operations/runtime worker.
- [ ] Storage is in-process only, LOCKED by the owner on 2026-11-13, replacing the earlier entry that made files a library wire resource: no storage operation is exposed on gRPC, on either boundary. A product defines its storage backend, registers it with the library when assembling the runtime, and reaches it only through the library's storage operations, which have private visibility. Everything a client or a plugin may do with stored data is a product operation that wraps a library storage operation, with the product's own typed messages and policy: file transfer, queries, record and object access alike. Remove both wire `Storage` services, their duplicated handlers and their generated SDK code; keep the native operations and their stories. The file-transfer shape agreed earlier (bare verbs, `File` and `Upload`, Create, Get, Resume, Append, Publish, Delete, Read, Watch) is the recommended shape for a product's wrappers, declared by the product. `Plugins.Create` keeps taking a file reference for a managed bundle; the bundle is uploaded through the product's own upload operation, and the library resolves the reference in-process. The schematic example gains the wrappers it needs. The registry of storage object definitions is not listed on the wire by the library either.
  - [x] D.8 Definition: Remove both Storage services and all storage wire request/result declarations; publish shared File/Upload vocabulary and the complete native type handoff. Native implementation migration remains open.
- [ ] No `Catalog` on the wire, decided by the owner on 2026-11-13: catalog is how the code organizes its registries, not a resource a caller names. Replace `CatalogEntries.List` and its `CatalogKind` switch on the client and context faces by one typed resource per registry, each in its own file: `Operations.List` (and `Operations.Get` for one contract), `Schemas.List`, `Streams.List`, and the storage object definitions under the storage resource (`Storage.Objects.List`) unless reading the handlers shows they stand alone. Remove the opaque `bytes entries` reply field; every reply is typed. Migrate the runtime, the four SDK roots and the consumers, and state the consumer product's change for its next pin.
  - [x] D.9 Definition: Replace the kind switch and opaque registry reply with typed Operations List/Get, client Schemas.List and Streams.List; preserve the reported Catalog description and remove object listing from the wire.


## SDK and Application Content Completion

- [x] C.1 Declare one generated 8 MiB message default, retain the 4 MiB chunk and explicit ceiling validation; migrate protocol, runtime listener/carriers/guest, four SDKs, transfer guide and requirement delta. Proof: `resume_fence_publish_and_range_across_faces`, Rust/Python SDK `managed_plugin_passes_every_rule` / `test_remote_plugin_passes_every_rule` including the large-message case, Wasm SDK check and C# build.
- [x] C.2 Ship the importable conformance plugin and its generated bindings in each published SDK; move definitions out of executable entry points and migrate callers/guides. Isolated package proofs pass: `test_installed_wheel_serves_the_public_fixture`, `test_published_crates_serve_the_public_fixture`, `test_nuget_package_serves_the_public_fixture` and `test_source_package_serves_the_public_fixture`, each through all 31 reference-host rules.
- [x] C.3 Generate caller-owned per-call context for every C++ root method and Watch; audit Rust/Python/C# equivalents and migrate generated consumers. The four SDK journeys `one_call_preserves_metadata_deadline_override_and_cancellation` / `test_one_call_preserves_metadata_deadline_override_and_cancellation` pass; the C# journey runs in its six-test SDK suite.
- [x] C.4 Add one storage-owned content-source/expiring-lease contract, scope-preserving application registration and shared ranged reads; migrate diagnostics and bundle readers without duplicate persistence ownership. Focused proofs pass: `storage::application_content::client_and_plugin_read_application_content_under_expiring_scoped_leases` and `failure::tests::application_failure_survives_origin_and_carrier_roundtrip`. The former covers both faces, digest corruption, scope/refusal, expiry and unchanged saved-file reads.
- [x] C.5 Run format, Rust/Python/repository checks, protocol/SDK drift, four-SDK conformance, named new/storage/manager/access/status/wire stories, guest System and Tests build; repair findings. All pass: 31 protocol cases per SDK; Python tooling 59 and SDK 5; C# 6; C++ 5 native tests and 2 SDK journeys; Rust caller-control/conformance/drift journeys; runtime application-content 1, unary-transfer 1, plugin-manager 8, access 2, status 3 and wire-identity 1; guest contracts 6 and composition 1. These are focused Windows proofs, not a full lane or Linux/hosted validation. Python selects the freshly built host with HARMOS_CONFORMANCE_HOST.
- [x] C.5.1 Both `cargo metadata --locked --offline` and `cargo metadata --offline` pass on the clean committed tree with Cargo.lock unchanged. No push or lifecycle commands were run.

Owned inventory for C.1–C.4: `proto/`, `crates/harmos-protocol`, storage content/manager/read
contracts, operations Storage.GetFileBytes, runtime storage/diagnostics/plugin content and
their stories, `sdk/{rust,python,csharp,cpp}`, descriptor/root and package generators under
`tools/`, conformance fixtures/runner, affected examples, README and topic guides, and this
change's design/tasks/requirement deltas. One writer owns this whole inventory. Dependency
order is C.1, C.2, C.3, C.4, followed by C.5; each part closes with its named focused proof.
