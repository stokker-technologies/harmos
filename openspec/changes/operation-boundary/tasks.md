# Operation Boundary Tasks

Groups 2–7 are additive and land before the breaking groups 9–10. Each group commits with
Conventional Commits on the integration branch and proves only its changed behavior through
focused `mise run test`, `mise run build` and `mise run generate` invocations; full lanes run
only after group 11 closes. Products adopt the revision that landed a group in their own
change; each completed group records the revision a product can pin below.

Owner amendments: products use a typed `runtime.for_call(&ctx)` Rust facade over
registered native operations. All four plugin SDKs, protocol generation, Wasm guests
and conformance remain. The task's owner decisions supersede older design examples.

## 1. Inventory and Contract

- [x] 1.1 Lock the source inventory against design.md: every public export of `harmos-runtime`, `harmos`, `harmos-operations`, `harmos-storage`, `harmos-query` and `harmos-domain`, every manager `Deref` and getter, every engine type in an operation signature, every `callers` option under `proto/harmos/host/v1`, every composition reader and every statistics kernel; record any surprise in design.md before editing. Accept when the inventory lists owners and consumers (runtime, operations, SDKs, conformance, Schematic, stories) for each item.
- [x] 1.2 Update `docs/architecture.md`: quote and replace Product Composition and Narrowing, record the facade, private host operations, plugin protocol facilities, storage coverage and statistics home as locked direction with proposed status, and update Storage and Query Ownership and Consumer Work Lists. Accept when the document names no manager registration or narrowing contract as current; prove with `mise run check -- --repository`.

## 2. Compound Atomic Mutation

- [ ] 2.1 Add the compound mutation request/result to the native storage contracts in `harmos-domain/src/model/storage` and `harmos-operations/src/storage/native` (create, conditional set, delete, record append items; one mutation identity). Accept when the declaration registers in the storage catalog; prove with `mise run build -- --package harmos-operations --check`.
- [ ] 2.2 Add the backend capability and one-commit primitive to `harmos-storage/src/backend.rs`, implement it in the in-memory backend and the reference SQLite backend, and refuse with a capability error elsewhere. Prove with new stories `compound_mutation_commits_atomically`, `compound_mutation_conflict_leaves_no_effect` and `compound_mutation_replays_receipt` under `crates/harmos-storage/tests/integration` run by `mise run test -- --paths <story file>`.
- [ ] 2.3 Wire the native handler through admission, receipts and cut publication, and add a runtime story calling it through the operation caller. Prove with `mise run test -- --package harmos-runtime --target integration --exact compound_mutation_through_operations`; document the operation in `docs/storage.md`.

## 3. Object and Scoped Deletion

- [ ] 3.1 Add object deletion (optional expected revision, deletion revision) as a native storage operation and backend primitive in both bundled backends. Prove with `object_delete_hides_at_new_cuts_only` in `crates/harmos-storage/tests/integration`.
- [ ] 3.2 Add scoped deletion of objects, records, samples, projection rows and files under one opaque scope with replayable receipts and reclamation after retained cut release. Prove with `scoped_delete_reclaims_after_cut_release` and `scoped_delete_replays_receipt`.
- [ ] 3.3 Allow compound mutation items to include object deletion and prove `compound_mutation_deletes_and_appends` through the operation caller; document deletion and reclamation semantics in `docs/storage.md`.

## 4. Storage Before Full Startup

- [ ] 4.1 Let the builder produce a storage-only runtime that owns the storage manager and admits storage operations without listeners, plugins or services, then upgrades to the full runtime or closes. Prove with `storage_only_runtime_admits_operations` in `crates/harmos-runtime/tests/storage`.
- [ ] 4.2 Add maintenance operations for a consistent backend copy to a target and backend disposal through new backend hooks, with capability errors when absent; implement the hooks in the reference backend. Prove with `maintenance_copy_is_consistent_at_one_cut` and `maintenance_without_hook_refuses`; document both in `docs/storage.md` and `docs/product-guide.md`.

## 5. Expression Transforms

- [ ] 5.1 Add `Scale`, `Offset` and `Contains` to the domain transform contract and their execution in `harmos-query/src/transform`, preserving null and missing-value semantics. Prove with focused transform tests via `mise run test -- --package harmos-query --target lib` filtered to the new tests and a `list_rows` story through the operation caller; update `docs/storage.md`.

## 6. Statistics Consolidation

- [ ] 6.1 Move moment, quantile, order-statistic and histogram kernels under `harmos-query::statistics` (crate-internal), make transform modules and the benchmark percentile helper delegate to them, and delete duplicates. Accept when no kernel exists outside that module; prove with the existing aggregate and histogram tests via `mise run test -- --package harmos-query --target lib`.
- [ ] 6.2 Make several percentiles of one field share one spool and rank selection in aggregate execution. Prove with a deterministic test that counts spool passes and compares results with single-percentile requests.
- [ ] 6.3 Add mergeable summaries with first/last carrying time and position, and expose ordered first/last as aggregate measures. Prove merge equivalence over adjacent ranges with deterministic data.
- [ ] 6.4 Add the box-summary transform (quartiles, Tukey whiskers, bounded deterministic outliers with total count). Prove constant, extreme-range and over-bound outlier cases; document the transform in `docs/storage.md`.

