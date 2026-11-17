# Operation Boundary Tasks

Groups 2–7 are additive and land before the breaking groups 9–10. Each group commits with
Conventional Commits on the integration branch and proves only its changed behavior through
focused `mise run test`, `mise run build` and `mise run generate` invocations; full lanes run
only after group 11 closes. Products adopt the revision that landed a group in their own
change; each completed group records the revision a product can pin below (owner override).

## 1. Inventory and Contract

- [x] 1.1 Lock the source inventory against design.md: every public export of `harmos-runtime`, `harmos`, `harmos-operations`, `harmos-storage`, `harmos-query` and `harmos-domain`, every manager `Deref` and getter, every engine type in an operation signature, every `callers` option under `proto/harmos/host/v1`, every composition reader and every statistics kernel; record any surprise in design.md before editing. Accept when the inventory lists owners and consumers (runtime, operations, SDKs, conformance, Schematic, stories) for each item.
- [x] 1.2 Update `docs/architecture.md`: quote and replace Product Composition and Narrowing, record the facade, private host operations, plugin protocol facilities, storage coverage and statistics home as locked direction with proposed status, and update Storage and Query Ownership and Consumer Work Lists. Accept when the document names no manager registration or narrowing contract as current; prove with `mise run check -- --repository`.
- [x] 1.3 Amend D7, facade and SDK/policy ownership for the 2026-11-16 `plugin-boundary` decision; verify the three changes with strict OpenSpec validation. Documentation only; implementation remains open.

Group 1 Pin: `cffd2ec57486c7ad5e7de405495a340681612b99`.

## 2. Compound Atomic Mutation

- [x] 2.1 Add the compound mutation request/result to the native storage contracts in `harmos-domain/src/model/storage` and `harmos-operations/src/storage/native` (create, conditional set, delete, record append items; one mutation identity). Accept when the declaration registers in the storage catalog; prove with `mise run build -- --package harmos-operations --check`.
- [x] 2.2 Add the backend capability and one-commit primitive to `harmos-storage/src/backend.rs`, implement it in the in-memory reference backend, and refuse with a capability error elsewhere. Harmos owns this contract and memory reference only; durable product backends live in the product. Supply an importable backend conformance test kit for products to prove atomic commits, conflicts and receipt replay. Prove with new stories `compound_mutation_commits_atomically`, `compound_mutation_conflict_leaves_no_effect` and `compound_mutation_replays_receipt` under `crates/harmos-storage/tests/integration` run by `mise run test -- --paths <story file>`.
- [x] 2.3 Wire the native handler through admission, receipts and cut publication, and add a runtime story calling it through operation dispatch. Prove with `mise run test -- --package harmos-runtime --target integration --exact compound_mutation_through_operations`; document the operation in `docs/storage.md`.

Compound Native/Memory Checkpoint Pin: `6197796daee6feed793f33d3330e4c8a8356444f` (2.1, 2.3, 3.3).

Group 2 Pin: `502121ff` (product backend conformance kit; focused compound stories, build and Rust lint passed).

## 3. Object and Scoped Deletion

- [x] 3.1 Add object deletion (optional expected revision, deletion revision) as a native storage operation and backend primitive in the contract and memory reference backend. Prove with `object_delete_hides_at_new_cuts_only` in `crates/harmos-storage/tests/integration`.
- [x] 3.2 Add scoped deletion of objects, records, samples, projection rows and files under one opaque scope with replayable receipts and reclamation after retained cut release. Prove with `scoped_delete_reclaims_after_cut_release` and `scoped_delete_replays_receipt`.
- [x] 3.3 Allow compound mutation items to include object deletion and prove `compound_mutation_deletes_and_appends` through operation dispatch; document deletion and reclamation semantics in `docs/storage.md`.

