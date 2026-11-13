# Architecture

Transfers remain unary, amended by the owner on 2026-11-13. The generated
`StorageTransferSettings.chunk_bytes` under `StorageSettings.transfer` owns the
single 4 MiB default for append and ranged-read sizes on both faces. Generated
`MessageSettings.message_bytes` declares the separate 8 MiB message default once.
Client SDKs, listeners, plugin carriers and Wasm guests use it and refuse ceilings
that cannot carry the configured chunk plus framing at startup, naming the action and
setting. Streaming transfer handlers, proto requests, bindings, SDK surfaces,
stories and benchmark cases are deleted; Watch and native readers remain.

Application-owned immutable content registers through StorageManager's single
`ContentSource`. Resolution receives the inherited caller scope and returns length,
SHA-256 and a bounded lease with explicit expiry. Client/plugin GetFileBytes retain
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
without bound are paged. CatalogEntries on both faces, Plugins and Access.Pairings
return the complete set without page requests, tokens or application byte bounds.
The carrier message ceiling is their sole response guard; overflow names the action.
Records, samples, log lines, trace events, files and rows retain position-resolved paging.

## Protocol Source Convention

Author maintained files under `proto/harmos/` with this skeleton, including empty
sections. Titles use Title Case. Buf owns canonical layout and preserves these
detached section comments without formatter configuration. Empty messages have
no interior comments; Buf renders their bodies as `{}`.

```proto
// SPDX-License-Identifier: Apache-2.0

// <Title>
//
// <One or two lines describing the resource's purpose.>

syntax = "proto3";

package <package>;

// === Imports === //

// === Options === //

// === Services === //

// === Requests and Responses === //

// === Resources === //

// === Enums === //
```

Keep each request directly before its response in service method order, shared
payloads in Resources and enums last. Use one field per line. Keep one service
per file; a layout-only change must preserve existing file boundaries even when
that prevents meeting this convention. `storage_settings.proto` intentionally
retains proto2: its field presence and explicit defaults are protocol contracts.
Formatting never changes packages, declarations, names, numbers, types or options.
Layout-only descriptor comparisons remove source information, sort each file's
imports and sort only its top-level messages and enums by name. All other
descriptor bytes must match, including service method order, message field order,
nested declaration order, enum values and options. Compare the complete file-name
set before comparing each normalized file descriptor.

Every service, call and message has a contract comment. Document opaque identities,
revisions, positions, cuts, continuations, capabilities, limits, optional/unset
behavior and discriminated variants. Explain refusal and replay where the owning
handler defines them. Derive comments from implemented handlers and consumers,
their deterministic stories, and the maintained design; a design decision alone
does not establish that behavior has landed. When a handler only forwards metadata,
describe that boundary rather than inventing the downstream owner's defaults.
Request and response comments describe their payloads rather than repeating the
call's instructions; an empty payload needs only the call name. File purposes name
the audience: applications or tools call the host through `harmos/v1`, the host
calls a plugin through `harmos/plugin/v1`, and a running plugin calls back to the
host from inside an invocation through `harmos/context/v1`, answered for that plugin.
For example, `Samples.Append` forwards absent sequence/context/timestamp metadata;
its handler does not assign defaults or interpret following-row duration metadata.

The root `buf.yaml` selects only maintained Harmos inputs, excluding application
and third-party sources. `mise run format` formats them and honors `--check`;
`mise run check -- --repository` checks formatting and runs Buf lint. Standard
lint plus required service/call/message comments keeps the locked request/response
grammar; each exception in `buf.yaml` names the incompatible existing declaration
shape. Regenerate Rust and Python outputs with `mise run generate -- --sdk rust`
and `mise run generate -- --sdk python`; build the Rust generator example through
the focused build task when its executable is absent. The focused
`generated_code_matches_proto::generated_code_matches_proto` protocol integration
test compares the committed Rust sources and descriptor projections byte for byte.

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
   checks face-local service and wrapper descriptors. `plugin_cannot_call_a_client_face_even_with_its_declared_contract`
   and `client_invokes_public_plugin_through_the_host_and_protected_is_refused`
   guard native admission, including absence of Instance/context on the client listener.
   Storage and management descriptors use the same complete SDK generator;
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
| Audience faces and face-local contracts (2026-11-12) | Separate who serves/calls a wire API without removing operation visibility. | `openspec/changes/operation-grammar/` |
| Runtime composition, manager inventory and product-owned settings/secrets (2026-11-12) | Reuse generic machinery under one lifecycle authority while retaining application policy. | `openspec/changes/operation-grammar/` |
| Resource grammar, nested packages and shared primitives (2026-11-12) | Generate predictable SDKs without flattening storage guarantees. | `openspec/changes/operation-grammar/` |
| Latest durable reads, measured lag and caller resending (2026-11-12) | Repeatable reads and lost replies must preserve durable boundaries and original outcomes. | `openspec/changes/operation-grammar/` |
| No automatic twins; distinct Stop and Cancel (2026-11-12) | Keep one method per promise and preserve cooperative final results. | `openspec/changes/operation-grammar/` |
| Host-only control uses Protected | Keep host controls on the instance carrier and refuse peer/client calls. | `openspec/changes/operation-grammar/` |
| Retained plugin history and derived revisions (2026-11-07) | Written schema identity must outlive plugin replacement or removal. | `openspec/changes/archive/2026-11-07-plugin-state-survives-shape-changes/`, `openspec/changes/archive/2026-11-07-stream-schema-revisions/` |
| Segmented recovery snapshots (2026-11-08) | Bounded checkpoint pieces must preserve complete recovery and atomic publication. | `openspec/changes/archive/2026-11-08-snapshot-payload-limit/` |
| Deterministic stories (agreed 2026-11-10) | Correctness must not depend on workstation speed or measurement noise. | `openspec/changes/test-stories/` |

## Protocol Boundary Definition and Consumer Migration

This definition pass implements the owner amendments of 2026-11-13. The declarations
and generated outputs below are the new contract; hand-written consumers are intentionally
unmigrated and do not compile yet. This section supersedes earlier package paths, wire
storage placement, registry resources and settings ownership in this guide. Historical
runtime proof elsewhere in this document applies to the preceding implementation until
the named workers migrate it. No implementation or full-project proof is implied.

