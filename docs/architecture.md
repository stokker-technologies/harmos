# Architecture

Transfers remain unary, amended by the owner on 2026-11-13. The generated
`StorageTransferSettings.chunk_bytes` under `StorageSettings.transfer` owns the
single 4 MiB default for native append and ranged-read sizes and product wrappers. Generated
`MessageSettings.message_bytes` declares the separate 8 MiB message default once.
Client SDKs, listeners, plugin carriers and Wasm guests use it and refuse ceilings
that cannot carry the configured chunk plus framing at startup, naming the action and
setting. Streaming transfer handlers, proto requests, bindings, SDK surfaces,
stories and benchmark cases are deleted; Watch and native readers remain.

Application-owned immutable content registers through StorageManager's single
`ContentSource`. Resolution receives the inherited caller scope and returns length,
SHA-256 and a bounded lease with explicit expiry. Native GetFileBytes and product wrappers retain
that lease in the existing bounded read-cut registry, bound to the reference and
scope. Expiry refuses rather than renewing a continuation. Native hosts use
`open_content(&reference, scope, at)` and `ContentRead::get_bytes(offset, maximum)`.
Library-history references keep their saved-storage path; no application bytes or
metadata enter library persistence. Diagnostics and bundle installation use the
same content boundary. See [Application-Owned Content](storage.md#application-owned-content).

## How to Use This Document

Python client resources are committed generated classes derived from protocol
descriptors by `tools/python/src/harmos_tools/build/resource_roots.py`. Each RPC
has explicit request, response and per-call option types; server streams retain
typed async iteration and native cancellation. `sdk/python/src/harmos/client.py`
initializes the library root and owns connections created by `Client.connect`.
Products generate and compose their own typed resources. No runtime attribute
binding or serialized resource table participates in any SDK's client naming.
See the [Python SDK Guide](python-sdk.md).

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
without bound are paged. Operations, Schemas, Streams, Plugins and Access.Pairings
return the complete set without page requests, tokens or application byte bounds.
The carrier message ceiling is their sole response guard; overflow names the action.
Records, samples, log lines, trace events, files and rows retain position-resolved paging.

## Protocol Source Convention

Use the selected proto skeleton, preserving Imports, Options, Services, Resources
and Enums even when empty. A resource owns one file, its service and its wrappers.
After Services, replace the generic Requests and Responses section with one
`// === Name === //` section per operation in service order; each contains the
request directly before the response. Nested resources live in subfolders.
Every service, call and message has a contract comment. Describe optional selectors,
opaque revisions/cuts, refusal and replay from their owning contracts. Buf owns
canonical layout, field-per-line formatting and comment lint. Proto2 transfer
presence/defaults remain unchanged.

```proto
syntax = "proto3";
package harmos.host.v1;
// === Imports === //
import "harmos/model/v1/callers.proto";
// === Options === //
// === Services === //
// Example resource; each operation owns its caller declaration.
service Resource {
  // Read the resource.
  rpc Get(ResourceGetRequest) returns (ResourceGetResponse) {
    option (harmos.model.v1.callers) = ALL;
  }
}
// === Get === //
// Selects the resource.
message ResourceGetRequest {}
// Returns the resource.
message ResourceGetResponse {}
// === Resources === //
// === Enums === //
```

The independent library and example Buf modules run through `mise run format` and
`mise run check -- --repository`; the latter also enforces mandatory caller options.
Regenerate with `mise run generate -- --sdk rust`, then python, csharp and cpp.
Generation owns every output and removes obsolete generated modules. The protocol
drift test compares committed sources and descriptor projections byte for byte.
See the mapping below for the current definition-pass/consumer boundary.

## Invariants

Wire operation admission uses the protocol-owned validator in
`crates/harmos-protocol/src/schema.rs`, independently of native dotted identities.
The [wire identity contract](../openspec/changes/operation-grammar/design.md#wire-operation-identity-admission)
covers numeric template bindings and application-owned face packages. Declaration admission,
guest admission and host protobuf declarations share it; visibility, descriptor fingerprints
and caller-face authorization remain independent checks.

1. **Executor Packages and Declared Callers.** Model has no operations. Host and
   plugin packages identify where operations run; the method option declares who
   may call each one. Shared operations have one identity and wrapper pair.
   The host mediates cross-plugin calls; Private operations stay in-process.
   **Why:** avoid resource twins while preserving caller-specific admission.
   **Where:** proto/harmos, the mapping and work lists below.
   **Proof:** Buf plus caller-option checks and generator narrowing/projection tests
   cover definitions. Runtime denial cases 33/34 remain with their consumer.
   **Status:** definitions/generated outputs landed; runtime and SDK consumers pending.

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
   process, absent from its declarations, invisible to the host, never on any wire
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
   **Proof:** common conformance case 24 guards local dispatch, declaration omission
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
   **Where:** `proto/harmos/plugin/v1/instance.proto`, `proto/harmos/plugin/v1/behaviour.md`,
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
   OperationManager owns operation discovery and invocations; StreamManager owns
   publication/demand; StorageManager owns persistence/production; GrpcManager owns
   listeners/access; ServiceManager owns resident services/status and host clock;
   StateManager owns state/replay; RecordManager owns record registration/lifecycle;
   plugin management owns entries, typed settings, placement, activation and shutdown.
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
   **Status:** landed; native proof is tracked separately from source.

9. **Library and Product Composition.** The library owns hosting, access, status,
   invocations, time, plugin management and typed plugin settings. Products compose
   these complete wire services directly. Storage and publication remain Private
   in-process capabilities; products declare their own typed wire operations and
   policy over them. Products own schemas, indexes/projections, objects, operations,
   authorization, secrets, UI, archives and credential integration.
   **Why:** one owner per atomic operation and no duplicate wire twins. The library
   stores plugin settings beside entries; product secrets remain outside that store.
   **Where:** Owner Amendments of 2026-11-13 and the boundary mapping below.
   **Proof:** settings, replacement and trusted caller journeys under
   `crates/harmos-runtime/tests/plugin_grpc/manager`; application upload and discovery
   under `examples/schematic/app/tests/storage`.
   **Status:** landed source; candidate validation is separate.

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
    bound page tokens. Durable lag is accepted as measured, amended by the owner on
    2026-11-13: small about 99 ms; large median 391 ms (178–570); mixed median
    346 ms (173–493) at p95. No accepted-data snapshot is built; Nightly reports
    p50, p95 and maximum per load without a failing threshold.
    **Why:** repeatable reads and read-your-write receipts must not hide a persistence
    barrier. Lag remains measurement, never an assertion against real time in a test.
    **Where:** `openspec/changes/operation-grammar/specs/storage-visibility/spec.md`.
    **Proof:** Issued-page, native-list and cut-lifetime stories in
    `crates/harmos-runtime/tests/storage/` cover durable selection and continuation.
    `watch_rows_closes_opening_races_and_resynchronizes_on_retention_loss` covers
    socket/child durable observation, opening races, filtered progress and resync;
    `durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps`
    in the native storage manager tests covers committed-prefix gating.
    `filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations`
    covers metadata observations, pinned continuation, eviction and restart refusal.
    The owner accepted the repeated benchmark measurements on 2026-11-13.
    **Status:** landed; native Linux/hosted measurements remain pending.

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

17. **Generic Library SDKs.** One SDK package per language, amended by the owner on
    2026-11-13: Python `harmos-sdk` (import `harmos`), Rust `harmos`, NuGet `Harmos`
    and CMake `Harmos` (`Harmos::Cpp`) include authoring, client roots and host
    protocol needs. Audience-specific builds and stub guards are removed. The host
    enforces faces and visibility at runtime, regardless of available bindings.
    Keep all four SDKs in this repository, with
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
    C++ installs `Harmos::Cpp` with config-package dependencies; its conformance
    implementation belongs to example support, outside the installed library. Conformance
    protocol input lives under `sdk/tests/fixtures/conformance/proto/`; generated fixture
    messages and implementations belong only to test and conformance targets in every SDK.
    C# builds every project through `sdk/csharp/Harmos.slnx`; central properties and
    package versions own the .NET build. Committed client roots live in `src/Harmos/Generated`
    and `tests/Harmos.Tests/Generated`. All build outputs live under `artifacts`; executable
    manifests from MSBuild are the path authority for test and fixture consumers. NuGet `Harmos`
    takes its version from the Cargo workspace and includes SourceLink and repository metadata.

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

19. **Declaration Integrity Before Activation.** Validate declarations digest, descriptors,
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
    share `harmos_storage::ContentSource` and `ContentRead`; the service manager owns archive retention.
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
| Executor layout and mandatory callers (2026-11-14) | One operation home; caller admission is independent of executor and visibility. | `openspec/changes/operation-grammar/` |
| Runtime composition, manager inventory and product-owned settings/secrets (2026-11-12) | Reuse generic machinery under one lifecycle authority while retaining application policy. | `openspec/changes/operation-grammar/` |
| Resource grammar, nested packages and shared primitives (2026-11-12) | Generate predictable SDKs without flattening storage guarantees. | `openspec/changes/operation-grammar/` |
| Latest durable reads, measured lag and caller resending (2026-11-12) | Repeatable reads and lost replies must preserve durable boundaries and original outcomes. | `openspec/changes/operation-grammar/` |
| No automatic twins; distinct Stop and Cancel (2026-11-12) | Keep one method per promise and preserve cooperative final results. | `openspec/changes/operation-grammar/` |
| Host-only control uses Protected | Keep host controls on the instance carrier and refuse peer/client calls. | `openspec/changes/operation-grammar/` |
| Retained plugin history and derived revisions (2026-11-07) | Written schema identity must outlive plugin replacement or removal. | `openspec/changes/archive/2026-11-07-plugin-state-survives-shape-changes/`, `openspec/changes/archive/2026-11-07-stream-schema-revisions/` |
| Segmented recovery snapshots (2026-11-08) | Bounded checkpoint pieces must preserve complete recovery and atomic publication. | `openspec/changes/archive/2026-11-08-snapshot-payload-limit/` |
| Deterministic stories (agreed 2026-11-10) | Correctness must not depend on workstation speed or measurement noise. | `openspec/changes/test-stories/` |

## Protocol Boundary Definition and Consumer Migration

### Executor Layout and Definition Status

The 2026-11-14 owner decision replaces audience packages with executor packages.
This definition pass lands protocol declarations, generators and generated outputs;
hand-written consumers and runtime admission remain assigned below. Former names
in the mapping are migration references only. There are no compatibility packages.

```text
proto/harmos/model/v1/    harmos.model.v1     exchanged values; no services
proto/harmos/host/v1/     harmos.host.v1      operations executed on the host
proto/harmos/plugin/v1/   harmos.plugin.v1    operations executed on a plugin
```

Before moving files, the duplicate inventory was: Operations List/Get and Time
Get/Sleep are identical pairs; Invocations joins client Invoke with shared Get,
Cancel and Watch, renaming Invoke to Create; Settings joins nested client List/Set
with plugin-host List, keeping the richer response and one optional name selector.
Settings.List was the only same-name clash with different field layouts: the new
shape retains the client's name=1, at=2 and declarations/value/revision/resolved_at
response tags 1–4. All other operations keep their names and field numbers.

The tree changes from 33 proto files, 19 services and 45 operations to 33 proto
files, 15 services and 37 operations (host 14/33; plugin 1/4; model zero services).
Storage operations stay in-process and are outside both censuses. The runtime
always includes its storage manager and default in-memory adapter; this pass
makes no storage/query source changes.

### Call Mapping

Every row names the former identity, its single replacement, its replacement
request/response and the declared callers. Repeated target identities are merged
operations, not duplicate declarations. Resource segments follow the version:
`harmos.host.v1.plugins.Settings` is `host/v1/plugins/settings.proto`.

| Former Identity | New Identity | New Request / Response | Callers |
| --- | --- | --- | --- |
| `harmos.client.v1.access.Certificates/Create` | `harmos.host.v1.access.Certificates/Create` | `harmos.host.v1.access.CertificatesCreateRequest` / `harmos.host.v1.access.CertificatesCreateResponse` | CLIENTS |
| `harmos.client.v1.access.Listener/Get` | `harmos.host.v1.access.Listener/Get` | `harmos.host.v1.access.ListenerGetRequest` / `harmos.host.v1.access.ListenerGetResponse` | CLIENTS |
| `harmos.client.v1.access.Listener/Set` | `harmos.host.v1.access.Listener/Set` | `harmos.host.v1.access.ListenerSetRequest` / `harmos.host.v1.access.ListenerSetResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/Create` | `harmos.host.v1.access.Pairings/Create` | `harmos.host.v1.access.PairingsCreateRequest` / `harmos.host.v1.access.PairingsCreateResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/List` | `harmos.host.v1.access.Pairings/List` | `harmos.host.v1.access.PairingsListRequest` / `harmos.host.v1.access.PairingsListResponse` | CLIENTS |
| `harmos.client.v1.access.Pairings/Delete` | `harmos.host.v1.access.Pairings/Delete` | `harmos.host.v1.access.PairingsDeleteRequest` / `harmos.host.v1.access.PairingsDeleteResponse` | CLIENTS |
| `harmos.client.v1.Invocations/Invoke` | `harmos.host.v1.Invocations/Create` | `harmos.host.v1.InvocationsCreateRequest` / `harmos.host.v1.InvocationsCreateResponse` | CLIENTS |
| `harmos.client.v1.Invocations/Get` | `harmos.host.v1.Invocations/Get` | `harmos.host.v1.InvocationsGetRequest` / `harmos.host.v1.InvocationsGetResponse` | ALL |
| `harmos.client.v1.Invocations/Cancel` | `harmos.host.v1.Invocations/Cancel` | `harmos.host.v1.InvocationsCancelRequest` / `harmos.host.v1.InvocationsCancelResponse` | ALL |
| `harmos.client.v1.Invocations/Watch` | `harmos.host.v1.Invocations/Watch` | `harmos.host.v1.InvocationsWatchRequest` / `harmos.host.v1.InvocationsWatchResponse` | ALL |
| `harmos.client.v1.Operations/List` | `harmos.host.v1.Operations/List` | `harmos.host.v1.OperationsListRequest` / `harmos.host.v1.OperationsListResponse` | ALL |
| `harmos.client.v1.Operations/Get` | `harmos.host.v1.Operations/Get` | `harmos.host.v1.OperationsGetRequest` / `harmos.host.v1.OperationsGetResponse` | ALL |
| `harmos.client.v1.Plugins/Get` | `harmos.host.v1.Plugins/Get` | `harmos.host.v1.PluginsGetRequest` / `harmos.host.v1.PluginsGetResponse` | CLIENTS |
| `harmos.client.v1.Plugins/List` | `harmos.host.v1.Plugins/List` | `harmos.host.v1.PluginsListRequest` / `harmos.host.v1.PluginsListResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Watch` | `harmos.host.v1.Plugins/Watch` | `harmos.host.v1.PluginsWatchRequest` / `harmos.host.v1.PluginsWatchResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Create` | `harmos.host.v1.Plugins/Create` | `harmos.host.v1.PluginsCreateRequest` / `harmos.host.v1.PluginsCreateResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Set` | `harmos.host.v1.Plugins/Set` | `harmos.host.v1.PluginsSetRequest` / `harmos.host.v1.PluginsSetResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Delete` | `harmos.host.v1.Plugins/Delete` | `harmos.host.v1.PluginsDeleteRequest` / `harmos.host.v1.PluginsDeleteResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Restart` | `harmos.host.v1.Plugins/Restart` | `harmos.host.v1.PluginsRestartRequest` / `harmos.host.v1.PluginsRestartResponse` | CLIENTS |
| `harmos.client.v1.Plugins/Replace` | `harmos.host.v1.Plugins/Replace` | `harmos.host.v1.PluginsReplaceRequest` / `harmos.host.v1.PluginsReplaceResponse` | CLIENTS |
| `harmos.client.v1.plugins.Settings/List` | `harmos.host.v1.plugins.Settings/List` | `harmos.host.v1.plugins.SettingsListRequest` / `harmos.host.v1.plugins.SettingsListResponse` | ALL |
| `harmos.client.v1.plugins.Settings/Set` | `harmos.host.v1.plugins.Settings/Set` | `harmos.host.v1.plugins.SettingsSetRequest` / `harmos.host.v1.plugins.SettingsSetResponse` | CLIENTS |
| `harmos.client.v1.Schemas/List` | `harmos.host.v1.Schemas/List` | `harmos.host.v1.SchemasListRequest` / `harmos.host.v1.SchemasListResponse` | CLIENTS |
| `harmos.client.v1.Status/Get` | `harmos.host.v1.Status/Get` | `harmos.host.v1.StatusGetRequest` / `harmos.host.v1.StatusGetResponse` | CLIENTS |
| `harmos.client.v1.Status/Watch` | `harmos.host.v1.Status/Watch` | `harmos.host.v1.StatusWatchRequest` / `harmos.host.v1.StatusWatchResponse` | CLIENTS |
| `harmos.client.v1.status.Diagnostics/Get` | `harmos.host.v1.status.Diagnostics/Get` | `harmos.host.v1.status.DiagnosticsGetRequest` / `harmos.host.v1.status.DiagnosticsGetResponse` | CLIENTS |
| `harmos.client.v1.status.Logs/List` | `harmos.host.v1.status.Logs/List` | `harmos.host.v1.status.LogsListRequest` / `harmos.host.v1.status.LogsListResponse` | CLIENTS |
| `harmos.client.v1.status.Logs/Watch` | `harmos.host.v1.status.Logs/Watch` | `harmos.host.v1.status.LogsWatchRequest` / `harmos.host.v1.status.LogsWatchResponse` | CLIENTS |
| `harmos.client.v1.status.TraceEvents/List` | `harmos.host.v1.status.TraceEvents/List` | `harmos.host.v1.status.TraceEventsListRequest` / `harmos.host.v1.status.TraceEventsListResponse` | CLIENTS |
| `harmos.client.v1.status.TraceEvents/Watch` | `harmos.host.v1.status.TraceEvents/Watch` | `harmos.host.v1.status.TraceEventsWatchRequest` / `harmos.host.v1.status.TraceEventsWatchResponse` | CLIENTS |
| `harmos.client.v1.Streams/List` | `harmos.host.v1.Streams/List` | `harmos.host.v1.StreamsListRequest` / `harmos.host.v1.StreamsListResponse` | CLIENTS |
| `harmos.client.v1.Time/Get` | `harmos.host.v1.Time/Get` | `harmos.host.v1.TimeGetRequest` / `harmos.host.v1.TimeGetResponse` | ALL |
| `harmos.client.v1.Time/Sleep` | `harmos.host.v1.Time/Sleep` | `harmos.host.v1.TimeSleepRequest` / `harmos.host.v1.TimeSleepResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Get` | `harmos.host.v1.Invocations/Get` | `harmos.host.v1.InvocationsGetRequest` / `harmos.host.v1.InvocationsGetResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Cancel` | `harmos.host.v1.Invocations/Cancel` | `harmos.host.v1.InvocationsCancelRequest` / `harmos.host.v1.InvocationsCancelResponse` | ALL |
| `harmos.plugin.host.v1.Invocations/Watch` | `harmos.host.v1.Invocations/Watch` | `harmos.host.v1.InvocationsWatchRequest` / `harmos.host.v1.InvocationsWatchResponse` | ALL |
| `harmos.plugin.host.v1.Operations/List` | `harmos.host.v1.Operations/List` | `harmos.host.v1.OperationsListRequest` / `harmos.host.v1.OperationsListResponse` | ALL |
| `harmos.plugin.host.v1.Operations/Get` | `harmos.host.v1.Operations/Get` | `harmos.host.v1.OperationsGetRequest` / `harmos.host.v1.OperationsGetResponse` | ALL |
| `harmos.plugin.host.v1.Settings/List` | `harmos.host.v1.plugins.Settings/List` | `harmos.host.v1.plugins.SettingsListRequest` / `harmos.host.v1.plugins.SettingsListResponse` | ALL |
| `harmos.plugin.host.v1.Time/Get` | `harmos.host.v1.Time/Get` | `harmos.host.v1.TimeGetRequest` / `harmos.host.v1.TimeGetResponse` | ALL |
| `harmos.plugin.host.v1.Time/Sleep` | `harmos.host.v1.Time/Sleep` | `harmos.host.v1.TimeSleepRequest` / `harmos.host.v1.TimeSleepResponse` | ALL |
| `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.InstanceGetRequest` / `harmos.plugin.v1.InstanceGetResponse` | CLIENTS |
| `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.InstanceInvokeRequest` / `harmos.plugin.v1.InstanceInvokeResponse` | CLIENTS |
| `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.InstanceSamplesRequest` / `harmos.plugin.v1.InstanceSamplesResponse` | CLIENTS |
| `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.InstanceStopRequest` / `harmos.plugin.v1.InstanceStopResponse` | CLIENTS |

### Message and Enum Mapping

Every top-level message and enum from the former tree is listed, including the
unchanged Instance wrappers so carrier moves can be applied mechanically. Nested
messages, oneofs and enum identities follow their enclosing type's replacement
prefix, with their names, numbers and order unchanged. The method-option extension
is new: `harmos.model.v1.callers` in `model/v1/callers.proto`.

| Former Identity | New Identity | New Source |
| --- | --- | --- |
| `harmos.client.v1.access.CertificatesCreateRequest` | `harmos.host.v1.access.CertificatesCreateRequest` | `harmos/host/v1/access/certificates.proto` |
| `harmos.client.v1.access.CertificatesCreateResponse` | `harmos.host.v1.access.CertificatesCreateResponse` | `harmos/host/v1/access/certificates.proto` |
| `harmos.client.v1.access.ListenerGetRequest` | `harmos.host.v1.access.ListenerGetRequest` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerGetResponse` | `harmos.host.v1.access.ListenerGetResponse` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerSetRequest` | `harmos.host.v1.access.ListenerSetRequest` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerSetResponse` | `harmos.host.v1.access.ListenerSetResponse` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.ListenerValue` | `harmos.host.v1.access.ListenerValue` | `harmos/host/v1/access/listener.proto` |
| `harmos.client.v1.access.PairingsCreateRequest` | `harmos.host.v1.access.PairingsCreateRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsCreateResponse` | `harmos.host.v1.access.PairingsCreateResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsListRequest` | `harmos.host.v1.access.PairingsListRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsListResponse` | `harmos.host.v1.access.PairingsListResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsDeleteRequest` | `harmos.host.v1.access.PairingsDeleteRequest` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.PairingsDeleteResponse` | `harmos.host.v1.access.PairingsDeleteResponse` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.access.Pairing` | `harmos.host.v1.access.Pairing` | `harmos/host/v1/access/pairings.proto` |
| `harmos.client.v1.InvocationsInvokeRequest` | `harmos.host.v1.InvocationsCreateRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsInvokeResponse` | `harmos.host.v1.InvocationsCreateResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsGetRequest` | `harmos.host.v1.InvocationsGetRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsGetResponse` | `harmos.host.v1.InvocationsGetResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsCancelRequest` | `harmos.host.v1.InvocationsCancelRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsCancelResponse` | `harmos.host.v1.InvocationsCancelResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsWatchRequest` | `harmos.host.v1.InvocationsWatchRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.InvocationsWatchResponse` | `harmos.host.v1.InvocationsWatchResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.client.v1.OperationsListRequest` | `harmos.host.v1.OperationsListRequest` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsListResponse` | `harmos.host.v1.OperationsListResponse` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsGetRequest` | `harmos.host.v1.OperationsGetRequest` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.OperationsGetResponse` | `harmos.host.v1.OperationsGetResponse` | `harmos/host/v1/operations.proto` |
| `harmos.client.v1.PluginsGetRequest` | `harmos.host.v1.PluginsGetRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsGetResponse` | `harmos.host.v1.PluginsGetResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsListRequest` | `harmos.host.v1.PluginsListRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsListResponse` | `harmos.host.v1.PluginsListResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsWatchRequest` | `harmos.host.v1.PluginsWatchRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsWatchResponse` | `harmos.host.v1.PluginsWatchResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsCreateRequest` | `harmos.host.v1.PluginsCreateRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsCreateResponse` | `harmos.host.v1.PluginsCreateResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsSetRequest` | `harmos.host.v1.PluginsSetRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsSetResponse` | `harmos.host.v1.PluginsSetResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsDeleteRequest` | `harmos.host.v1.PluginsDeleteRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsDeleteResponse` | `harmos.host.v1.PluginsDeleteResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsRestartRequest` | `harmos.host.v1.PluginsRestartRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsRestartResponse` | `harmos.host.v1.PluginsRestartResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsReplaceRequest` | `harmos.host.v1.PluginsReplaceRequest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginsReplaceResponse` | `harmos.host.v1.PluginsReplaceResponse` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginValue` | `harmos.host.v1.PluginValue` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginDescription` | `harmos.host.v1.PluginDescription` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginRemote` | `harmos.host.v1.PluginRemote` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginManaged` | `harmos.host.v1.PluginManaged` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginGuest` | `harmos.host.v1.PluginGuest` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginPlacement` | `harmos.host.v1.PluginPlacement` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginProcess` | `harmos.host.v1.PluginProcess` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginEntry` | `harmos.host.v1.PluginEntry` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginSnapshot` | `harmos.host.v1.PluginSnapshot` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginChange` | `harmos.host.v1.PluginChange` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.PluginState` | `harmos.host.v1.PluginState` | `harmos/host/v1/plugins.proto` |
| `harmos.client.v1.plugins.SettingsListRequest` | `harmos.host.v1.plugins.SettingsListRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsListResponse` | `harmos.host.v1.plugins.SettingsListResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsSetRequest` | `harmos.host.v1.plugins.SettingsSetRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.plugins.SettingsSetResponse` | `harmos.host.v1.plugins.SettingsSetResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.client.v1.SchemasListRequest` | `harmos.host.v1.SchemasListRequest` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.SchemasListResponse` | `harmos.host.v1.SchemasListResponse` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.SchemaDescription` | `harmos.host.v1.SchemaDescription` | `harmos/host/v1/schemas.proto` |
| `harmos.client.v1.StatusGetRequest` | `harmos.host.v1.StatusGetRequest` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusGetResponse` | `harmos.host.v1.StatusGetResponse` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusWatchRequest` | `harmos.host.v1.StatusWatchRequest` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.StatusWatchResponse` | `harmos.host.v1.StatusWatchResponse` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HostIdentity` | `harmos.host.v1.HostIdentity` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HealthComponent` | `harmos.host.v1.HealthComponent` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.HostStatus` | `harmos.host.v1.HostStatus` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.ComponentState` | `harmos.host.v1.ComponentState` | `harmos/host/v1/status.proto` |
| `harmos.client.v1.status.DiagnosticsGetRequest` | `harmos.host.v1.status.DiagnosticsGetRequest` | `harmos/host/v1/status/diagnostics.proto` |
| `harmos.client.v1.status.DiagnosticsGetResponse` | `harmos.host.v1.status.DiagnosticsGetResponse` | `harmos/host/v1/status/diagnostics.proto` |
| `harmos.client.v1.status.LogsListRequest` | `harmos.host.v1.status.LogsListRequest` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsListResponse` | `harmos.host.v1.status.LogsListResponse` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsWatchRequest` | `harmos.host.v1.status.LogsWatchRequest` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.LogsWatchResponse` | `harmos.host.v1.status.LogsWatchResponse` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.Event` | `harmos.host.v1.status.Event` | `harmos/host/v1/status/logs.proto` |
| `harmos.client.v1.status.TraceEventsListRequest` | `harmos.host.v1.status.TraceEventsListRequest` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsListResponse` | `harmos.host.v1.status.TraceEventsListResponse` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsWatchRequest` | `harmos.host.v1.status.TraceEventsWatchRequest` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.status.TraceEventsWatchResponse` | `harmos.host.v1.status.TraceEventsWatchResponse` | `harmos/host/v1/status/trace_events.proto` |
| `harmos.client.v1.StreamsListRequest` | `harmos.host.v1.StreamsListRequest` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.StreamsListResponse` | `harmos.host.v1.StreamsListResponse` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.StreamIdentity` | `harmos.host.v1.StreamIdentity` | `harmos/host/v1/streams.proto` |
| `harmos.client.v1.TimeGetRequest` | `harmos.host.v1.TimeGetRequest` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeGetResponse` | `harmos.host.v1.TimeGetResponse` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeSleepRequest` | `harmos.host.v1.TimeSleepRequest` | `harmos/host/v1/time.proto` |
| `harmos.client.v1.TimeSleepResponse` | `harmos.host.v1.TimeSleepResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.InvocationsGetRequest` | `harmos.host.v1.InvocationsGetRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsGetResponse` | `harmos.host.v1.InvocationsGetResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsCancelRequest` | `harmos.host.v1.InvocationsCancelRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsCancelResponse` | `harmos.host.v1.InvocationsCancelResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsWatchRequest` | `harmos.host.v1.InvocationsWatchRequest` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.InvocationsWatchResponse` | `harmos.host.v1.InvocationsWatchResponse` | `harmos/host/v1/invocations.proto` |
| `harmos.plugin.host.v1.OperationsListRequest` | `harmos.host.v1.OperationsListRequest` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsListResponse` | `harmos.host.v1.OperationsListResponse` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsGetRequest` | `harmos.host.v1.OperationsGetRequest` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.OperationsGetResponse` | `harmos.host.v1.OperationsGetResponse` | `harmos/host/v1/operations.proto` |
| `harmos.plugin.host.v1.SettingsListRequest` | `harmos.host.v1.plugins.SettingsListRequest` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.plugin.host.v1.SettingsListResponse` | `harmos.host.v1.plugins.SettingsListResponse` | `harmos/host/v1/plugins/settings.proto` |
| `harmos.plugin.host.v1.TimeGetRequest` | `harmos.host.v1.TimeGetRequest` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeGetResponse` | `harmos.host.v1.TimeGetResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeSleepRequest` | `harmos.host.v1.TimeSleepRequest` | `harmos/host/v1/time.proto` |
| `harmos.plugin.host.v1.TimeSleepResponse` | `harmos.host.v1.TimeSleepResponse` | `harmos/host/v1/time.proto` |
| `harmos.plugin.v1.InstanceGetRequest` | `harmos.plugin.v1.InstanceGetRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceGetResponse` | `harmos.plugin.v1.InstanceGetResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceInvokeRequest` | `harmos.plugin.v1.InstanceInvokeRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceInvokeResponse` | `harmos.plugin.v1.InstanceInvokeResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceSamplesRequest` | `harmos.plugin.v1.InstanceSamplesRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceSamplesResponse` | `harmos.plugin.v1.InstanceSamplesResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceStopRequest` | `harmos.plugin.v1.InstanceStopRequest` | `harmos/plugin/v1/instance.proto` |
| `harmos.plugin.v1.InstanceStopResponse` | `harmos.plugin.v1.InstanceStopResponse` | `harmos/plugin/v1/instance.proto` |
| `harmos.v1.Declarations` | `harmos.model.v1.Declarations` | `harmos/model/v1/description.proto` |
| `harmos.v1.Section` | `harmos.model.v1.Section` | `harmos/model/v1/description.proto` |
| `harmos.v1.Failure` | `harmos.model.v1.Failure` | `harmos/model/v1/failure.proto` |
| `harmos.v1.Classification` | `harmos.model.v1.Classification` | `harmos/model/v1/failure.proto` |
| `harmos.v1.FileMetadata` | `harmos.model.v1.FileMetadata` | `harmos/model/v1/file.proto` |
| `harmos.v1.FileReference` | `harmos.model.v1.FileReference` | `harmos/model/v1/file.proto` |
| `harmos.v1.File` | `harmos.model.v1.File` | `harmos/model/v1/file.proto` |
| `harmos.v1.Upload` | `harmos.model.v1.Upload` | `harmos/model/v1/file.proto` |
| `harmos.v1.UploadState` | `harmos.model.v1.UploadState` | `harmos/model/v1/file.proto` |
| `harmos.v1.GuestRegistration` | `harmos.plugin.v1.GuestRegistration` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestRequirement` | `harmos.plugin.v1.GuestRequirement` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestCommand` | `harmos.plugin.v1.GuestCommand` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestPrepare` | `harmos.plugin.v1.GuestPrepare` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestInvocation` | `harmos.plugin.v1.GuestInvocation` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestValue` | `harmos.plugin.v1.GuestValue` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestInitialize` | `harmos.plugin.v1.GuestInitialize` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestShutdown` | `harmos.plugin.v1.GuestShutdown` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChild` | `harmos.plugin.v1.GuestChild` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildRequest` | `harmos.plugin.v1.GuestChildRequest` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildControl` | `harmos.plugin.v1.GuestChildControl` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestSubmitted` | `harmos.plugin.v1.GuestSubmitted` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.GuestChildAction` | `harmos.plugin.v1.GuestChildAction` | `harmos/plugin/v1/guest.proto` |
| `harmos.v1.Request` | `harmos.plugin.v1.Request` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.Cancel` | `harmos.plugin.v1.Cancel` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.Response` | `harmos.plugin.v1.Response` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildRequest` | `harmos.plugin.v1.ChildRequest` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildDemand` | `harmos.plugin.v1.ChildDemand` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildCancel` | `harmos.plugin.v1.ChildCancel` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.ChildResponse` | `harmos.plugin.v1.ChildResponse` | `harmos/plugin/v1/invocation.proto` |
| `harmos.v1.InstanceState` | `harmos.model.v1.InstanceState` | `harmos/model/v1/invocation.proto` |
| `harmos.v1.OperationContract` | `harmos.model.v1.OperationContract` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Requirement` | `harmos.model.v1.Requirement` | `harmos/model/v1/operation.proto` |
| `harmos.v1.AssignedOperation` | `harmos.plugin.v1.AssignedOperation` | `harmos/plugin/v1/operation.proto` |
| `harmos.v1.OperationAssignments` | `harmos.plugin.v1.OperationAssignments` | `harmos/plugin/v1/operation.proto` |
| `harmos.v1.Protobuf` | `harmos.model.v1.Protobuf` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Grpc` | `harmos.model.v1.Grpc` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Grant` | `harmos.model.v1.Grant` | `harmos/model/v1/operation.proto` |
| `harmos.v1.DiscoveredContract` | `harmos.model.v1.DiscoveredContract` | `harmos/model/v1/operation.proto` |
| `harmos.v1.DiscoveredOperation` | `harmos.model.v1.DiscoveredOperation` | `harmos/model/v1/operation.proto` |
| `harmos.v1.Visibility` | `harmos.model.v1.Visibility` | `harmos/model/v1/operation.proto` |
| `harmos.v1.PageRequest` | `harmos.model.v1.PageRequest` | `harmos/model/v1/page.proto` |
| `harmos.v1.OperationPolicy` | `harmos.model.v1.OperationPolicy` | `harmos/model/v1/policy.proto` |
| `harmos.v1.RetryPolicy` | `harmos.model.v1.RetryPolicy` | `harmos/model/v1/policy.proto` |
| `harmos.v1.History` | `harmos.model.v1.History` | `harmos/model/v1/position.proto` |
| `harmos.v1.Position` | `harmos.model.v1.Position` | `harmos/model/v1/position.proto` |
| `harmos.v1.ResourceRevision` | `harmos.model.v1.ResourceRevision` | `harmos/model/v1/position.proto` |
| `harmos.v1.ReadCut` | `harmos.model.v1.ReadCut` | `harmos/model/v1/position.proto` |
| `harmos.v1.Receipt` | `harmos.model.v1.Receipt` | `harmos/model/v1/receipt.proto` |
| `harmos.v1.SchemaKey` | `harmos.host.v1.SchemaKey` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaField` | `harmos.host.v1.SchemaField` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaDefinition` | `harmos.host.v1.SchemaDefinition` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaRepresentation` | `harmos.host.v1.SchemaRepresentation` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaGraph` | `harmos.host.v1.SchemaGraph` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaInput` | `deleted (unused wire message)` | `—` |
| `harmos.v1.PayloadEncoding` | `harmos.host.v1.PayloadEncoding` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaCardinality` | `harmos.host.v1.SchemaCardinality` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaKind` | `harmos.host.v1.SchemaKind` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaForm` | `harmos.host.v1.SchemaForm` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SchemaOmission` | `harmos.host.v1.SchemaOmission` | `harmos/host/v1/schema.proto` |
| `harmos.v1.SettingsDeclarations` | `harmos.model.v1.SettingsDeclarations` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingDeclaration` | `harmos.model.v1.SettingDeclaration` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingValue` | `harmos.model.v1.SettingValue` | `harmos/model/v1/setting.proto` |
| `harmos.v1.SettingValues` | `harmos.model.v1.SettingValues` | `harmos/model/v1/setting.proto` |
| `harmos.v1.StreamDeclaration` | `harmos.model.v1.StreamDeclaration` | `harmos/model/v1/stream.proto` |
| `harmos.v1.Tick` | `harmos.plugin.v1.Tick` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.SamplesSelection` | `harmos.plugin.v1.SamplesSelection` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.Sample` | `harmos.plugin.v1.Sample` | `harmos/plugin/v1/stream.proto` |
| `harmos.v1.StorageTransferSettings` | `harmos.model.v1.StorageTransferSettings` | `harmos/model/v1/transfer.proto` |
| `harmos.v1.MessageSettings` | `harmos.model.v1.MessageSettings` | `harmos/model/v1/transfer.proto` |
| `harmos.v1.TransferFraming` | `harmos.model.v1.TransferFraming` | `harmos/model/v1/transfer.proto` |

Decision: Instance uses CLIENTS because its host is the connection client; only
that host may connect. No library host operation is currently PLUGINS-only, so
rule 33 uses a fixture operation. SchemaKey is also used by Streams.List; it remains
registry vocabulary beside Schemas, not a model storage interface. Behaviour now
lives at `proto/harmos/plugin/v1/behaviour.md` beside the connection service.

### Caller Options and SDK Projection

```proto
// model/v1/callers.proto
extend google.protobuf.MethodOptions { Callers callers = 51001; }
enum Callers { CALLERS_UNSPECIFIED = 0; CLIENTS = 1; PLUGINS = 2; ALL = 3; }
// host/v1/time.proto
rpc Get(TimeGetRequest) returns (TimeGetResponse) {
  option (harmos.model.v1.callers) = ALL;
}
```

Every RPC under host/ and plugin/ requires a present, recognized nonzero option;
`mise run check -- --repository` compiles descriptors and checks it, alongside Buf
format and lint. Generated descriptors retain custom method options. Runtime consumers must read
those options from the raw descriptor bytes with prost_reflect; decoding and
re-encoding through prost_types discards unknown custom options. A method's
executor package does not imply its callers. Host operations with CLIENTS or ALL
generate the client surface; PLUGINS or ALL generate the plugin context. Plugin
executor services generate connection stubs, never a host client/context resource.
Reference documentation must use these same projections. Visibility, fingerprints,
trusted identity, inherited scope and resource grants remain independent gates.
The host must refuse a wrong caller before executing any handler or admitting a
managed/encoded child; hiding the method in an SDK is not authorization.

`plugins.Settings.List` clients must supply name. Plugins may omit it or name their
own trusted entry; another name is refused before storage access. Its response
contains declarations, complete values, revision and resolved_at for both callers.
Set is CLIENTS-only and replaces the complete map. No old top-level Settings
service, forwarding adapter or request/response twin survives.

The four public SDK barrels continue to hide wire packages. Shared values use the
root barrel; generated client and plugin resources keep their existing call style:
request content as arguments, owned transport context, and a single response field
returned as the payload. Multi-field replies retain their complete response type.
Python keeps wire modules under `_wire/model`, `_wire/host` and `_wire/plugin`.
Rust protocol modules are `model`, `host` and `plugin`. ALL payloads are exported
through both Rust/Python audience barrels, through the C++ shared root, and through
C# `Harmos.Clients` (the context uses that same public payload type).

### Product Composition and Narrowing

The product-owned `protocol/composition.toml` is the single declarative composition
input for both runtime assembly and SDK generation. Library entries list descriptor
artifacts and the package roots to compose (nested resource packages are included);
paths are relative to that file. Keep dependencies' model packages whenever their
operations reference them. The `callers` table contains only explicit restrictions.

```toml
[[libraries]]
descriptors = "../vendor/library/descriptors.bin"
packages = ["harmos.model.v1", "harmos.host.v1", "harmos.plugin.v1"]

[callers]
"harmos.host.v1.Time/Get" = "PLUGINS"
```

```rust
// Locked assembly API; the core consumer implements this method.
setup.composition(include_str!("../protocol/composition.toml"))?;
```

`composition(&str) -> Result<(), AssemblyError>` applies the file's caller
restrictions to the linked library declarations before listeners or plugins start.
Runtime assembly does not open generator descriptor paths. It verifies the listed
package roots against registered descriptors. The core consumer owns the parser and
admission implementation; this is a declared contract, not landed runtime behavior.
The generator implementation is landed:

```sh
mise run generate -- --descriptors protocol/descriptors.bin --composition protocol/composition.toml --output target/generated/sdk --namespace application --sdk python
```

The CLI accepts the same input for every generated language. An explicit composition
replaces implicit library inclusion. It rejects missing listed packages, conflicting
descriptor files, unknown operation keys, invalid callers and widening. The effective
set must be a nonempty subset: ALL may become CLIENTS or PLUGINS; an exclusive
caller may only remain itself. No mutable widening or per-listener second authority
exists. A product's own wire services also declare caller options. Consumers migrate
product packages to model/host/plugin with the next pin; this pass only updates the
example's imports so regeneration remains possible.

### Native Type Ownership

Storage remains Private and in-process. Products own typed file, upload, query,
record and object wire operations over those native calls. Model FileReference,
File, Upload, PageRequest and Receipt remain exchanged values, preserving their
field numbers and encodings. Plugins.Create resolves managed-bundle FileReference
through the existing in-process content owner. No storage wire API, new backend,
feature gate or alternate storage owner is introduced here.

### Application Operations in Discovery

The existing registry composes application and library operations; it needs no
second discovery inventory. Register each declaration once, using its executor
identity and effective callers. Discovery returns admitted contracts with their
fingerprints and projections. Dynamic client calls use Invocations.Create with
operation, fingerprint and payload; application typed clients come from the
application's descriptors and the same composition file.

### Consumer Work Lists

Each numbered item is one remaining integration unit. Regenerate after source
changes; never hand-edit generated code. Run focused proof per owner, then the
combined candidate's required gates. These lists do not claim consumer compilation.

#### Core Crates — CONSUMER WORK LIST (8 Items)

1. `crates/harmos-domain/src/{protobuf,schema}/**`, `crates/harmos-storage/src/**`
   and all `crates/**` wire imports: replace shared v1 with model; route carrier
   Request/Cancel/Response/Child*/Tick/Sample/assignment types to plugin and schema
   registry vocabulary to host. Delete SchemaInput consumers; no wire alias.
2. `crates/harmos-operations/src/{clock,lifecycle,discovery,plugins,service,access}/**`
   (including nested registry modules): replace client/plugin-host wrapper twins
   with host types. Rename Invoke declaration to Create; remove duplicate clock,
   lifecycle and discovery declarations/handlers and use clock::declarations_types.
   Keep one folder per resource and one operation file per method.
3. `crates/harmos-runtime/src/operations/**`, `src/grpc/**`: replace face-derived
   admission with executor routing plus method callers. Implement composition(&str)
   as above, refuse widening and wrong-caller dispatch (direct gRPC, managed,
   encoded and plugin child paths), retain visibility/grant checks; delete old
   client/context package heuristics and duplicated service registration.
4. `crates/harmos-runtime/src/plugin/{grpc,guest}/**`, `src/operations/manager/**`:
   use plugin carriers and assignments, host method identities and regenerated
   fingerprints. Preserve activation fencing, child correlation, tick lifetime,
   cancellation and failure preservation; delete plugin::host carrier paths.
5. `crates/harmos-runtime/src/plugins/**` and settings handlers under
   `crates/harmos-operations/src/plugins/**`: implement optional name resolution
   from trusted CallingPlugin, client-required selection and foreign-name refusal;
   return the richer List shape and remove the old Settings service adapter.
6. `crates/harmos-protocol/src/schema.rs` and `tests/integration/{audience_faces,
   generated_code_matches_proto,shared_primitives,schema_fingerprints}.rs`:
   migrate executor identity classification and descriptor expectations; remove
   old audience enum variants, duplicate wrapper comparison and client-clock file
   assertion. Replace them with option/projection/admission journeys; no prose or
   census-constant assertions. Protocol support imports already use model/plugin.
7. `crates/harmos-runtime/tests/{operations,plugin_grpc,grpc,plugins,guest}/**`,
   affected benches and fixtures: migrate IDs, messages and host registration;
   add deterministic caller rejection in both directions and narrowed ALL
   admission; preserve storage/query work owned by its concurrent change.
8. `docs/product-guide.md`, `README.md`, `docs/{architecture,rust-sdk,python-sdk,
   csharp-sdk,cpp-sdk}.md` and links to behaviour: replace old wire/module paths,
   request-object client examples and tonic Request/into_inner usage with public
   flattened calls. Keep README's SDK independence claim true and remove stale
   face-local claims. Coordinate storage text with its owner.

#### Rust SDK — CONSUMER WORK LIST (6 Items)

1. `sdk/rust/src/wire.rs`, `src/{context,connection,client,clock,operation}.rs`
   and submodules: private model/host/plugin imports; replace InvocationsInvoke
   with InvocationsCreate. Delete client/plugin-host wire package imports and
   duplicated clock graph use; keep the public harmos/client/plugin barrels.
2. `sdk/rust/src/{native,server,plugin,wasm,stream}.rs` and submodules: move carrier
   frames and assignments to plugin, use regenerated host IDs and fingerprints,
   preserve native/wasm admission, scope and tick-scoped children.
3. `sdk/rust/src/context.rs`, `src/plugin.rs`: use host.plugins.settings.list,
   PluginsSettingsList requirement and richer SettingsListResponse.value;
   omit name or use the caller's name. Delete old host.settings shortcuts and
   host-only context wrapper assumptions; keep whole-map client Set.
4. `sdk/rust/tests/{integration,system,wasm,support}/**`, binaries under src/bin
   and fixture consumers: migrate wrappers/IDs and add caller/narrowing journeys.
   Generated outputs are already replaced; do not restore deleted context bins.
5. `sdk/rust/Cargo.toml`, `examples/client.rs`: remove schematic-protocol
   dev-dependency and schematic.validate use. SDK client examples use only library
   calls. Put the application call under examples/schematic through its generated
   client; keep one separate short dynamic-call example using Invocations.Create.
6. `docs/rust-sdk.md`, `docs/product-guide.md`, `README.md`: flatten request arguments,
   hide protocol packages and remove old request construction/.into_inner examples;
   link the library-only, application typed and short dynamic examples separately.

#### Python SDK — CONSUMER WORK LIST (6 Items)

1. `sdk/python/src/harmos/{_authoring,_contracts,_connection,_invocation,_schema}.py`
   and related private modules: replace _wire.v1/client/plugin.host imports with
   model/host/plugin and use Invocations.Create; keep dotted public barrels.
2. `_invocation.py`, `_clock.py`, `_native.py` and carrier consumers: use plugin
   child/sample/assignment frames and host operation IDs; preserve fingerprint,
   activation, cancellation and tick lifetime behavior.
3. `_invocation.py` settings conveniences and examples: use
   context.host.plugins.settings.list, PLUGINS_SETTINGS_LIST and the richer
   value/revision/declarations/cut reply; delete old top-level Settings calls.
4. `sdk/python/tests/{integration,support,fixtures}/**`: migrate imports, fixture
   selectors and required IDs; add wrong-caller/narrowed-surface journeys without
   importing removed packages or exposing wire requests publicly.
5. `sdk/python/examples/**`: library SDK examples depend on library calls only;
   move application-operation calls under examples/schematic with its generated
   client, and keep one separate short dynamic Invocations.Create example.
6. `docs/python-sdk.md`, `docs/product-guide.md`: preserve flattened keyword-only
   calls, per-call context, payload projection and public imports; remove old
   application-dependent SDK example instructions and obsolete wire identities.

#### C# SDK — CONSUMER WORK LIST (6 Items)

1. `sdk/csharp/src/Harmos/{Context,Plugin,Operation,Client,Clock}.cs` and companion
   modules: replace old protocol namespace assumptions with generated public
   payloads and internal plugin carriers; keep Harmos/Clients/Plugins surfaces.
2. `Context.cs`, server/carrier handlers: use plugin frames/assignments and host
   identities, including Invocations.Create; retain host-only Instance admission,
   scope, typed failures, child cancellation and tick lifetime.
3. Settings conveniences/requirements: use Host.Plugins.Settings.List and
   HostOperation.PluginsSettingsList; read the richer Harmos.Clients payload.
   Delete top-level Host.Settings calls and duplicate response assumptions.
4. `sdk/csharp/tests/Harmos.Tests/**`, conformance fixtures: migrate generated
   imports, IDs and wrapper expectations; add caller/narrowing journeys. Do not
   hand-edit Generated/Client.g.cs, Host.g.cs or Wire/ outputs.
5. `sdk/csharp/examples/**`: SDK examples call library operations only; move
   application operations into examples/schematic through its generated client
   and retain a separate short dynamic Invocations.Create example.
6. `docs/csharp-sdk.md`, `docs/product-guide.md`: use flattened content arguments,
   owned CallOptions and payload returns; remove obsolete audience namespaces and
   application-dependent SDK examples.

#### C++ SDK — CONSUMER WORK LIST (6 Items)

1. `sdk/cpp/include/harmos/{harmos,context,client_transport}.hpp` and related
   detail headers: replace old generated package imports with model/host/plugin;
   keep harmos/client/plugin public namespaces and request privacy.
2. `sdk/cpp/src/{context,server,catalog}.cpp` and carrier code: move child/sample/
   assignment types to plugin, adopt host identities and Invocations.Create,
   preserve fingerprint, correlation, scope and failure semantics.
3. Settings conveniences/requirements: use host().plugins.settings.list and
   HostOperation::PluginsSettingsList; consume the richer shared root payload.
   Delete old settings resource calls and plugin-host wire wrappers.
4. `sdk/cpp/tests/**`, `examples/conformance/**`: migrate fixtures, protobuf include
   paths and namespace expectations; add caller/narrowing proof. CMake's maintained
   protocol inventory and generated resource/header sources are already replaced.
5. `sdk/cpp/examples/{client,plugin}/**`: library-only SDK calls; application calls
   move to examples/schematic through the application client; retain one short
   dynamic Invocations.Create example independently of application dependencies.
6. `docs/cpp-sdk.md`, `docs/product-guide.md`: preserve flattened arguments,
   owned transport, option values and payload returns; remove old wire packages
   and application-dependent SDK example instructions.

#### Conformance, Example and Fixtures — CONSUMER WORK LIST (8 Items)

1. `tools/conformance/src/{suite,reference,fixture}.rs`: replace model/plugin/host
   imports, old clock/Settings IDs and request/response names; retain all existing
   numbered cases and add cases 33/34 with independent result reporting.
2. `tools/conformance/src/suite.rs`, runtime-host fixtures and each SDK fixture:
   rule 33 registers a PLUGINS-only host fixture with an observable invocation
   counter; a client must be refused before the counter changes. Rule 34 sends
   a plugin child call to CLIENTS-only Invocations.Create and checks refusal with
   no dispatched invocation. Keep the fixture declarations deterministic.
3. `tools/conformance/tests/{integration,support}/**` and
   `sdk/tests/fixtures/{conformance,client}/proto/**`: migrate product executor
   packages and mandatory callers, keep one declared home and compose nested
   resources. Use product composition input for narrowed audience journeys.
4. `examples/schematic/proto/**`, `protocol/codegen.rs`, `protocol/src/**`:
   migrate the example's own model/host/plugin packages and service files,
   declare callers, add protocol/composition.toml and generate its own typed
   clients. Shared library imports and generated Rust messages already moved.
5. `examples/schematic/{app,operations,guest,sidecar,cli}/**`: migrate host operation
   IDs, plugin frames and public SDK calls. `app/tests/storage/main.rs` must call
   the operation registry with content arguments instead of request objects;
   use Invocations.Create for its dynamic client dispatch. Storage wrappers
   continue to call Private native operations, preserving storage-owner changes.
6. `examples/schematic/` client example: move schematic.validate out of
   `sdk/rust/examples/client.rs`, call it through the application's generated
   client, and remove SDK-to-example dependencies. Keep a separate short dynamic
   operation example, clearly distinct from the typed application client.
7. `docs/product-guide.md`, example guides and SDK guides: replace stale
   harmos_protocol::v1, tonic::Request::new/.into_inner and the schematic.validate
   SDK client example; link the application client and library dynamic example.
   Update behaviour links to proto/harmos/plugin/v1/behaviour.md.
8. All fixture-generated descriptors/bindings and tests: regenerate from source,
   verify a second generation is unchanged, then run focused consumer proof and
   the combined integration gate. Delete superseded face-specific fixtures,
   copied Storage wire contracts and old generated files; keep existing rules
   1–32 and preserve native storage encodings.

### Next Pin for Products That Vendor the Protocol

Replace the entire proto/harmos tree and regenerate all languages. Remove client/
and plugin/host/ folders and old shared v1 files. Apply the complete identity map,
mandatory callers and product narrowing input before enabling listeners. Products
own storage wrappers and application-specific publication; no library Storage or
Samples.Append service is recreated. Compose the library's host resources directly.

## Guest Entries and Reported Declarations

Every host-registered WebAssembly plugin has a durable `PluginEntry` keyed by its
registered identity with `PluginPlacement { guest: PluginGuest {} }`. Configure and
start the plugin entry manager before `PluginHost.register_guest`. Registration
materializes typed settings before guest initialization; the host
`plugins.Settings.List` uses the admitted guest generation's owner; name is optional
and may only equal that owner when supplied.
Client `Plugins.List/Get` and `Plugins.Settings.List/Set` use that same entry.
Deleting it removes settings in the same registry commit and retires the guest.
The product supplies and re-registers its module after host restart; guest placement
is a marker, not a second module store. `Plugins.Create` refuses guest placement
and directs the host to register the loaded module in-process. `Plugins.Replace`
remains managed-only; native guest reload keeps its existing entry and settings.

`Instance.Get`, `GuestRegistration` and `PluginDescription` report `Declarations`
through `declarations`; `PluginEntry.declarations_digest` identifies those bytes.
There is no Catalog alias. Fields retain their numbers. The four SDK builders use
`declarations()` (`Declarations()` in C#). Internal operation, object, state and
packaging registries retain catalog terminology; they do not name this wire payload.
Consumers must regenerate bindings, rename the reported message/fields and SDK
builder calls, and configure the entry manager before registering guest modules.

## SDK Call Contracts

All four typed client generators flatten request fields in declaration order. Required
fields are positional; definition-level optional fields and retry identity are optional.
Python uses keyword-only arguments; C# uses optional parameters. Rust exposes a plain
method and a `_with` method taking one generated options value; C++ defaults that options
value. A missing request identity is generated once per call. Explicit identities are
preserved for retry safety. Request messages belong only to private wire code; generic
operation dispatch remains message-based for application operations.

Calls create their transport context. Options permit metadata, cancellation and deadline
overrides. No SDK invents a default deadline; operation policy owns it. Plugin handlers
use `Context.host` (`host()` in Rust/C++) for typed Time, Plugins.Settings, Operations and
Invocations calls, declaring their requirements with `HostOperation`.

A response with exactly one declared field returns that field, including a repeated
collection. Streaming calls project every item by the same rule. Missing singular message
payloads refuse the read. Empty and multiple-field responses retain their response type:

- Client: Access.Listener.Get/Set; Access.Pairings.Create/List/Delete; Plugins.Get/List/
  Watch/Create/Set/Replace; Plugins.Settings.List/Set; Status.Get/Watch; Status.Diagnostics.Get;
  Status.Logs.List/Watch; Status.TraceEvents.List/Watch; Time.Sleep.
- Plugin context: Plugins.Settings.List and Time.Sleep.

Status.Get and Status.Watch retain both status and resolved read-cut metadata. The direct
HostStatus return shown in the requested example requires an owner decision about that
metadata; the single-field rule currently preserves it. Optional read cuts and pages are
marked in the protocol definitions rather than inferred by parameter name. The optional
Plugins.Watch name selects all entries when absent.

Product generators merge descriptors, keep their own protocol modules private and export
response types through a client barrel. The shared composition fixture proves a product
Annotations resource beneath library Plugins alongside the sibling Jobs.Progress root.
Python composed roots inherit Client connection ownership and preserve typed resource
properties. No application-side resource grafting is required.