- [x] 3.4 G1: Persist scope-qualified retained-cut ownership in the backend, reopen native handles after restart, release scope ownership atomically on deletion, preserve independent readers, and reclaim unreferenced rows/files without pinning unrelated deleted scopes. Extend the backend conformance kit with a two-stage restart proof. Prove with the retained-scope restart/reclamation and native-registry restart tests, focused storage/operations build and lint.
- [x] 3.5 G3: Include compound entries in followers, observe applied conditional revisions, suppress conflicts, and issue watch/projection cursors only at complete mutation frontiers. Define deletion/expired-cut resync and prove bounded compound pages, conditional/conflict/delete sequences, historical replay and the backend conformance kit with deterministic tests.
- [x] 3.6 Integrate G1/G3 with the landed columnar append and operation-owned DTO changes: project columnar samples in accepted followers, keep durable sample cursors at complete append frontiers, preserve retained scope ownership while invalidating session tokens, and extend deterministic columnar watch/restart and backend conformance proof. Prove with focused storage/native snapshot tests, storage/operations build, format and lint.
- [x] 3.7 Disable automatic retries for native CreateSnapshot and RetainSnapshot while retention has no stable request identity; add a regression test checking every registered built-in operation's policy against its replay declaration.

G1/G3 Proof (Windows): `mise run test -- --paths crates/harmos-storage/tests/integration/conditional_objects/deletion.rs crates/harmos-storage/tests/integration/manager/durable_watch.rs` passed all 136 tests in the affected storage target, including compound/scoped backend conformance and the two-stage restart kit. `mise run test -- --package harmos-operations --target lib --story storage::native::snapshot::tests` passed both native restart/copy/disposal and compound/expired-cut resync tests. `mise run build -- --package harmos-operations --lib`, `mise run lint` (Rust and Python), `mise run format -- --language rust`, `mise run check:rust` and strict OpenSpec validation passed. Full lanes, native Linux and hosted proof remain integration work; no benchmark measurements ran.

Group 3 Pin: `1c4f3aec8cf12dd59fe636545e76f72d6d5860bf` (native object/scope deletion, explicit object scope, retained-cut reclamation and scoped backend conformance; build, Rust lint, format and focused proof passed).

G1/G3 Product Candidate Pin: `bf297bf672cbfe841abfc087ae9fe3358957a13e` (durable ownership and complete watch frontiers; the local source commit passed the Windows proof above. Push/integration and native Linux/hosted proof remain with the integration owner).

## 4. Storage Before Full Startup

- [x] 4.1 Let the builder produce a storage-only runtime that owns the storage manager and admits storage operations without listeners, plugins or services, then drains and closes before the full runtime reopens the same registered backends, or closes for offline use. Prove with `storage_only_runtime_admits_operations` in `crates/harmos-runtime/tests/storage`.
- [x] 4.2 Add maintenance operations for a consistent backend copy to a target and backend disposal through new backend hooks, with capability errors when absent; implement the hooks in the reference backend. Prove with `maintenance_copy_is_consistent_at_one_cut` and `maintenance_without_hook_refuses`; document both in `docs/storage.md` and `docs/product-guide.md`.

Group 4 Pin: `b3b37c6e115dddfdbd8dff72dbccfda969be08d4` (drain/reopen assembly, consistent primary-backend copy with lifecycle/I/O guards and offline disposal; build, Rust lint, format, repository/dependency checks and 17 focused stories passed).

## 5. Expression Transforms

- [x] 5.1 Add `Scale`, `Offset` and `Contains` to the domain transform contract and their execution in `harmos-query/src/transform`, preserving null and missing-value semantics. Prove with focused transform tests via `mise run test -- --package harmos-query --target lib` filtered to the new tests and a `list_rows` story through operation dispatch; update `docs/storage.md`.

Group 5 Pin: `b3b37c6e115dddfdbd8dff72dbccfda969be08d4` (streaming Float64 scale/offset and typed string/list containment; build, Rust lint, format, repository/dependency checks, 5 query tests, 4 native dispatch stories and 12 backend conformance stories passed).

## 6. Statistics Consolidation

- [x] 6.1 Move moment, quantile, order-statistic and histogram kernels under `harmos-query::statistics` (crate-internal), make transform modules and the benchmark percentile helper delegate to them, and delete duplicates. Accept when no kernel exists outside that module; prove with the existing aggregate and histogram tests via `mise run test -- --package harmos-query --target lib`.
- [x] 6.2 Make several percentiles of one field share one spool and rank selection in aggregate execution. Prove with a deterministic test that counts spool passes and compares results with single-percentile requests.
- [x] 6.3 Add mergeable summaries with first/last carrying time and position, and expose ordered first/last as aggregate measures. Prove merge equivalence over adjacent ranges with deterministic data.
- [x] 6.4 Add the box-summary transform (quartiles, Tukey whiskers, bounded deterministic outliers with total count). Prove constant, extreme-range and over-bound outlier cases; document the transform in `docs/storage.md`.
- [x] 6.5 Bound the query benchmark's percentile comparison by floating-point precision and replace its test-support path overrides with benchmark-owned storage and projection fixtures.