The descriptor census is client 12 services/59 calls → 14/33, plugin-host 6/40 →
4/8, and plugin 1/4 → 1/4. Shared `harmos.v1` has zero services.

### Call Mapping

The former and final request/response columns contain fully qualified message names.
`—` means no library wire wrapper survives. Nested package segments compose resource
parents (`access`, `status`, `plugins`) rather than adding root Access RPCs.

| Former Identity | Former Request / Response | Final Identity | Final Request / Response | Boundary |
| --- | --- | --- | --- | --- |
| `harmos.context.v1.CatalogEntries/List` (`operations`) | `harmos.context.v1.CatalogEntriesListRequest` / `harmos.context.v1.CatalogEntriesListResponse` | `harmos.plugin.host.v1.Operations/List` | `harmos.plugin.host.v1.OperationsListRequest` / `harmos.plugin.host.v1.OperationsListResponse` | Plugin-Host |
| `harmos.context.v1.CatalogEntries/List` (`schemas`) | `harmos.context.v1.CatalogEntriesListRequest` / `harmos.context.v1.CatalogEntriesListResponse` | removed | — | Plugin-Host |
| `harmos.context.v1.CatalogEntries/List` (`streams`) | `harmos.context.v1.CatalogEntriesListRequest` / `harmos.context.v1.CatalogEntriesListResponse` | removed | — | Plugin-Host |
| `harmos.context.v1.CatalogEntries/List` (`objects`) | `harmos.context.v1.CatalogEntriesListRequest` / `harmos.context.v1.CatalogEntriesListResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.HostValues/Get` | `harmos.context.v1.HostValuesGetRequest` / `harmos.context.v1.HostValuesGetResponse` | removed | — | Plugin-Host |
| `harmos.context.v1.Invocations/Get` | `harmos.context.v1.InvocationsGetRequest` / `harmos.context.v1.InvocationsGetResponse` | `harmos.plugin.host.v1.Invocations/Get` | `harmos.plugin.host.v1.InvocationsGetRequest` / `harmos.plugin.host.v1.InvocationsGetResponse` | Plugin-Host |
| `harmos.context.v1.Invocations/Cancel` | `harmos.context.v1.InvocationsCancelRequest` / `harmos.context.v1.InvocationsCancelResponse` | `harmos.plugin.host.v1.Invocations/Cancel` | `harmos.plugin.host.v1.InvocationsCancelRequest` / `harmos.plugin.host.v1.InvocationsCancelResponse` | Plugin-Host |
| `harmos.context.v1.Invocations/Watch` | `harmos.context.v1.InvocationsWatchRequest` / `harmos.context.v1.InvocationsWatchResponse` | `harmos.plugin.host.v1.Invocations/Watch` | `harmos.plugin.host.v1.InvocationsWatchRequest` / `harmos.plugin.host.v1.InvocationsWatchResponse` | Plugin-Host |
| `harmos.context.v1.Samples/Append` | `harmos.context.v1.SamplesAppendRequest` / `harmos.context.v1.SamplesAppendResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/CreateFileProduction` | `harmos.context.v1.StorageCreateFileProductionRequest` / `harmos.context.v1.StorageCreateFileProductionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetFileProduction` | `harmos.context.v1.StorageGetFileProductionRequest` / `harmos.context.v1.StorageGetFileProductionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/AppendFileBytes` | `harmos.context.v1.StorageAppendFileBytesRequest` / `harmos.context.v1.StorageAppendFileBytesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/CreateFileWriter` | `harmos.context.v1.StorageCreateFileWriterRequest` / `harmos.context.v1.StorageCreateFileWriterResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/PublishFileProduction` | `harmos.context.v1.StoragePublishFileProductionRequest` / `harmos.context.v1.StoragePublishFileProductionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/DeleteFileProduction` | `harmos.context.v1.StorageDeleteFileProductionRequest` / `harmos.context.v1.StorageDeleteFileProductionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetFileBytes` | `harmos.context.v1.StorageGetFileBytesRequest` / `harmos.context.v1.StorageGetFileBytesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/Capabilities` | `harmos.context.v1.StorageCapabilitiesRequest` / `harmos.context.v1.StorageCapabilitiesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/CreateSnapshot` | `harmos.context.v1.StorageCreateSnapshotRequest` / `harmos.context.v1.StorageCreateSnapshotResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/DeleteSnapshot` | `harmos.context.v1.StorageDeleteSnapshotRequest` / `harmos.context.v1.StorageDeleteSnapshotResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/ListColumnRows` | `harmos.context.v1.StorageListColumnRowsRequest` / `harmos.context.v1.StorageListColumnRowsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/ListIndexEntries` | `harmos.context.v1.StorageListIndexEntriesRequest` / `harmos.context.v1.StorageListIndexEntriesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/SetIndexDefinition` | `harmos.context.v1.StorageSetIndexDefinitionRequest` / `harmos.context.v1.StorageSetIndexDefinitionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/SetProjectionDefinition` | `harmos.context.v1.StorageSetProjectionDefinitionRequest` / `harmos.context.v1.StorageSetProjectionDefinitionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/AdvanceProjection` | `harmos.context.v1.StorageAdvanceProjectionRequest` / `harmos.context.v1.StorageAdvanceProjectionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/ListProjectionRows` | `harmos.context.v1.StorageListProjectionRowsRequest` / `harmos.context.v1.StorageListProjectionRowsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetProjectionStatus` | `harmos.context.v1.StorageGetProjectionStatusRequest` / `harmos.context.v1.StorageGetProjectionStatusResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/SetObject` | `harmos.context.v1.StorageSetObjectRequest` / `harmos.context.v1.StorageSetObjectResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/ListRows` | `harmos.context.v1.StorageListRowsRequest` / `harmos.context.v1.StorageListRowsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/WatchRows` | `harmos.context.v1.StorageWatchRowsRequest` / `harmos.context.v1.StorageWatchRowsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/AppendRecords` | `harmos.context.v1.StorageAppendRecordsRequest` / `harmos.context.v1.StorageAppendRecordsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/AppendSamples` | `harmos.context.v1.StorageAppendSamplesRequest` / `harmos.context.v1.StorageAppendSamplesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/AppendObjectRevisions` | `harmos.context.v1.StorageAppendObjectRevisionsRequest` / `harmos.context.v1.StorageAppendObjectRevisionsResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetFileHash` | `harmos.context.v1.StorageGetFileHashRequest` / `harmos.context.v1.StorageGetFileHashResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/CaptureFile` | `harmos.context.v1.StorageCaptureFileRequest` / `harmos.context.v1.StorageCaptureFileResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/WatchFileBytes` | `harmos.context.v1.StorageWatchFileBytesRequest` / `harmos.context.v1.StorageWatchFileBytesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/WatchFileProduction` | `harmos.context.v1.StorageWatchFileProductionRequest` / `harmos.context.v1.StorageWatchFileProductionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/ListDirectoryEntries` | `harmos.context.v1.StorageListDirectoryEntriesRequest` / `harmos.context.v1.StorageListDirectoryEntriesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/WatchDirectoryEntries` | `harmos.context.v1.StorageWatchDirectoryEntriesRequest` / `harmos.context.v1.StorageWatchDirectoryEntriesResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetIndexDefinition` | `harmos.context.v1.StorageGetIndexDefinitionRequest` / `harmos.context.v1.StorageGetIndexDefinitionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetProjectionDefinition` | `harmos.context.v1.StorageGetProjectionDefinitionRequest` / `harmos.context.v1.StorageGetProjectionDefinitionResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Storage/GetObject` | `harmos.context.v1.StorageGetObjectRequest` / `harmos.context.v1.StorageGetObjectResponse` | removed from the wire (in-process only) | — | Plugin-Host |
| `harmos.context.v1.Time/Get` | `harmos.context.v1.TimeGetRequest` / `harmos.context.v1.TimeGetResponse` | `harmos.plugin.host.v1.Time/Get` | `harmos.plugin.host.v1.TimeGetRequest` / `harmos.plugin.host.v1.TimeGetResponse` | Plugin-Host |
| `harmos.context.v1.Time/Sleep` | `harmos.context.v1.TimeSleepRequest` / `harmos.context.v1.TimeSleepResponse` | `harmos.plugin.host.v1.Time/Sleep` | `harmos.plugin.host.v1.TimeSleepRequest` / `harmos.plugin.host.v1.TimeSleepResponse` | Plugin-Host |
| `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.InstanceGetRequest` / `harmos.plugin.v1.InstanceGetResponse` | `harmos.plugin.v1.Instance/Get` | `harmos.plugin.v1.InstanceGetRequest` / `harmos.plugin.v1.InstanceGetResponse` | Plugin |
| `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.InstanceInvokeRequest` / `harmos.plugin.v1.InstanceInvokeResponse` | `harmos.plugin.v1.Instance/Invoke` | `harmos.plugin.v1.InstanceInvokeRequest` / `harmos.plugin.v1.InstanceInvokeResponse` | Plugin |
| `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.InstanceSamplesRequest` / `harmos.plugin.v1.InstanceSamplesResponse` | `harmos.plugin.v1.Instance/Samples` | `harmos.plugin.v1.InstanceSamplesRequest` / `harmos.plugin.v1.InstanceSamplesResponse` | Plugin; additive tick-scoped children: request `child = 3`; response `child = 2`, `child_cancel = 3`, `child_demand = 4`; existing fields preserved |
| `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.InstanceStopRequest` / `harmos.plugin.v1.InstanceStopResponse` | `harmos.plugin.v1.Instance/Stop` | `harmos.plugin.v1.InstanceStopRequest` / `harmos.plugin.v1.InstanceStopResponse` | Plugin |
| `harmos.v1.access.Certificates/Create` | `harmos.v1.access.CertificatesCreateRequest` / `harmos.v1.access.CertificatesCreateResponse` | `harmos.client.v1.access.Certificates/Create` | `harmos.client.v1.access.CertificatesCreateRequest` / `harmos.client.v1.access.CertificatesCreateResponse` | Client |
| `harmos.v1.access.Listener/Get` | `harmos.v1.access.ListenerGetRequest` / `harmos.v1.access.ListenerGetResponse` | `harmos.client.v1.access.Listener/Get` | `harmos.client.v1.access.ListenerGetRequest` / `harmos.client.v1.access.ListenerGetResponse` | Client |
| `harmos.v1.access.Listener/Set` | `harmos.v1.access.ListenerSetRequest` / `harmos.v1.access.ListenerSetResponse` | `harmos.client.v1.access.Listener/Set` | `harmos.client.v1.access.ListenerSetRequest` / `harmos.client.v1.access.ListenerSetResponse` | Client |
| `harmos.v1.access.Pairings/Create` | `harmos.v1.access.PairingsCreateRequest` / `harmos.v1.access.PairingsCreateResponse` | `harmos.client.v1.access.Pairings/Create` | `harmos.client.v1.access.PairingsCreateRequest` / `harmos.client.v1.access.PairingsCreateResponse` | Client |
| `harmos.v1.access.Pairings/List` | `harmos.v1.access.PairingsListRequest` / `harmos.v1.access.PairingsListResponse` | `harmos.client.v1.access.Pairings/List` | `harmos.client.v1.access.PairingsListRequest` / `harmos.client.v1.access.PairingsListResponse` | Client |
| `harmos.v1.access.Pairings/Delete` | `harmos.v1.access.PairingsDeleteRequest` / `harmos.v1.access.PairingsDeleteResponse` | `harmos.client.v1.access.Pairings/Delete` | `harmos.client.v1.access.PairingsDeleteRequest` / `harmos.client.v1.access.PairingsDeleteResponse` | Client |
| `harmos.v1.CatalogEntries/List` (`operations`) | `harmos.v1.CatalogEntriesListRequest` / `harmos.v1.CatalogEntriesListResponse` | `harmos.client.v1.Operations/List` | `harmos.client.v1.OperationsListRequest` / `harmos.client.v1.OperationsListResponse` | Client |
| `harmos.v1.CatalogEntries/List` (`schemas`) | `harmos.v1.CatalogEntriesListRequest` / `harmos.v1.CatalogEntriesListResponse` | `harmos.client.v1.Schemas/List` | `harmos.client.v1.SchemasListRequest` / `harmos.client.v1.SchemasListResponse` | Client |
| `harmos.v1.CatalogEntries/List` (`streams`) | `harmos.v1.CatalogEntriesListRequest` / `harmos.v1.CatalogEntriesListResponse` | `harmos.client.v1.Streams/List` | `harmos.client.v1.StreamsListRequest` / `harmos.client.v1.StreamsListResponse` | Client |
| `harmos.v1.CatalogEntries/List` (`objects`) | `harmos.v1.CatalogEntriesListRequest` / `harmos.v1.CatalogEntriesListResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Invocations/Invoke` | `harmos.v1.InvocationsInvokeRequest` / `harmos.v1.InvocationsInvokeResponse` | `harmos.client.v1.Invocations/Invoke` | `harmos.client.v1.InvocationsInvokeRequest` / `harmos.client.v1.InvocationsInvokeResponse` | Client |
| `harmos.v1.Invocations/Get` | `harmos.v1.InvocationsGetRequest` / `harmos.v1.InvocationsGetResponse` | `harmos.client.v1.Invocations/Get` | `harmos.client.v1.InvocationsGetRequest` / `harmos.client.v1.InvocationsGetResponse` | Client |
| `harmos.v1.Invocations/Cancel` | `harmos.v1.InvocationsCancelRequest` / `harmos.v1.InvocationsCancelResponse` | `harmos.client.v1.Invocations/Cancel` | `harmos.client.v1.InvocationsCancelRequest` / `harmos.client.v1.InvocationsCancelResponse` | Client |
| `harmos.v1.Invocations/Watch` | `harmos.v1.InvocationsWatchRequest` / `harmos.v1.InvocationsWatchResponse` | `harmos.client.v1.Invocations/Watch` | `harmos.client.v1.InvocationsWatchRequest` / `harmos.client.v1.InvocationsWatchResponse` | Client |
| `harmos.v1.Plugins/Get` | `harmos.v1.PluginsGetRequest` / `harmos.v1.PluginsGetResponse` | `harmos.client.v1.Plugins/Get` | `harmos.client.v1.PluginsGetRequest` / `harmos.client.v1.PluginsGetResponse` | Client |
| `harmos.v1.Plugins/List` | `harmos.v1.PluginsListRequest` / `harmos.v1.PluginsListResponse` | `harmos.client.v1.Plugins/List` | `harmos.client.v1.PluginsListRequest` / `harmos.client.v1.PluginsListResponse` | Client |
| `harmos.v1.Plugins/Watch` | `harmos.v1.PluginsWatchRequest` / `harmos.v1.PluginsWatchResponse` | `harmos.client.v1.Plugins/Watch` | `harmos.client.v1.PluginsWatchRequest` / `harmos.client.v1.PluginsWatchResponse` | Client |
| `harmos.v1.Plugins/Create` | `harmos.v1.PluginsCreateRequest` / `harmos.v1.PluginsCreateResponse` | `harmos.client.v1.Plugins/Create` | `harmos.client.v1.PluginsCreateRequest` / `harmos.client.v1.PluginsCreateResponse` | Client |
| `harmos.v1.Plugins/Set` | `harmos.v1.PluginsSetRequest` / `harmos.v1.PluginsSetResponse` | `harmos.client.v1.Plugins/Set` | `harmos.client.v1.PluginsSetRequest` / `harmos.client.v1.PluginsSetResponse` | Client |
| `harmos.v1.Plugins/Delete` | `harmos.v1.PluginsDeleteRequest` / `harmos.v1.PluginsDeleteResponse` | `harmos.client.v1.Plugins/Delete` | `harmos.client.v1.PluginsDeleteRequest` / `harmos.client.v1.PluginsDeleteResponse` | Client |
| `harmos.v1.Plugins/Restart` | `harmos.v1.PluginsRestartRequest` / `harmos.v1.PluginsRestartResponse` | `harmos.client.v1.Plugins/Restart` | `harmos.client.v1.PluginsRestartRequest` / `harmos.client.v1.PluginsRestartResponse` | Client |
| `harmos.v1.Status/Get` | `harmos.v1.StatusGetRequest` / `harmos.v1.StatusGetResponse` | `harmos.client.v1.Status/Get` | `harmos.client.v1.StatusGetRequest` / `harmos.client.v1.StatusGetResponse` | Client |
| `harmos.v1.Status/Watch` | `harmos.v1.StatusWatchRequest` / `harmos.v1.StatusWatchResponse` | `harmos.client.v1.Status/Watch` | `harmos.client.v1.StatusWatchRequest` / `harmos.client.v1.StatusWatchResponse` | Client |
| `harmos.v1.status.Diagnostics/Get` | `harmos.v1.status.DiagnosticsGetRequest` / `harmos.v1.status.DiagnosticsGetResponse` | `harmos.client.v1.status.Diagnostics/Get` | `harmos.client.v1.status.DiagnosticsGetRequest` / `harmos.client.v1.status.DiagnosticsGetResponse` | Client |
| `harmos.v1.status.Logs/List` | `harmos.v1.status.LogsListRequest` / `harmos.v1.status.LogsListResponse` | `harmos.client.v1.status.Logs/List` | `harmos.client.v1.status.LogsListRequest` / `harmos.client.v1.status.LogsListResponse` | Client |
| `harmos.v1.status.Logs/Watch` | `harmos.v1.status.LogsWatchRequest` / `harmos.v1.status.LogsWatchResponse` | `harmos.client.v1.status.Logs/Watch` | `harmos.client.v1.status.LogsWatchRequest` / `harmos.client.v1.status.LogsWatchResponse` | Client |
| `harmos.v1.status.TraceEvents/List` | `harmos.v1.status.TraceEventsListRequest` / `harmos.v1.status.TraceEventsListResponse` | `harmos.client.v1.status.TraceEvents/List` | `harmos.client.v1.status.TraceEventsListRequest` / `harmos.client.v1.status.TraceEventsListResponse` | Client |
| `harmos.v1.status.TraceEvents/Watch` | `harmos.v1.status.TraceEventsWatchRequest` / `harmos.v1.status.TraceEventsWatchResponse` | `harmos.client.v1.status.TraceEvents/Watch` | `harmos.client.v1.status.TraceEventsWatchRequest` / `harmos.client.v1.status.TraceEventsWatchResponse` | Client |
| `harmos.v1.Storage/CreateFileProduction` | `harmos.v1.StorageCreateFileProductionRequest` / `harmos.v1.StorageCreateFileProductionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetFileProduction` | `harmos.v1.StorageGetFileProductionRequest` / `harmos.v1.StorageGetFileProductionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/AppendFileBytes` | `harmos.v1.StorageAppendFileBytesRequest` / `harmos.v1.StorageAppendFileBytesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/CreateFileWriter` | `harmos.v1.StorageCreateFileWriterRequest` / `harmos.v1.StorageCreateFileWriterResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/PublishFileProduction` | `harmos.v1.StoragePublishFileProductionRequest` / `harmos.v1.StoragePublishFileProductionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/DeleteFileProduction` | `harmos.v1.StorageDeleteFileProductionRequest` / `harmos.v1.StorageDeleteFileProductionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetFileBytes` | `harmos.v1.StorageGetFileBytesRequest` / `harmos.v1.StorageGetFileBytesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/Capabilities` | `harmos.v1.StorageCapabilitiesRequest` / `harmos.v1.StorageCapabilitiesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/CreateSnapshot` | `harmos.v1.StorageCreateSnapshotRequest` / `harmos.v1.StorageCreateSnapshotResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/DeleteSnapshot` | `harmos.v1.StorageDeleteSnapshotRequest` / `harmos.v1.StorageDeleteSnapshotResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/ListColumnRows` | `harmos.v1.StorageListColumnRowsRequest` / `harmos.v1.StorageListColumnRowsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/ListIndexEntries` | `harmos.v1.StorageListIndexEntriesRequest` / `harmos.v1.StorageListIndexEntriesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/SetIndexDefinition` | `harmos.v1.StorageSetIndexDefinitionRequest` / `harmos.v1.StorageSetIndexDefinitionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/SetProjectionDefinition` | `harmos.v1.StorageSetProjectionDefinitionRequest` / `harmos.v1.StorageSetProjectionDefinitionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/AdvanceProjection` | `harmos.v1.StorageAdvanceProjectionRequest` / `harmos.v1.StorageAdvanceProjectionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/ListProjectionRows` | `harmos.v1.StorageListProjectionRowsRequest` / `harmos.v1.StorageListProjectionRowsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetProjectionStatus` | `harmos.v1.StorageGetProjectionStatusRequest` / `harmos.v1.StorageGetProjectionStatusResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/SetObject` | `harmos.v1.StorageSetObjectRequest` / `harmos.v1.StorageSetObjectResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/ListRows` | `harmos.v1.StorageListRowsRequest` / `harmos.v1.StorageListRowsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/WatchRows` | `harmos.v1.StorageWatchRowsRequest` / `harmos.v1.StorageWatchRowsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/AppendRecords` | `harmos.v1.StorageAppendRecordsRequest` / `harmos.v1.StorageAppendRecordsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/AppendSamples` | `harmos.v1.StorageAppendSamplesRequest` / `harmos.v1.StorageAppendSamplesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/AppendObjectRevisions` | `harmos.v1.StorageAppendObjectRevisionsRequest` / `harmos.v1.StorageAppendObjectRevisionsResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetFileHash` | `harmos.v1.StorageGetFileHashRequest` / `harmos.v1.StorageGetFileHashResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/CaptureFile` | `harmos.v1.StorageCaptureFileRequest` / `harmos.v1.StorageCaptureFileResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/WatchFileBytes` | `harmos.v1.StorageWatchFileBytesRequest` / `harmos.v1.StorageWatchFileBytesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/WatchFileProduction` | `harmos.v1.StorageWatchFileProductionRequest` / `harmos.v1.StorageWatchFileProductionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/ListDirectoryEntries` | `harmos.v1.StorageListDirectoryEntriesRequest` / `harmos.v1.StorageListDirectoryEntriesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/WatchDirectoryEntries` | `harmos.v1.StorageWatchDirectoryEntriesRequest` / `harmos.v1.StorageWatchDirectoryEntriesResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetIndexDefinition` | `harmos.v1.StorageGetIndexDefinitionRequest` / `harmos.v1.StorageGetIndexDefinitionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetProjectionDefinition` | `harmos.v1.StorageGetProjectionDefinitionRequest` / `harmos.v1.StorageGetProjectionDefinitionResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Storage/GetObject` | `harmos.v1.StorageGetObjectRequest` / `harmos.v1.StorageGetObjectResponse` | removed from the wire (in-process only) | — | Client |
| `harmos.v1.Time/Get` | `harmos.v1.TimeGetRequest` / `harmos.v1.TimeGetResponse` | `harmos.client.v1.Time/Get` | `harmos.client.v1.TimeGetRequest` / `harmos.client.v1.TimeGetResponse` | Client |
| `harmos.v1.Time/Sleep` | `harmos.v1.TimeSleepRequest` / `harmos.v1.TimeSleepResponse` | `harmos.client.v1.Time/Sleep` | `harmos.client.v1.TimeSleepRequest` / `harmos.client.v1.TimeSleepResponse` | Client |