## 7. Bounded Bucket Summaries

- [ ] 7.1 Add the bucket-summary transform with a bucket bound, width doubling and pairwise merge, preserving first/last/min/max points with positions, count, mean and moments, plus continuation from a supplied prior summary and source cut. Prove that continued summaries equal whole-range summaries, cancellation and the bucket bound with deterministic data through `list_rows`; document it in `docs/storage.md`.

## 8. Additive Revision Handoff

- [ ] 8.1 With groups 2–7 closed, run focused proof for every touched story, `mise run format` and the Rust and Python language checks on owned work, regenerate SDK bindings if contracts changed (`mise run generate -- --sdk rust`, then python, csharp, cpp), and commit. Accept when the commit is the usable additive revision products adopt; record in design.md Migration Plan that the additive groups are closed.

## 9. Idiomatic Facade

- [ ] 9.1 Make the builder accept backends, catalog and storage settings and own manager construction, start and close; delete public `StorageManager` constructors and `setup.settings().storage(..)`. Prove with the runtime storage stories after migrating them to the builder.
- [ ] 9.2 Add in-process host operations for plugin activation, clock binding and sample start/stop that products previously drove through managers, preserving cleanup and cancellation. Prove with `plugin_activation_through_operations` and `sampling_through_operations` stories.
- [ ] 9.3 Replace engine types in operation results (`QueryOutput` in native requests) with operation-owned batch DTOs. Prove with the storage operation stories and `mise run build -- --package harmos-operations --check`.
- [ ] 9.4 Remove both manager `Deref` implementations, manager getters on `Runtime`, `Handle` and service contexts, and whole-crate re-exports; export only builder, operation calling and the backend authoring module. Do not change operation visibility. Prove with a compile-fail test that manager access is absent and `mise run build -- --package harmos-runtime --check`.
- [ ] 9.5 Migrate Harmos's own consumers: Schematic example, runtime and storage stories, SDK fixtures and benchmarks, to the facade and operations; add the repository check rejecting direct internal-crate dependencies outside implementation crates. Prove with `mise run build -- --manifest-path examples/schematic/Cargo.toml --package schematic-app` and `mise run check -- --repository`.
- [ ] 9.6 Update `docs/architecture.md`, `docs/product-guide.md` and `docs/storage.md` to the builder and facade contract and remove direct manager examples; prove with `mise run check -- --repository`.

## 10. Private Host Operations

- [ ] 10.1 Add plugin protocol host facilities to `proto/harmos/plugin/v1` and the carrier for every call the plugin SDK contexts make today (time get/sleep, own settings list, own invocation Get/Cancel/Watch, admitted discovery List/Get) and move all four plugin SDK contexts to them without removing any capability. Prove with the plugin carrier stories, existing child-call stories and per-language SDK fixture tests.
- [ ] 10.2 Remove the `callers` option from every RPC under `proto/harmos/host/v1`, register Harmos host operations in-process only and stop serving them on the listener; keep `callers.proto` for product wrappers. Prove with `harmos_host_operations_are_unreachable_on_the_wire` covering direct gRPC, encoded invocation and child requests.
- [ ] 10.3 Delete `Builder::composition`, composition parsing, the generator `--composition` option and their narrowing tests (in `crates/harmos-runtime/tests/plugin_grpc/application_faces.rs` and `tools/python/tests/integration/test_protocol_callers.py`), moving any coverage that is not about narrowing to the wrapper stories; generators derive projections from product descriptors and declared audiences. Prove with generator tests against the Schematic descriptors.
- [ ] 10.4 Make in-process discovery apply the dispatch predicate for scoped plugin callers, closing the `scope: None` gap. Prove with a story where a wrapper forwards plugin discovery and omits entries dispatch refuses.
- [ ] 10.5 Delete generated `Client` resources over Harmos host operations in the Rust, Python, C# and C++ SDKs, keeping connection, credentials, call options, failures, transfer helpers and the Harmos host request/response messages as exported payload values for product wrappers; regenerate with `mise run generate -- --sdk rust`, then python, csharp, cpp. Prove with each SDK's focused fixture build.
- [ ] 10.6 Move conformance to a fixture product that declares wrappers, replace wrong-caller rules with unreachable-host-operation and wrapper-audience rules, keep plugin protocol rules; make Schematic declare wrappers for what its client uses. Prove with `mise run build -- --package harmos-conformance` and the Schematic client example build.
- [ ] 10.7 Update `docs/architecture.md` (Caller Options and SDK Projection, Application Operations in Discovery, Next Pin), SDK guides and the README for private host operations, product wrappers and plugin protocol facilities; prove with `mise run check -- --repository`.

## 11. Integration

- [ ] 11.1 Run `openspec validate operation-boundary --strict`, the format task and the Rust, Python, C# and C++ language checks on the combined tree, and confirm every source item above is closed; report any pending native platform or hosted proof explicitly.

## Workflow Follow-Up

- Products adopt group 8's revision for storage and statistics, then the revision that landed groups 9–10 in the task that removes their use of the removed surface.
- Full native Linux and Windows validation and hosted CI follow source closure under the delivery authorization.
- Archive this change after delivery and fill each new capability's Purpose in the main specs.