## 7. Bounded Bucket Summaries

- [x] 7.1 Add the bucket-summary transform with a bucket bound, width doubling and pairwise merge, preserving first/last/min/max points with positions, count, mean and moments, plus continuation from a supplied prior summary and source cut. Prove that continued summaries equal whole-range summaries, cancellation and the bucket bound with deterministic data through `list_rows`; document it in `docs/storage.md`.

Groups 6–7 Pin: `9745a250b5ae1882864c1e1b7e4b530d0211d780` (local statistics checkpoint; not pushed).
The checkpoint includes shared exact percentile selection, ordered mergeable summaries,
box summaries and bounded bucket continuations. Windows proof passed: targeted build,
Rust lint, format and dependency check; 31 query unit tests, 9 query integration stories,
33 operations unit tests, the native ListRows statistics story, 4 aggregate codec stories
and the grouped time-bucket replacement story. The existing query bench passed one
Criterion smoke run. Group 8, combined validation and Linux/hosted proof remain open.

## 8. Additive Revision Handoff

- [ ] 8.1 With groups 2–7 closed, run focused proof for every touched story, `mise run format` and the Rust and Python language checks on owned work, regenerate SDK bindings if contracts changed (`mise run generate -- --sdk rust`, then python, csharp, cpp), and commit. Accept when the commit is the usable additive revision products adopt; record in design.md Migration Plan that the additive groups are closed.

## G2. Columnar Sample Appends

- [x] G2 Implement the product storage mapping gap end to end: retain each AppendSamples Arrow batch as one backend unit, validate scope and ordering without row payload conversion, commit its mutation receipt atomically, implement memory retention and importable conformance for accepted/completed receipts, replay/reopen and bounded admission, and add the 128-stream, 1 kHz, 100 ms-window samples/s benchmark. Prove with the storage build, Rust lint, focused storage and conformance stories, format, and one `mise run bench -- --storage --filter columnar_append` run. Record the product-pinnable revision after proof.

G2 Source Pin: `93fcfb7a4117cd4b57dd9238f8e0b064ac534d5b` (local worker checkpoint; publish with the combined additive delivery before product adoption).

G2 Proof: Windows `mise run build -- --package harmos-storage --lib`, `mise run test -- --package harmos-storage` (167 passed, including six columnar/conformance stories), `mise run lint`, `mise run format`, `mise run check:rust`, `mise run ci:docs` and `mise run bench -- --storage --filter columnar_append` passed. Native Linux and persistent-product backend proof remain part of integration; benchmark measurements are reported at handoff.

## 9. Idiomatic Facade

- [ ] 9.1 Make the builder accept backends, catalog and storage settings and own manager construction, start and close; delete public `StorageManager` constructors and `setup.settings().storage(..)`. Prove with the runtime storage stories after migrating them to the builder.
- [ ] 9.2 Add in-process host operations for plugin activation, clock binding and sample start/stop that products previously drove through managers, preserving cleanup and cancellation. Prove with `plugin_activation_through_operations` and `sampling_through_operations` stories.
- [x] 9.3 Replace engine types in operation results (`QueryOutput` in native requests) with operation-owned batch DTOs. Prove with the storage operation stories and `mise run build -- --package harmos-operations --check`.
- [ ] 9.4 Add the typed in-process facade of design.md D6: `Runtime::for_call(&ctx)` returning capability groups (storage, plugins, streams, invocations, time and the remaining host operation families), each method dispatching one native operation with its registered policy and one-call overrides from the call context, exposing no manager, engine or scheduler. Prove with stories `facade_calls_use_registered_policy` and `facade_override_applies_to_one_call` in `crates/harmos-runtime/tests/integration` via `mise run test -- --paths <story file>`.
  - [x] Native storage/publication foundation: root and scoped `for_call`, all 41 storage methods and typed watch endpoints, registered-policy admission, consumed call-context overrides, parent cancellation and runtime fencing. Five paused-clock facade integration stories pass; plugin activation and the other host families remain open.