New calls have no former identity:

| Former Identity | Final Identity | Request / Response | Boundary |
| --- | --- | --- | --- |
| — | `harmos.client.v1.Operations/Get` | `harmos.client.v1.OperationsGetRequest` / `harmos.client.v1.OperationsGetResponse` | Client |
| — | `harmos.plugin.host.v1.Operations/Get` | `harmos.plugin.host.v1.OperationsGetRequest` / `harmos.plugin.host.v1.OperationsGetResponse` | Plugin-Host |
| — | `harmos.client.v1.Plugins/Replace` | `harmos.client.v1.PluginsReplaceRequest` / `harmos.client.v1.PluginsReplaceResponse` | Client |
| — | `harmos.client.v1.plugins.Settings/List` | `harmos.client.v1.plugins.SettingsListRequest` / `harmos.client.v1.plugins.SettingsListResponse` | Client |
| — | `harmos.client.v1.plugins.Settings/Set` | `harmos.client.v1.plugins.SettingsSetRequest` / `harmos.client.v1.plugins.SettingsSetResponse` | Client |
| — | `harmos.plugin.host.v1.Settings/List` | `harmos.plugin.host.v1.SettingsListRequest` / `harmos.plugin.host.v1.SettingsListResponse` | Plugin-Host |

### Definition Decisions

