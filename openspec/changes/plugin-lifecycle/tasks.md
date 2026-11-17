# Plugin Lifecycle Tasks

Each group commits with Conventional Commits on the integration branch, keeps the build green
and proves only its changed behavior through focused `mise run test`, `mise run build` and
`mise run generate` invocations; full lanes run only after group 7 closes. Products adopt the
revision that landed this change in their own change; record the product-pinnable worker revision below (owner override).

## 1. Inventory and Contract

- [ ] 1.1 Lock the source inventory against design.md Context: every public `launch`, `shutdown` and `serve` symbol and which of them denote start, stop or run; every `on_stop`, `on_initialize`, `on_shutdown`, `on_cleanup` and `cleanup` hook in the runtime, operations crate and four SDKs; every declaration builder chain and `declarations()` call in SDKs, examples, fixtures and conformance; every import alias and path override in the repository. Accept when each item names its owner and consumers and an owning task below; record surprises in design.md before editing.
- [ ] 1.2 Update `docs/architecture.md` with the paired-verb rule, host verbs, plugin phases (admission, initialize, finalize, carrier loss), registrar and module catalogs, product declaration sections and the no-alias rule as locked direction with proposed status; keep Guest Entries and Reported Declarations' single payload name. Prove with `mise run check -- --repository`.
- [x] 1.3 Amend SDK/registrar ownership and kind admission for `plugin-boundary`; review both boundary changes and pass strict OpenSpec validation. Documentation only; lifecycle implementation stays open.

## 2. Host Verbs and Hook Names