- [ ] 9.5 Give every built-in that dispatches through `call_protobuf` today (`crates/harmos-runtime/src/operations/{surface,handler}.rs`, `src/services/mod.rs` and their callers) a native declaration, so facade calls pass typed values without encoding; keep `call_protobuf` only for genuinely encoded wire paths. Prove with a story `facade_calls_do_not_encode` that counts encode calls through a test codec and the existing protected built-in and scoped sleep stories.
- [ ] 9.6 Remove both manager `Deref` implementations, manager getters on `Runtime`, `Handle` and service contexts, and whole-crate re-exports; export only builder, the typed facade (task 9.4) and the backend authoring module. Do not change operation visibility. Prove with a compile-fail test that manager access is absent and `mise run build -- --package harmos-runtime --check`.
  - [x] Remove whole-crate `domain`/`builtins` reexports, unused renamed private exports and the catalog glob from the entry library; migrate affected storage consumers to explicit facade DTOs and builder catalogs. Manager privacy and both `Deref` removals remain open.
  - [x] Expose the backend-authoring contracts and value/error closure through `harmos_runtime::backend`, remove the storage-only session's operation-manager getter and wrapper, migrate its drain/reopen/cancellation story to typed calls, retain catalog-separation coverage internally, and prove the absent exports with compile-fail doctests. Builder ownership, the other manager getters and both manager `Deref` implementations remain open.
- [ ] 9.7 Migrate Harmos's own consumers: Schematic example, runtime and storage stories, SDK fixtures and benchmarks, to the facade and operations; add the repository check rejecting direct internal-crate dependencies outside implementation crates. Prove with `mise run build -- --manifest-path examples/schematic/Cargo.toml --package schematic-app` and `mise run check -- --repository`.
  - [x] Migrate the affected Schematic storage journeys and native expression row-listing story to the storage facade and explicit DTO exports; the Schematic app build and three existing journeys pass. The remaining consumer inventory and dependency fence stay open.
- [ ] 9.8 Update `docs/architecture.md`, `docs/product-guide.md` and `docs/storage.md` to the builder and facade contract and remove direct manager examples; prove with `mise run check -- --repository`.

Native Result DTO Checkpoint Pin: `552d7df6a3a386d93b8beaf6263d16f96391c006` (9.3).
Storage operations own row IPC frames, column/index/projection selectors, definitions,
pages and status values. Engine seek cursors stay behind continuation tokens. Windows
proof passed: build, Rust lint, format and focused storage and statistics-consumer stories.
The remaining facade tasks stay open.

Native Facade Foundation Pin: `7bb345b73bb9bfa5214e46432bd81a3f3cf6fbca` (partial 9.4, 9.6 and 9.7; local checkpoint, not pushed).

Windows targeted proof passed: runtime library build, Schematic app check, five paused-clock
facade integration stories, one context-removal unit test, the expression row-listing story
and both Schematic storage journeys. Format, repository/dependency checks and strict OpenSpec
validation and the complete required lint task passed. The remaining facade families, native built-ins,
builder ownership, manager privacy, dependency fence and host/SDK cutover stay open; this is
not source closure for groups 9–10. Native Linux and hosted proof remain pending.

Backend Authoring and Storage Session Pin: `402961139c395aed7af4f2135e63c32f3eb0c6d5` (partial 9.6; local checkpoint, not pushed).

Windows runtime and Schematic app builds, both focused storage-session stories,
both compile-fail doctests, Rust format/dependency checks, repository contracts
and `mise run lint` passed. Builder ownership, the remaining
manager privacy and getters, both manager `Deref` removals, the product dependency
fence and the remaining consumer migration stay open; this is not source closure
for the runtime ownership group. Native Linux and hosted proof remain pending.

## 10. Private Host Operations