- Keep `harmos.v1.Catalog` and the `catalog` fields as the plugin-reported description. `Instance.Get`, guest registration and managed-entry `PluginDescription` still report operations, requirements, streams, opaque product sections, digest and reachable types (fields 1–6 unchanged). The removed resource was registry browsing, not this activation admission payload. Handlers in `plugin/grpc`, SDK authoring and `schema::catalog_digest` consume all these fields.
- Keep `SettingsDeclarations` as the typed contribution section in `Catalog.sections`; its declarations now contain name, title, description and typed default. No second direct settings inventory is added to Catalog or its digest. Settings are library-owned; secrets remain product sections and product operations.
- `Settings.Set` replaces one complete `SettingValues` map for the named plugin entry. List returns the complete map, with defaults materialized; all declared names and their value variants must match. Settings have their own opaque revision and issued read cut. Plugin-host List has no plugin selector and no change notification. Replay lifetime and persistence are implementation work under the existing manager receipt policy.
- Replace takes `PluginPlacement`, constrained to managed-to-managed replacement. It preserves entry name, settings and product data; failed staging preserves the previous serving bundle. `PluginChange.deleted` remains the sole wire deletion signal. Deleting an entry removes its settings atomically; product cleanup follows the in-process watch and startup reconciliation and is eventual.
- Operations.Get selects one operation by its complete ID and returns the same `DiscoveredOperation` metadata used in List. Discovery has no invented read cut: the current handler issues none.
- Schemas.List preserves the actual `StorageSchema` position/key/graph; Streams.List preserves `StoredIdentity::Stream` schema/scope/item/stream/first. The shared schema graph is registry vocabulary, not storage operations. Object identities have no library wire listing. No SDK reads persisted schemas or streams today, so neither service is added to the plugin-host boundary.
- `FileReference.position` now uses shared `Position`/`History`, preserving the position fields and encoding. `File` wraps the immutable reference. `Upload` carries production ID/history, writer generation, accepted/durable lengths and state; product wrappers supply operations and policy. Shared proto2 transfer defaults remain in `transfer.proto` because all SDK carriers use the previously locked chunk, message and framing declarations.
- Python client wire modules use `harmos._wire.client.v1` to avoid the authored `harmos.client` module. Wire identities remain `harmos.client.v1`; shared and plugin module paths follow the proto folders. The SDK handler keeps the name context.
- Access remains a generated parent node of Listener, Pairings and Certificates, with no new empty root service. Nested C# namespaces use `AccessResources`, `StatusResources` and `PluginsResources`.
- Root Buf now checks the library module independently. The Schematic proto imports the deleted plugin-host storage request; its worker must migrate that import and restore its independent Buf validation. Library lint rules were not weakened. The only remaining exceptions are service suffixes, persisted schema enum prefixes/zero names, and version suffixes on nested resource packages.