- [x] 2.1 Rename `Runtime::launch`/`launch_with` to `start`/`start_with`, `shutdown`/`shutdown_with_timeout` to `stop`/`stop_with_timeout`, add `Builder::new` construction (D1 constructor deviation recorded in design.md) and `run` (start, wait for a stop request, stop) per design.md D1, and migrate runtime stories, benchmarks and the Schematic example. Prove with `runtime::lifecycle::runtime_run_stops_like_stop` via `mise run test -- --package harmos-runtime --target integration --exact runtime::lifecycle::runtime_run_stops_like_stop` and `mise run build -- --manifest-path examples/schematic/Cargo.toml --package schematic-app`.
- [x] 2.2 Rename `cleanup` to `finalize` on `Lifecycle`, `Service`, `Carrier`, `ServiceInstance` and `NativeInstance` with unchanged semantics, keeping `prepare` and the `stop` hooks (design.md D5). Proved with focused runtime tests `invalid_setup_cleanup`, `partial_cleanup_retry`, `finalizing_projects_cleanup` and `restart_cancels_refill` via `mise run test -- --package harmos-runtime --target integration --exact <full test path>` (four passed).
- [x] 2.3 Rename SDK serving entry points to `run` (Rust, Python, C++ `run`; C# `RunAsync`) and the Python endpoint `close` to `stop`; update the four SDK guides and `docs/product-guide.md`. Proved with Rust/Python/C#/C++ fixture builds and focused tests, plus `mise run check:repository`; C# and C++ language checks passed.

## 3. Registration

- [ ] 3.1 Implement internal Rust carrier-helper accumulation for generic declarations and opaque product sections without plugin-type growth; expose the public registrar/registration API only in the fixture product SDK under `plugin-boundary`. Delete superseded builder chains and prove typed accumulation plus duplicate refusal through focused SDK/fixture tests; update helper guidance.
- [ ] 3.2 Dissolve `harmos::wasm::Registration` into the plugin value plus `harmos::wasm::Guest` carrying capacity, keeping the `harmos_register` export symbol and the stamped-metadata check. Prove with the guest registration stories in `crates/harmos-runtime/tests` and the Wasm fixture build.
- [ ] 3.3 Implement matching accumulation in Python/C#/C++ carrier helpers and product-owned fixture registrars (C++ `add`), replacing builder composition without author-facing Harmos types. Prove focused fixtures in every language and include generated changes in `plugin-boundary` task 6.1's coordinated regeneration; update helper guidance.
- [ ] 3.4 Give every contributing module in the Schematic example, SDK fixtures and conformance plugins a `catalog()` registered once by a root register function, and update the four SDK guides and `docs/product-guide.md` with the register phase, module catalogs and the inert-registration rule. Prove with the Schematic build, `mise run build -- --package harmos-conformance` and `mise run check -- --repository`.

### Lifecycle Worker Checkpoint

- [x] Host construction/start/stop/run source and migrated runtime/Schematic callers;
  `Builder::new(origin)` returns its own registration type and `run` takes a product-owned stop future.
- [x] Internal Rust, Python, C# and C++ declaration accumulation, replacing the former
  declaration builder methods; module catalogs in the four conformance fixtures and Schematic.
  Rust settings registrations merge into one settings declaration section before duplicate validation.
- [x] Serving helpers use run/RunAsync; Python endpoints pair start/stop. SDK consumers,
  generators and maintained guides migrate with the source. Generated output stays tracked.
- [ ] Complete the native/Wasm lifecycle and guest wrapper replacement after the manager
  integration boundary in design.md is assigned; no hook/protocol compatibility path was added.
- [ ] Finish the public clock/shared-test-support and host-client-generated alias inventory;
  these unresolved items remain under group 6 rather than becoming exceptions. Runtime feature-gate layout also remains with its owner.
- [ ] Combined source closure, all affected conformance rules, Linux and hosted proof.

## 4. Plugin Phases on the Wire and in the Host

- [ ] 4.1 Add `Instance.Initialize`, replace `Instance.Stop` with `Instance.Finalize` including reason, rename `GuestShutdown`/policy to finalize and move lifecycle policies into generic declarations. Lock these carrier declarations with both boundary changes for `plugin-boundary` task 6.1's single regeneration; prove the resulting protocol through a focused package build.
- [ ] 4.2 Make plugin activation call initialize after admission and before any invocation or sample window, and make stop and removal refuse new invocations, cancel and settle in-flight work, then call finalize exactly once per attempted initialize, for native and Wasm placements. Prove with new stories `plugin_finalizes_after_failed_initialize`, `plugin_invocation_waits_for_initialize`, `plugin_stop_cancels_then_finalizes` and `plugin_initialize_deadline_finalizes` in `crates/harmos-runtime/tests/plugin_grpc`, plus the guest equivalents, via `mise run test -- --paths <story file>`.
- [ ] 4.3 Update conformance in `tools/conformance` with initialize, finalize, failed-initialize and cancel-then-finalize rules for every SDK reference plugin. Prove with `mise run build -- --package harmos-conformance` and the conformance fixture run for each language.

## 5. Plugin Hooks in the SDKs

- [ ] 5.1 Replace Rust `Plugin::on_stop` and Wasm `on_initialize`/`on_shutdown`/`on_cleanup` with `initialize` and `finalize` hooks receiving a context and, for finalize, the reason; run finalize locally on carrier loss after an attempted initialize. Prove with SDK tests for failed initialize and carrier loss via `mise run test -- --package harmos --target integration`.
- [ ] 5.2 Make the same change in Python (`@on_stop` removed), C# (`OnStop` removed) and C++ (`on_stop` removed), with idiomatic casing and async forms. Prove with each SDK's focused fixture tests for failed initialize, finalize reason and carrier loss.
- [ ] 5.3 Update `docs/architecture.md` (Managed Process Ownership, Guest Entries and Reported Declarations, Call Mapping and Message and Enum Mapping rows for Instance and guest lifecycle), `docs/plugin-manager.md` and the four SDK guides for plugin phases. Prove with `mise run check -- --repository`.

## 6. Qualified Paths

- [ ] 6.1 Remove every import alias and path override in `sdk/` (sources, generators and their templates, tests, fixtures, examples) and `tools/conformance`, referring to types and modules by qualified path; generators stop emitting aliases. Accept when the inventory shows only named exceptions; prove with each SDK's focused fixture build, `mise run generate -- --sdk rust`, then python, csharp, cpp, and `mise run check -- --repository`.
- [ ] 6.2 Remove the remaining aliases and path overrides in `crates/`, `tools/` and `examples/`, moving shared test support into ordinary module trees or a dev-only crate where `#[path]` shares it today. Prove with focused builds of each touched package and the touched stories.
- [ ] 6.3 Record the no-alias, no-path-override rule and each named exception with its reason in `docs/architecture.md` and the four SDK guides. Prove with `mise run check -- --repository`.

## 7. Integration

- [ ] 7.1 Run `openspec validate plugin-lifecycle --strict`, the format task and the Rust, Python, C# and C++ language checks on the combined tree, and confirm every source item above is closed; report any pending native platform or hosted proof explicitly.

## Workflow Follow-Up

- C++ generation currently rebuilds the complete protocol library after rewriting
  generated sources even for renderer-only changes. Avoiding unchanged output
  churn or separating descriptor generation from compilation is a tooling follow-up.

- The native SDK metadata unit harness compiles its Wasmtime dev-dependency closure.
  Separating guest compiler-contract fixtures from that harness is a feedback-loop
  opportunity outside this lifecycle patch; it needs its own owned test/package change.

- Owner decision on design.md Open Question 1 (application `prepare`, application and service `stop` hooks); a confirmed answer becomes its own change.
- A security and prerequisite phase between admission and initialize is a possible later change.
- Products adopt the revision that landed this change in their own `plugin-lifecycle` change.
- Full native Linux and Windows validation and hosted CI follow source closure under the delivery authorization.
- Archive this change after delivery and fill each new capability's Purpose in the main specs.