- [ ] 10.1 Add plugin protocol host facilities to `proto/harmos/plugin/v1` and the carrier for every call the plugin SDK contexts make today (time get/sleep, own settings list, own invocation Get/Cancel/Watch, admitted discovery List/Get) and move all four plugin SDK contexts to them without removing any capability. Prove with the plugin carrier stories, existing child-call stories and per-language SDK fixture tests.
- [ ] 10.2 Remove the `callers` option from every RPC under `proto/harmos/host/v1`, register Harmos host operations in-process only and stop serving them on the listener; keep `callers.proto` for product wrappers. Prove with `harmos_host_operations_are_unreachable_on_the_wire` covering direct gRPC, encoded invocation and child requests.
- [x] 10.3 Delete `Builder::composition`, composition parsing, the generator `--composition` option and their narrowing tests (in `crates/harmos-runtime/tests/plugin_grpc/application_faces.rs` and `tools/python/tests/integration/test_protocol_callers.py`), moving any coverage that is not about narrowing to the wrapper stories; generators derive projections from product descriptors and declared audiences. Prove with generator tests against the Schematic descriptors.
  - [x] Remove runtime/setup parsing and the manager override map, descriptor-generator narrowing and composition inputs; migrate Schematic code generation and all four fixture consumers to descriptor audiences; delete narrowing-only stories, preserve scoped discovery proof, regenerate tracked outputs, and update README and affected guides. Owned source: runtime/setup/manager wiring, tools/python descriptor generators and focused tests, SDK fixture build inputs, Schematic protocol/app wiring, conformance caller fixture, and their maintained documentation. Excludes the parallel storage and compiler-cache work.
Composition Removal Pin: `5ecd6a96326c1a3adc0c47e78da4002bf4461a64` (10.3; local checkpoint, not pushed).

- [x] 10.4 Make in-process discovery apply the dispatch predicate for scoped plugin callers, closing the `scope: None` gap. Prove with a story where a wrapper forwards plugin discovery and omits entries dispatch refuses.
- [ ] 10.5 Delete Harmos host-client roots and host RPC clients; retain internal Rust/Python/C#/C++ carrier helpers, shared generation, supported guest ABI, generic conformance and reusable connection plumbing under D7 and `plugin-boundary`. Move management messages to products and hide other reused payload types behind product author APIs. Coordinate one final regeneration with `plugin-boundary` and `plugin-lifecycle` through the generate tasks; prove focused generated consumers in every language.
  - [ ] Retire the standalone/inherited Python client rendering branches in `tools/python/src/harmos_tools/build/resources/python.py` with the host-client roots instead of preserving two client composition mechanisms.
- [ ] 10.6 Move conformance to a fixture product that declares wrappers, replace wrong-caller rules with unreachable-host-operation and wrapper-audience rules, keep plugin protocol rules; make Schematic declare wrappers for what its client uses. Prove with `mise run build -- --package harmos-conformance` and the Schematic client example build.
- [ ] 10.7 Update `docs/architecture.md` (Caller Options and SDK Projection, Application Operations in Discovery, Next Pin), SDK guides and the README for private host operations, product wrappers and plugin protocol facilities; prove with `mise run check -- --repository`.
- [x] 10.8 Allocate C++ constructor and call parameter names against resource members and call locals, cover nested naming collisions in a generator regression test, and regenerate the tracked C++ output.

Plugin Discovery Checkpoint Pin: `aec532eb34965b8b177d13526267ddd3ac1988e0` (10.4;
includes the native result DTO checkpoint). Shared child admission enforces declared
requirements, fingerprints, audience and provider visibility through product List/Get
wrappers, preserving fingerprint failure classification. Windows proof passed: runtime,
Schematic and conformance builds, Rust lint, format, wrapper discovery, protected host/peer,
child-call and 13 scoped-sleep stories. Host exposure and SDK cutover tasks stay open.

The first `--paths` test selection expanded to the entire integration binary and failed
before fixture preparation. Focused affected stories passed after building the fixtures;
that broader run has not been repeated. Use `--story` or `--exact` for focused proof.
Linux, combined-source validation and hosted proof remain open under task 11.

## 11. Integration

- [ ] 11.1 Run `openspec validate operation-boundary --strict`, the format task and the Rust, Python, C# and C++ language checks on the combined tree, and confirm every source item above is closed; report any pending native platform or hosted proof explicitly.

## Workflow Follow-Up

- Products adopt group 8's revision for storage and statistics, then the revision that landed groups 9–10 in the task that removes their use of the removed surface.
- Full native Linux and Windows validation and hosted CI follow source closure under the delivery authorization.
- Archive this change after delivery and fill each new capability's Purpose in the main specs.