### Native Type Ownership

The old generated wire types are also the current in-process request types:
`harmos-operations/src/storage/client.rs` and `storage/context.rs` use the 32
`Storage<Method>Request/Response` pairs shown above as `ProtobufOperation` associated
types; they are not independent native models. The runtime worker must replace these
with one native request/result owner per storage operation and remove both face
projections, duplicate registrations, protobuf coupling and Public visibility. Native
storage operations become Private. Preserve mutation encodings, replay, receipt
positions, durability, scoped content leases, retained reads and all meaningful native
stories. A product wrapper is a declaration and one call through a small native helper;
the helper is the operations/runtime worker’s implementation responsibility.

Four more currently native-only pairs also live in the deleted wire files and must
be owned natively: `StorageInfoRequest/Response`, `StorageDeleteFileRequest/Response`,
`StorageSetSchemaRequest/Response`, `StorageGetSchemaRequest/Response`. The latter
schema operations currently encode cursor revisions; retain those bytes explicitly
rather than letting removal of Prost types change persistence. Removed nested outcomes
`StorageSetObjectResponse::{Applied, Conflict}` likewise become native results.

The following retired payload/enum declarations are not a new wire home or aliases.
Reuse existing native domain/storage values where present, and make the remaining
in-process representations native in their existing owner:

