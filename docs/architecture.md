# Architecture

## How to Use This Document

Read this contract before proposing architecture, protocol, public API or ownership
changes. A proposal touching an invariant must quote it, explain the purpose of the
mechanism it changes, and update this document in the same delivery. Never remove,
merge or replace a mechanism until its purpose is established from its handler,
tests and design, or from the owner.

**landed** means verified in this tree, not newly executed proof. **locked direction**
names an agreed change whose complete contract has not landed. Partial proofs below
do not prove a future contract. All missing guards have unchecked tasks in
`openspec/changes/operation-grammar/tasks.md`; its first task covers visibility and
both directions of plugin/host invocation. A face is never a substitute for visibility.

Paging was amended by the owner on 2026-11-13: only data rows that can grow
without bound are paged. CatalogEntries on both faces, Plugins and Access.Pairings
return the complete set without page requests, tokens or application byte bounds.
The carrier message ceiling is their sole response guard; overflow names the action.
Records, samples, log lines, trace events, files and rows retain position-resolved paging.

## Invariants

Wire operation admission uses the protocol-owned validator in
`crates/harmos-protocol/src/schema.rs`, independently of native dotted identities.
The [wire identity contract](../openspec/changes/operation-grammar/design.md#wire-operation-identity-admission)
covers numeric template bindings and application-owned face packages. Catalog admission,
guest admission and host protobuf declarations share it; visibility, descriptor fingerprints
and caller-face authorization remain independent checks.

1. **Three Audience Faces.** The plugin protocol is served by one running plugin
   instance and called only by its host; context is served by the host to a plugin
   during an invocation; the client API is served by the host to clients. The host
   is always between a plugin and anyone else; local Private calls stay in-process.
   **Why:** isolate audience contracts without losing cross-plugin invocation.
   **Where:** `openspec/changes/operation-grammar/design.md`, Audience Faces and
   Face-Local Contracts; requirement: Audience-Separated Service Packages in
   `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   **Proof:** `service_packages_separate_audiences_and_only_declared_methods_stream`
   checks face-local service and wrapper descriptors. SDK audience generation probes
   reject leaked stubs. `plugin_cannot_call_a_client_face_even_with_its_declared_contract`
   and `client_invokes_public_plugin_through_the_host_and_protected_is_refused`
   guard native admission, including absence of Instance/context on the client listener.
   Storage and management descriptors use the same audience-filtered generator;
   the combined descriptor inventory matches the locked 19 services/103 methods.
   **Status:** landed for protocol/SDK declarations and storage handlers; broader guards remain open.

2. **Admission Boundaries.** The host refuses catalogued Private declarations and
   unspecified visibility. Protected admits the declaring owner and host; Public
   admits declared peers and authorized clients. Visibility is checked before
   provider selection branches, including host-provided built-ins.
   **Why:** matching requirements and fingerprints do not grant host visibility.
   **Where:** `crates/harmos-runtime/src/operations/manager/plugin_grpc.rs`,
   `crates/harmos-runtime/src/operations/manager/entry.rs` and
   `crates/harmos-runtime/src/plugin/grpc/catalog.rs`.
   **Proof:** `protected_host_operation_refuses_a_plugin_with_a_matching_requirement`
   and `protected_peer_operation_is_refused_across_plugins` are the focused guards.
   **Status:** landed with focused native proof.

3. **Three-Level Visibility.** Private is callable only inside the declaring plugin
   process, absent from its catalog, invisible to the host, never on any wire
   (including the host/plugin carrier), without a gRPC method; the SDK runs it
   locally with ordinary policy, retries, deadlines and cancellation. Protected is
   catalogued and callable by the plugin and host, with host calls using only that
   plugin's carrier, never a client-facing gRPC method; other plugins and clients
   are refused; Public admits other plugins by declared requirement and clients
   through the host and may bind a client-facing gRPC method; every SDK requires
   an explicit choice with no default, and host built-ins follow the decided mapping.
   **Why:** the owner's intent is "a private operation never appears on the wire, is consumed only internally by the plugin and does not allow association to a grpc endpoint" and "a plugin should be able to call another operation in another plugin if exposed".
   **Where:** Explicit Three-Level Operation Visibility in
   `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`;
   Decided Built-In Visibility Mapping in `openspec/changes/operation-grammar/design.md`.
   Host-only requests of a plugin must be Protected; Private dispatch stays inside
   the declaring SDK process.
   **Proof:** common conformance case 24 guards local dispatch, catalog omission
   and direct carrier refusal in all four SDKs. Rust uses its local operation map;
   Python, C# and C++ use their registered typed handlers without a child frame.
   Cases 26–28 additionally guard host-clock retry, attempt deadlines, child cleanup,
   isolated attempt resources and reason-bearing cooperative cancellation in all four SDKs.
   Native reserved controls are Protected; the complete storage mapping belongs to
   its declaration owner.
   **Status:** native SDK dispatch and policy implemented; combined built-in census open.

4. **Host-Mediated Public Calls.** A plugin calls a Public peer as plugin → host →
   plugin: the host resolves one registry, requires the declared requirement and
   matching fingerprint and an admitted provider, and carries inherited scope,
   failures and cancellation through the child invocation.
   **Why:** exposure must enable peer composition without bypassing host authority.
   **Where:** `crates/harmos-runtime/src/operations/manager/plugin_grpc.rs`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`; requirement: Host-Mediated
   Plugin Calls in `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** `child_calls_reach_host_and_public_peer_without_start_order` in
   `crates/harmos-runtime/tests/plugin_grpc/child_calls.rs` proves host and peer
   routing plus mismatched/absent providers; scope proof is invariant 7.
   `cancellation_reaches_the_plugin_before_settlement` in
   `crates/harmos-runtime/tests/plugin_grpc/cancellation.rs` proves cooperative
   settlement. `public_peer_inherits_scope_cause_and_caller_cancellation_through_the_host`
   combines peer scope, structured causes and cooperative child settlement;
   common conformance cases 24 and 29 guard local-only Private isolation and peer visibility.
   The client invocation story in invariant 1 covers Public and Protected client calls.
   **Status:** landed for native gRPC plugins and all four SDKs.

5. **Unary Plugin Operations.** A plugin operation has one request and one final
   response per attempt, even though its carrier multiplexes control/child frames;
   samples and observation are separate streams.
   **Why:** transport duplexing must not silently change the operation contract.
   **Where:** `proto/harmos/plugin/v1/instance.proto`, `proto/harmos/v1/behaviour.md`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`; requirement: Unary Plugin
   Attempts in `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** `host_invokes_registered_plugin_operation` in
   `crates/harmos-runtime/tests/plugin_grpc/host_invoke.rs` checks the request/result
   journey; `malformed_payload_fails_only_its_invocation` in
   `crates/harmos-runtime/tests/plugin_grpc/malformed_payload.rs` guards
   malformed unary input.
   **Status:** landed.

6. **Explicit Streaming and Cooperative Control.** Never generate an automatic
   streaming twin beside a method; streams serve samples and Watch or protocol
   carriers. A specifically promised typed final result uses one replacement
   managed method; Cancel interrupts running work, while Stop ends at a safe boundary.
   **Why:** preserve typed cooperative settlement without multiplying interfaces or
   confusing observation loss with work ownership; queued control refusals stay explicit.
   **Where:** No Automatic Streaming Twins and Distinct Stop and Cancel Actions in
   `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
   **Proof:** the face descriptor guard excludes generated twins; the discovered-client
   journey in `crates/harmos-runtime/tests/operations/wire/sdk_client.rs` exercises
   explicit managed replacement and cooperative final results. The product-control
   story preserves a queued Cancel refusal and its cause, then awaits a separate Stop
   boundary without issuing Cancel. Peer and local-policy stories guard child settlement.
   The final storage stream census awaits its declaration owner.
   **Status:** landed for non-storage projection/control; combined inventory open.

7. **Opaque Invocation Scope.** Carry caller-bound scope unchanged through children;
   never interpret it as authorization or require it for operations that do not ask.
   **Why:** applications choose its meaning; a correlation/selection value is not a grant.
   **Where:** `crates/harmos-runtime/src/operations/scope.rs`,
   `crates/harmos-runtime/src/plugin/grpc/invoke.rs`, `sdk/rust/src/context.rs`;
   Opaque Scope and Canonical Identity in
   `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   **Proof:** `bound_scope_reaches_handlers_and_children_and_missing_scope_is_refused`
   in `crates/harmos-runtime/tests/plugin_grpc/scope.rs`; `external_request_grants`
   in `crates/harmos-runtime/tests/plugin_grpc/external_authorization.rs` separately
   proves request-grant enforcement.
   **Status:** landed.

8. **Manager Ownership.** A manager exists when an owner coordinates live resources
   and lifecycle; plain storage operations need none. Service methods delegate
   orchestration to that owner, never a second registry, scheduler or upload engine.
   **Why:** one authority must own creation, cancellation, retirement and partial failure.
   **Where:** Boundary and Manager Ownership in
   `openspec/changes/operation-grammar/design.md`; Manager Ownership and Thin Service
   Methods in `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   OperationManager owns catalog/invocations/context HostValues; StreamManager owns
   publication/demand; StorageManager owns persistence/production; GrpcManager owns
   listeners/access; ServiceManager owns resident services/status and host clock;
   StateManager owns state/replay; RecordManager owns record registration/lifecycle;
   plugin management owns entries, placement, activation and shutdown.
   Existing owners live in `crates/harmos-runtime/src/operations/manager/mod.rs`,
   `crates/harmos-runtime/src/streams/manager.rs`,
   `crates/harmos-runtime/src/storage/manager.rs`,
   `crates/harmos-runtime/src/grpc/manager.rs`, `crates/harmos-runtime/src/services/mod.rs`,
   `crates/harmos-runtime/src/state/manager.rs`, `crates/harmos-runtime/src/records/manager.rs`
   and `crates/harmos-runtime/src/plugin/manager/mod.rs`.
   **Proof:** `client_manages_remote_plugins`, `client_manages_bundled_plugins`,
   `host_restores_plugin_entries` and `client_refuses_invalid_plugin_bundles` guard
   persistent entries and activation ownership. `client_pairs_and_rotates_access`
   and `credential_store_unavailable` guard listener/credential atomicity.
   `client_reads_status`, `client_pages_and_watches_events` and
   `client_reads_diagnostics_ranges` guard contributions, histories and archives.
   HostValues resolver registration/removal and failures have plugin_grpc stories.
   **Status:** landed; native proof is tracked separately from source.

9. **Library and Product Composition.** The library owns generic hosting, access,
   transfers, status, invocations, time, storage, streams, protocol and SDKs; products
   compose those services directly without redeclaring/wrapping them or rebuilding
   generic machinery on the protocol crate alone. Products own their schemas,
   indexes/projections, objects, operations, authorization, latest selection and
   settings/secrets stores, UI, archives and credential integration.
   **Why:** avoid duplicate dispatch, connection, launch and storage authorities;
   settings and secrets are product policy. Generic HostValues delivery and catalog
   settings declarations do not make a library settings store.
   **Where:** Generic Machinery and Product Composition and Product Definitions,
   Policy and Plugin Values in
   `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
   **Proof:** `declared_values_use_the_product_resolver_and_registration_lifetime`
   and `resolver_failure_and_a_mismatched_value_type_are_propagated` guard resolver
   composition without a library store. The manager, access and status client journeys
   compose runtime services with host-supplied persistence, credentials and facts.
   **Status:** landed for native host composition.

10. **Resource Operation Grammar.** Services name resources, methods use the closed
    ordinary verb set Get/List/Watch/Set/Create/Delete/Append; only declared domain
    actions are exceptions. Nested resources use parent sub-packages; wrappers are
    `<Service><Method>Request` and `<Service><Method>Response`; Set replaces the whole
    writable value under its expected revision, while Append adds ordered immutable data.
    **Why:** predictable generated clients must preserve conflict and immutability
    semantics; mixed-resource Storage retains distinguishing resource nouns.
    **Where:** Predictable Public Method Grammar, Nested Wire Packages and Wrappers
    in `openspec/changes/operation-grammar/specs/operation-contract/spec.md`;
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** client-root fixtures in all four SDKs compose library Plugins and
    plugins.Settings with product Jobs and jobs.Progress, preserving unary and Watch
    cardinality and qualified message identity. The complete storage mutation surface
    remains with its declaration owner; invariant 13 guards existing atomic behavior.
    **Status:** locked direction (`operation-grammar`).

11. **Position-Resolved Reads.** One read path takes optional `at`: absent selects
    latest durable data without requesting a flush or waiting for newer durability;
    every reply reports `resolved_at`. Continue only with issued positions/cuts and
    bound page tokens; durable lag is at most 250 ms p95 independent of payload size.
    **Why:** repeatable reads and read-your-write receipts must not hide a persistence
    barrier. The lag target is measured, never asserted against real time in a test.
    **Where:** `openspec/changes/operation-grammar/specs/storage-visibility/spec.md`.
    **Proof:** Issued-page, native-list and cut-lifetime stories in
    `crates/harmos-runtime/tests/storage/` cover durable selection and continuation.
    `watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss` covers
    socket/child durable observation, opening races, filtered progress and resync;
    `durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps`
    in the native storage manager tests covers committed-prefix gating.
    `filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations`
    covers metadata observations, pinned continuation, eviction and restart refusal.
    The measured lag bound remains open in the repeated benchmark investigation.
    **Status:** partially landed; no complete lag-bound proof.

12. **Stable Receipts After Lost Replies.** The caller resends; the same request
    identifier and normalized payload return the original receipt, while a changed
    payload conflicts and expired identities refuse. Admitted completion remains
    owned independently of a retry window; accepted and durable receipts stay distinct.
    **Why:** a lost reply must not duplicate effects, lose an admitted result or
    falsely acknowledge persistence; this does not promise an unlimited replay ledger.
    **Where:** Client-Owned Resending After Failed Calls in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`;
    `openspec/specs/storage-mutations/spec.md`;
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** Storage's unary-transfer, Append-record/sample/object, definition-
    receipt and schema-mirror journeys in `crates/harmos-runtime/tests/storage/`
    cover original receipts, changed requests, admitted work after caller loss and
    retained windows. Native schema/index/projection restart journeys cover atomic
    outcomes and `admitted_create_completes` covers completion through retry expiry.
    **Status:** landed for the migrated storage surface; the complete service census
    and non-storage mutation joins remain under `operation-grammar`.

13. **Distinct Storage Guarantees.** Saved reads pin durable data; follows may join
    accepted data and must not label it saved. Bounded physical scans, ordered index
    seeks and exact-revision/frontier projections are distinct from bulk ingestion
    and atomic conditional object replacement; preserve backend durability guarantees.
    **Why:** one generic read/write label would erase consistency and conflict behavior.
    **Where:** `crates/harmos-storage/src/snapshot.rs`,
    `crates/harmos-storage/src/backend.rs`, `crates/harmos-storage/src/sqlite/mod.rs`,
    `docs/storage.md`; requirement: Distinct Storage Guarantees in
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** `saved_windows_match_fallback_while_live_writes_and_cancellation_advance`
    in `crates/harmos-storage/tests/integration/columnar_snapshot.rs`;
    `seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart` in
    `crates/harmos-storage/tests/integration/index_seek.rs`;
    `projection_frontiers_survive_interrupted_publication_rebuild_and_restart` in
    `crates/harmos-storage/tests/integration/projection_replay.rs`;
    `conditional_objects_keep_conflicts_indexes_and_receipts_atomic_across_restart`
    in `crates/harmos-storage/tests/integration/conditional_objects.rs`.
    SQLite uses WAL with synchronous FULL; changing flush cadence is not permission
    to weaken durability. New durable Watch semantics remain invariant 11's direction.
    **Status:** landed.

14. **Refuse Incompatible Roots.** Incompatible host state definitions or corrupt
    recovery data refuse opening and preserve history; never reset silently.
    Back up before deliberately choosing/resetting a root; the application owns that decision.
    **Why:** an API/schema migration must not become silent data loss.
    **Where:** `crates/harmos-domain/src/state/transaction.rs`,
    `crates/harmos-runtime/src/storage/attachments.rs`, `docs/storage.md`;
    `openspec/specs/plugin-state-history/spec.md` and Safe Root Opening in
    `openspec/changes/operation-grammar/specs/storage-mutations/spec.md`.
    **Proof:** `changed_record_shape_refuses_root_and_preserves_history` in
    `crates/harmos-runtime/tests/storage/state/shape_versions.rs`;
    `corrupt_snapshot_blocks` in
    `crates/harmos-runtime/tests/storage/state/snapshots/corrupt_snapshot_blocks.rs`.
    **Status:** landed; backup/reset guidance is maintained in `docs/storage.md`.

15. **Callee-Owned Policy and Consumer Budgets.** Deadlines, delays and retries come
    from owning operation policies/configuration; callers override one call only.
    Never tune a budget or limit to make proof pass; consumers own storage budgets,
    admission, concurrency and response sizing, while the library owns correctness constants.
    **Why:** operational policy must have one owner and failures must stay visible.
    **Where:** `openspec/specs/operation-invocation/spec.md`,
    `openspec/specs/operation-authoring/spec.md`, `openspec/specs/canonical-build/spec.md`;
    Policy Ownership and Honest Budgets in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** `configured_policy_owns_io_retries_and_retirement` in
    `crates/harmos-storage/tests/integration/manager/policy.rs` proves configured
    I/O/recovery, retries, production capacity/lifetime and receipt expiry with
    controlled time. `a_child_deadline_override_does_not_change_the_next_storage_call`
    in `crates/harmos-runtime/tests/storage/callee_policy.rs` proves a plugin child
    override expires only its call. Existing operation policy stories preserve
    replay-safety admission. `StorageSettings.policy` owns operational defaults;
    request settings own production/receipt capacities and existing budgets.
    **Status:** landed for storage and invocation policy (task A.8).

16. **Crate Dependency Direction.** Runtime composes operations, storage, domain and
    protocol; operations depends on storage/domain/protocol, storage on domain/protocol,
    domain on protocol; the standalone Rust SDK depends on protocol/derive, never
    runtime, and lower crates and SDKs must not depend on runtime or a consuming product.
    **Why:** contracts and plugin/client authoring must work without host machinery;
    only runtime owns sandbox authority. Derive/build/dev edges do not invert production ownership.
    **Where:** `crates/harmos-runtime/Cargo.toml`, `crates/harmos-operations/Cargo.toml`,
    `crates/harmos-storage/Cargo.toml`, `crates/harmos-domain/Cargo.toml`,
    `crates/harmos-protocol/Cargo.toml`, `sdk/rust/Cargo.toml`;
    Library Dependency Direction in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** `test_production_edges_follow_domain_ownership` inspects normal and
    target production dependencies across all six libraries, excluding derive support.
    **Status:** landed.

17. **Generic Library SDKs.** Keep all four language SDKs in this repository, with
    the same concepts named consistently using each language's normal casing;
    product SDKs compose them. Library contracts, conformance and guides use generic
    vocabulary without product objects or private identities.
    **Why:** independent hosts must reuse the same library, not inherit another
    application's model. Common naming does not claim unresolved semantic parity.
    **Where:** `sdk/rust/`, `sdk/python/`, `sdk/csharp/`, `sdk/cpp/`,
    `tools/conformance/`; Self-Sufficient Library SDK Ownership in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** the independent reference host runs the shared conformance suite in
    each SDK. Four-language client-root fixtures compose library and product descriptors.
    Native plugin semantic parity is agreed; generic managed application clients remain
    Rust-only, while other languages use their generated application gRPC clients.
    **Status:** landed for these SDK contracts; Wasm is Rust-only.

18. **Deterministic Stories.** Correctness tests follow journeys using controlled
    clocks/events, never assertions on real elapsed time, machine load or sampled
    memory; measurements belong to benchmarks. Never test authored prose or one
    repository constant, and never add tautological assertions.
    **Why:** proof must fail on broken behavior, independently of the workstation.
    **Where:** `openspec/changes/test-stories/specs/test-stories/spec.md`;
    Deterministic Behavioral Proof in
    `openspec/changes/operation-grammar/specs/operation-contract/spec.md`.
    **Proof:** No proof yet for repository-wide compliance; controlled-clock examples
    such as invariant 23 do not establish it for every suite.
    **Status:** locked direction (`operation-grammar`, retaining the agreed `test-stories` rule).

19. **Catalog Integrity Before Activation.** Validate catalog digest, descriptors,
    schema fingerprints and requirements before admitting operations; refuse
    ambiguous providers instead of choosing by arrival order.
    **Why:** opaque bytes are safe to route only under the admitted contract, and
    a renamed operation must not silently satisfy an old requirement.
    **Where:** `crates/harmos-protocol/src/schema.rs`,
    `crates/harmos-runtime/src/plugin/grpc/catalog.rs`,
    `crates/harmos-runtime/src/operations/manager/admission.rs`;
    Catalog Admission Integrity in
    `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
    **Proof:** `host_refuses_tampered_catalogs_before_activation` in
    `crates/harmos-runtime/tests/plugin_grpc/catalog_admission.rs`;
    `duplicate_operation_names_both_owners` in
    `crates/harmos-runtime/tests/plugin_grpc/duplicate_operation.rs`.
    **Status:** landed for gRPC plugin admission.

20. **Committed Stream Revisions.** Derive revisions from canonical row schema and
    window; treat hashes as opaque, never choose current by numeric order. Activation
    commits current selection atomically; failed staging preserves it and readers
    retain their own revision's descriptor.
    **Why:** reload must not decode old rows with a new schema or invalidate retained readers.
    **Where:** `crates/harmos-domain/src/schema/stream_revision.rs`,
    `crates/harmos-runtime/src/streams/manager.rs`;
    `openspec/specs/stream-revisions/spec.md`.
    **Proof:** `changed_row_commits_a_new_current_revision`,
    `failed_reload_keeps_old_revision_current` and
    `restart_on_one_root_reads_each_revision_with_its_own_schema` in
    `crates/harmos-runtime/tests/streams/revisions.rs`.
    **Status:** landed.

21. **Retained Plugin History.** Read plugin-owned history with its written
    definitions after shape changes or removal; incompatible typed current reads
    refuse rather than converting retained history to current state.
    **Why:** removing/replacing executable code must not erase historical evidence;
    host recovery compatibility remains a separate constraint (invariant 14).
    **Where:** `crates/harmos-domain/src/state/envelope.rs`,
    `crates/harmos-runtime/src/storage/attachments.rs`;
    `openspec/specs/plugin-state-history/spec.md`.
    **Proof:** `plugin_history_survives_shape_change_and_absence_on_one_root` in
    `crates/harmos-runtime/tests/storage/state/shape_versions.rs`.
    **Status:** landed.

22. **Fenced File Production.** Production and publication are distinct: only the
    current writer may append/publish; resumption fences the previous writer and
    abandonment is explicit. Reconnection must not silently publish or delete work.
    **Why:** interrupted producers need recoverable progress without competing writers
    or treating partially written bytes as a finished file.
    **Where:** `crates/harmos-storage/src/staging/mutations.rs`,
    `crates/harmos-storage/src/staging/session.rs`,
    `crates/harmos-storage/src/file_production.rs`,
    `crates/harmos-storage/src/backend.rs`;
    `openspec/changes/operation-grammar/specs/storage-transfers/spec.md`.
    **Proof:** `sessions_survive_reopen` in
    `crates/harmos-storage/tests/integration/file_sessions.rs` and
    `mutation_receipts_survive_replays_lost_responses_and_cancellation` in
    `crates/harmos-runtime/tests/storage/manager/receipts.rs`.
    `resume_fence_publish_and_range_across_faces` guards unary client/plugin transfer
    sessions, replay, fencing and bounded immutable reads. Bundled manager stories
    upload through the same client session operations. Diagnostics and file snapshots
    share `harmos_storage::get_content_bytes`; the service manager owns archive retention.
    **Status:** landed for current production sessions.

23. **Authoritative Host Time.** Plugin waits and interval publication use the host
    clock; active work prevents virtual time from advancing past unsettled work.
    Device-driven publication remains distinct from host interval ticks.
    **Why:** deterministic simulation must see plugin demand and work ownership,
    rather than race a plugin's independent wall clock.
    **Where:** `crates/harmos-runtime/src/clock.rs`,
    `crates/harmos-runtime/src/plugin/grpc/invoke.rs`,
    `crates/harmos-runtime/src/plugin/grpc/publish.rs`;
    Authoritative Host Time in
    `openspec/changes/operation-grammar/specs/operation-invocation/spec.md`.
    **Proof:** `an_in_flight_plugin_call_blocks_virtual_time_until_settlement` in
    `crates/harmos-runtime/tests/plugin_grpc/active_call.rs`;
    `host_ticks_are_stored_in_order_with_host_timestamps` in
    `crates/harmos-runtime/tests/plugin_grpc/interval_stream.rs` and
    `device_samples_need_no_tick_and_keep_source_time_separate` in
    `crates/harmos-runtime/tests/plugin_grpc/device_stream.rs`.
    **Status:** landed.

24. **Atomic Recovery Snapshots.** Preserve state, undo history, receipts and tail
    in verified ordered bounded segments; publish only complete durable snapshots
    and remove covered journal entries only after publication.
    **Why:** interrupted checkpoint work must leave recoverable state equivalent
    to journal replay, without silently dropping undo or deduplication history.
    **Where:** `crates/harmos-runtime/src/state/snapshot_codec.rs`,
    `crates/harmos-storage/src/state/segments.rs`;
    `openspec/specs/segmented-state-snapshots/spec.md`.
    **Proof:** `segmented_snapshot_matches_full_replay` in
    `crates/harmos-runtime/tests/storage/state/snapshots/segmented_snapshot_matches_full_replay.rs`;
    `snapshot_failure_retryable` in
    `crates/harmos-runtime/tests/storage/state/snapshots/snapshot_failure_retryable.rs`.
    **Status:** landed.

## Glossary

- **Face / Visibility:** wire audience/serving boundary versus operation admission level; neither replaces the other.
- **Operation / Invocation / Attempt:** declared contract / one admitted execution / one try under its retry policy; an attempt is not a new logical request.
- **Position / Cut / Page Token / Snapshot:** progress in one history / a consistent selected boundary (possibly composite) / opaque continuation bound to a selection / retained read image or recovery image identified by context; none is a content digest or interchangeable cursor.
- **Accepted / Durable:** admitted into owned work versus acknowledged persisted under backend guarantees; accepted is not crash-safe acknowledgement.
- **Instance / Plugins Service:** one running plugin activation versus host management of plugin entries; the management service is not the plugin protocol endpoint.
- **Manager / Service:** owner of live orchestration and lifecycle versus an API service or resident lifecycle participant, as qualified; a service name is not a new manager.
- **Scope / Grant:** opaque inherited caller context versus checked permission to an operation/resource; scope text is never a token of authority.

## Decisions and Their Reasons

| Decision | Reason | Recording Change |
| --- | --- | --- |
| Three visibility levels; explicit choice; decided built-in mapping (2026-11-12) | Local privacy, host-only control and exposed peer calls need separate admission meanings. | `openspec/changes/operation-grammar/` |
| Audience faces and face-local contracts (2026-11-12) | Separate who serves/calls a wire API without removing operation visibility. | `openspec/changes/operation-grammar/` |
| Runtime composition, manager inventory and product-owned settings/secrets (2026-11-12) | Reuse generic machinery under one lifecycle authority while retaining application policy. | `openspec/changes/operation-grammar/` |
| Resource grammar, nested packages and shared primitives (2026-11-12) | Generate predictable SDKs without flattening storage guarantees. | `openspec/changes/operation-grammar/` |
| Latest durable reads, measured lag and caller resending (2026-11-12) | Repeatable reads and lost replies must preserve durable boundaries and original outcomes. | `openspec/changes/operation-grammar/` |
| No automatic twins; distinct Stop and Cancel (2026-11-12) | Keep one method per promise and preserve cooperative final results. | `openspec/changes/operation-grammar/` |
| Host-only control uses Protected | Keep host controls on the instance carrier and refuse peer/client calls. | `openspec/changes/operation-grammar/` |
| Retained plugin history and derived revisions (2026-11-07) | Written schema identity must outlive plugin replacement or removal. | `openspec/changes/archive/2026-11-07-plugin-state-survives-shape-changes/`, `openspec/changes/archive/2026-11-07-stream-schema-revisions/` |
| Segmented recovery snapshots (2026-11-08) | Bounded checkpoint pieces must preserve complete recovery and atomic publication. | `openspec/changes/archive/2026-11-08-snapshot-payload-limit/` |
| Deterministic stories (agreed 2026-11-10) | Correctness must not depend on workstation speed or measurement noise. | `openspec/changes/test-stories/` |