`StoreHistory`, `StorageCursor`, `MutationWindow`, `MutationId`, `StagedFile`, `FileSession`, `FileProducer`, `FileProgress`,
`DirectoryRule`, `DirectorySelection`, `DirectoryEntry`, `DirectoryCompletion`, `DirectoryRename`, `Empty`, `DirectoryEvent`, `FileSource`,
`ByteRange`, `FileBatch`, `FileCompletion`, `FileHash`, `FollowStart`, `FileEvent`, `RowSlice`, `ObjectSelection`,
`RecordSelection`, `StreamSelection`, `SchemaSelection`, `FileSelection`, `QuerySource`, `QueryCompletion`, `QueryOutput`, `FieldPath`,
`ExactNumber`, `ScalarValue`, `Comparison`, `ScalarValues`, `Condition`, `Term`, `Clause`, `Filter`,
`Projection`, `Project`, `Statistic`, `Measure`, `Aggregate`, `Histogram`, `Downsample`, `SortKey`,
`Sort`, `Transform`, `Query`, `StoredRecord`, `StoredRow`, `ObjectVersionToken`, `ObjectValue`, `ObjectEntry`,
`DataBatch`, `ReadLimits`, `StorageCapabilities`, `ColumnSource`, `ColumnPredicate`, `TimeBounds`, `ColumnSelection`, `ArrowPage`,
`IndexDefinition`, `IndexEntry`, `IndexSelection`, `IndexPage`, `TimeBucketPlan`, `SelectColumns`, `ProjectionDefinition`, `ProjectionSelection`,
`ProjectionPage`, `ProjectionStatus`, `SavedSnapshot`, `DefinitionRevision`, `ProjectionState`, `StorageFailure`, `FilePhase`, `CaptureConsistency`,
`ObjectVersions`, `Compare`, `DownsampleAlgorithm`, `ColumnKind`, `StorageFailureKind`.

`harmos-domain/src/model/storage` already owns histories/cursors, mutations, files,
directory values, keys, identities, objects and batches. `harmos-storage/src/query`
owns query plans, column/index/projection structures and bounds; several output buffers
are currently re-exported from the protocol crate and need native definitions. Move
the removed `storage_helpers.rs` directory-event constructor, structured storage failure
conversion, descriptor helper where still needed and output-buffer accounting to those
owners. `FollowingRow` and the removed Samples append pair need a native publication
representation for internal batch metadata; do not regenerate them as library wire types.

### Runtime Worker

- Implement samples child calls (behaviour rules 30–32): Route observer child calls through the existing admitted plugin context and registry; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

The consumer workers also migrate the affected README claims and owning guides
(`docs/product-guide.md`, `docs/plugin-manager.md`, `docs/storage.md` and language
guides) with their implementation. Their current executable examples describe the
preceding consumer tree, not proof of the new definition-only candidate.

- Migrate runtime admission and gRPC registration from client `harmos.v1[.*]` to `harmos.client.v1[.*]`, and context `harmos.context.v1` to `harmos.plugin.host.v1`. `harmos.plugin.v1.Instance` is unchanged. Remove Storage and Samples child bindings entirely; the native storage declarations/results above need their own owner.
- Mechanical Rust recipe: `harmos_protocol::v1::<client wrapper/resource>` → `harmos_protocol::client::<same name>`; `v1::access` → `client::access`; `v1::status` → `client::status`; `context::<retained wrapper>` → `plugin::host::<same name>`. Shared vocabulary remains `v1`. `storage::{FileMetadata, FileReference, schema graph types, transfer defaults}` → `v1::{same name}`; adapt `StorageCursor/StoreHistory` conversions to `Position/History`.
- Replace CatalogEntries dispatch with typed Operations/List/Get, Schemas/List and Streams/List. Map the existing schema and identity models field-for-field as specified above, preserving grant filters and complete bounded inventories. Delete the objects branch from library wire discovery.
- Implement Plugins.Replace staging/fencing and settings persistence, revision guards, replay identities, defaults and scoped plugin-host Settings.List on the existing entry owner. Publish trusted caller entry name plus reported description to product context handlers; remove HostValues resolver registration. A client field never chooses that caller.
- Keep product nested resources composed beneath Plugins; add the application-owned nested-resource fixture in all four SDKs. Atomically delete library settings with an entry, keep `PluginChange.deleted`, and leave eventual product cleanup to its in-process watch/startup reconciliation.
- The guest system fixture `tests/system/guest_contract/support.rs::Publisher` currently stages rows via Samples.Append, and `tests/operations/wire/generated_publication.rs` checks batched following-row metadata. These are concrete non-tick uses: migrate to a product fixture operation/native publication types, preserving scope and metadata. `Instance.Samples` still supports both tick answers and device-originated samples while demand is open. The Schematic example has no direct Samples.Append call found in this tree.

### Operations Worker

- Own the native request/result pairs listed under Native Type Ownership, including nested conditional outcomes; migrate `storage/client.rs`, `storage/context.rs`, transfers, lists, watches, pages, snapshots, schemas and registrations to one Private in-process declaration per capability. Keep real native behavior tests; delete redundant face tests only when their replacement covers the behavior.
- Rename generated wrappers with the Runtime Worker recipe; replace host CatalogEntries associated types and DiscoveryAccess responses with typed client/plugin-host Operations wrappers. Registry schemas/streams become typed client handlers. Remove HostValues and Samples wire declarations and expose the native one-call storage wrapper helper.
- No new generated module is the destination for removed Storage/Samples requests. Do not preserve them as compatibility aliases or another public package.

### Storage Worker

- Keep storage implementation/persistence in-process; reuse `harmos_domain::model::storage` types listed above. Replace `harmos_protocol::storage::<native value>` with the existing native owner or a native type defined in `harmos-storage` where it currently owns the behavior. Remove protocol-only query conversions that no consumer requires.
- Retained shared types move `harmos_protocol::storage::{FileMetadata,FileReference,SchemaKey,SchemaField,SchemaDefinition,SchemaRepresentation,SchemaGraph,SchemaInput,PayloadEncoding,SchemaCardinality,SchemaKind,SchemaForm,SchemaOmission,StorageTransferSettings,MessageSettings,TransferFraming}` → `harmos_protocol::v1::{same name}` at actual serialization boundaries. Use native values internally. Replace file-reference position conversion with `v1::Position`/`History`.
- Preserve cursor/revision encodings, conditional outcomes, mutations and old persisted data. Move protocol storage helpers and output-buffer accounting to their native owners; the protocol crate no longer exports `storage` or `storage_types`.

### Rust SDK Worker

- Implement samples child calls (behaviour rules 30–32): Enable observer contexts to call host operations, clock, settings and declared Watch through the Samples carrier; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

- Regenerated modules are `harmos_protocol::{v1,client,client::access,client::plugins,client::status,plugin,plugin::host}`. `context` and `storage` protocol modules are deleted. Apply the Runtime Worker mechanical recipe to hand-written SDK authoring, clock, transfer, server and guest code; retain the public SDK Context object.
- Generated client methods now live at `client.operations.{list,get}`, `client.schemas.list`, `client.streams.list`, `client.plugins.replace`, `client.plugins.settings.{list,set}`. The `catalog_entries` and `storage` roots are removed. `context/contracts.rs` and its per-message descriptor projections are already regenerated for `plugin::host`; migrate hand-written imports and declared requirements/fingerprints.
- Decode the existing Catalog settings section as typed declarations; read values through plugin-host Settings.List. Remove HostValues and Samples/Storage wire conveniences; product operations own any required wrappers. No SDK-owned replacement native storage request types are needed.

### Python SDK Worker

- Implement samples child calls (behaviour rules 30–32): Enable observer contexts to call host operations, clock, settings and declared Watch through the Samples carrier; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

- Old `harmos.v1.<service>_pb2[_grpc]` → `harmos._wire.client.v1.<service>_pb2[_grpc]`; `harmos.v1.access/status.*` → `harmos._wire.client.v1.access/status.*`. Old `harmos.context.v1.<retained service>_pb2` → `harmos.plugin.host.v1.<service>_pb2`. Instance remains `harmos.plugin.v1.instance_pb2[_grpc]`.
- Replace the former `harmos.v1.types_pb2` aggregate import with concept imports: `.position_pb2`, `.page_pb2`, `.receipt_pb2`, `.failure_pb2`, `.policy_pb2`, `.operation_pb2`, `.description_pb2`, `.invocation_pb2`, `.stream_pb2`, `.setting_pb2`. File/schema/default vocabulary moves from `storage_types_pb2`/`storage_settings_pb2` to `.file_pb2`/`.schema_pb2`/`.transfer_pb2`.
- Migrate authoring, invocation, transfer and package imports without reintroducing generated modules under `harmos.client`. Generated client root and fixture resources already use `_wire`. Remove storage/HostValues/Samples conveniences and switch registry/settings calls as for Rust. No native storage request types belong in the SDK.

### C# SDK Worker

- Implement samples child calls (behaviour rules 30–32): Enable observer contexts to call host operations, clock, settings and declared Watch through the Samples carrier; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

- `Harmos.V1.<client wrapper/resource>` → `Harmos.Client.V1.<same name>`; `Harmos.V1.AccessResources` → `Harmos.Client.V1.AccessResources`; `Harmos.V1.StatusResources` → `Harmos.Client.V1.StatusResources`; new settings wrappers live in `Harmos.Client.V1.PluginsResources`.
- `Harmos.Context.V1` → `Harmos.Plugin.Host.V1`; Instance remains `Harmos.Plugin.V1`. `Harmos.Storage.V1.<retained shared type>` → `Harmos.V1.<same name>`. The old file names Types/StorageTypes/StorageSettings are replaced by the concept files under `harmos/v1`.
- Regenerated `Generated/Client.g.cs` and composition fixture include typed registries and Plugins.Settings. Update any explicit protobuf input inventory/build imports, authoring/settings section builders, carrier requirements and transfer defaults; remove wire storage/HostValues/Samples helpers. No SDK-native storage request types are needed.

### C++ SDK Worker

- Implement samples child calls (behaviour rules 30–32): Enable observer contexts to call host operations, clock, settings and declared Watch through the Samples carrier; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

- `::harmos::v1::<client wrapper/resource>` → `::harmos::client::v1::<same name>`; nested `::harmos::v1::{access,status}` → `::harmos::client::v1::{access,status}`. `::harmos::context::v1` → `::harmos::plugin::host::v1`; Instance remains `::harmos::plugin::v1`.
- `::harmos::storage::v1::<retained shared type>` → `::harmos::v1::<same name>`. Include `harmos/client/v1/<service>.pb.h`, `harmos/plugin/host/v1/<service>.pb.h`, and the specific shared concept headers instead of `harmos/v1/types.pb.h` or storage aggregates.
- Update the hand-written input list in `sdk/cpp/cmake/Protocol.cmake` to the final proto inventory before its protocol build. Generated `harmos/generated/client.hpp` already uses the new packages. Update authoring, native server/context, transfer defaults and required fingerprints; remove obsolete wire convenience calls. No SDK-native storage request types are needed.

### Schematic Example Worker

- Apply the Rust module recipe and regenerate the product protocol after removing `harmos/context/v1/storage.proto` from `examples/schematic/proto/schematic/v1/operations.proto`. Its GetFileHash payload currently embeds `harmos.context.v1.StorageGetFileHashRequest`; replace it with a product-owned typed request and a native storage call.
- Declare only the product storage wrappers actually required by the example, using shared File/Upload/FileReference and the recommended bare verbs; do not recreate library Storage twins. Migrate analysis/hash/transfer/preview consumers and preserve existing native storage stories.
- Restore product-owned Buf validation after the proto imports compile, independently of the library module. No library wire aliases may be added to make the example compile.

### Conformance Tool Worker

- Implement samples child calls (behaviour rules 30–32): Implement deterministic cases for rules 30–32 in the reference host and fixtures; correlate out-of-order replies with IDs unique across ticks on one stream, preserve failures, cancel open children on tick answer/stream end, and refuse child traffic outside an unanswered tick. Preserve existing sample fields and tick ordering.

- Apply `v1::<client wrapper>` → `client::<same name>` and `context::<retained wrapper>` → `plugin::host::<same name>` to the reference host and fixtures; shared Catalog/frames stay `v1`. Adapt any aggregate proto imports to the concept files.
- Recompute descriptors/fingerprints for renamed service wrappers, preserve Instance wire identity, and migrate HostValues/registry tests to typed Settings/Operations. Storage and pushed publication belong to product fixture contracts or native stories, rather than library wire declarations. Conformance owns no native storage request replacement.

### Next Pin for Products That Vendor the Protocol

Replace the vendored `proto/harmos` tree as a whole, regenerate all language bindings
and update explicit file lists/imports, boundary admission and operation requirements
using the mechanical recipes above. Do not retain deleted aggregate proto files or
compatibility copies of Storage requests. Products wrapping storage must declare their
own typed client/plugin-host operations and policy and call the library’s Private
in-process storage operation through the native helper. File uploads/ranges, queries,
records and objects all need product wrappers; schema and stream registry listing can
use the library’s typed client resources. A managed Plugins.Create/Replace bundle uses
a shared FileReference resolved in-process; the product upload publishes that reference.
Compose Plugins, settings, access, status, invocations, operations and time directly;
do not wrap the complete library services. Keep product secrets in product resources
and publish declared needs in product sections. Native storage encodings and persisted
data must remain compatible even though the library wire storage surface is removed.
