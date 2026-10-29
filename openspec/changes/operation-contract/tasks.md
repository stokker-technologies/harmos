## 1. Guest Contract Completion

- [x] 1.1 Re-export the shared SDK Operation and Context; isolate kit lifecycle Context and remove guest-root Handler/ExecutionContext
  authoring exports.
- [x] 1.2 Migrate all seven sandbox guest handlers to request-owned operations in vertical files, declared resources and callees; remove
  superseded per-handler lifecycle probes and replace with request/attempt proofs.
- [x] 1.3 Migrate guest carrier consumers, scaffold templates, shared fixture declarations and affected host assertions; review README and
  update guest guide and skill references.

- [x] 1.4 Remove eager per-operation registration helpers that require remote-only declarations to implement execution; keep catalog
  registration and explicit RegistrationBatch::operation.

- [x] 1.5 Retain genuine format:rust output and revert unrelated aggregate-format/Hub-render changes.

- [x] 1.6 Align sandbox schema build optimization with the schematic guest, retain fuel limits, and repair guest facade contexts and
  qualified encoded-call assertions found by focused proof.

## 2. Targeted Proof

- [x] 2.1 Run format:rust and focused harmos-guest, harmos-macros and harmos-sdk tests.
- [x] 2.2 Build the Wasm sandbox and schematic guest using project tasks; check schematic consumer crates.
- [x] 2.3 Run the affected runtime guest operation stories against the refreshed components.
- [x] 2.4 Commit closed steps locally with the requested trailer; no push or PR.
- [ ] 2.5 Retire with `stkr specification finish operation-contract` after Hub passes `--type change` to OpenSpec validation; the current command refuses the name shared by this change and its main spec.

## 3. Workspace Clean Break

- [x] 3.5 Rename the shared execution snapshot and all references without aliases; focused five-package compilation succeeds.
- [x] 3.6 Emit declared resources for execution-only runtime/SDK macro paths and migrate SDK/resource unit fixtures; preserve runtime gating for reusable guest declarations. Focused SDK and macro tests pass.

- [x] 3.1 Audit every remaining lifecycle-shaped implementation; migrate runtime test support, tests, benches and sidecar fixtures to request-owned operations with declared resources, streams and dotted IDs.
- [x] 3.2 Replace the legacy handler trait with declaration-bound internal carriers; rename the shared execution snapshot and migrate SDK/runtime/service consumers and exports without compatibility aliases.
- [x] 3.3 Migrate lifecycle built-ins and inspect guest fixtures, examples, scaffold templates and README claims; remove superseded operation lifecycle stories.
- [x] 3.4 Run the affected feature-enabled tests and the requested five-package focused tests, Wasm/guest fixture builds, schematic consumer check and Rust formatting; record signature counts and commit closed steps locally.

- [x] 3.7 Migrate shared compute/policy fixtures, stateless native and sidecar operations, owned-response fixtures, dispatch/throughput fixtures and SDK/runtime unit fixtures; repair the tracing fixtures.
- [x] 3.8 Migrate remaining stateful/streaming handler fixtures in operation wire, plugin, sidecar, guest host, listener and service tests; preserve carrier settlement coverage while removing authored initialization/cleanup assumptions.

- [x] 3.9 Retire per-operation initialization/finalization stories superseded by consuming attempts; add deterministic cancellation/drop, retry cloning and stream settlement coverage while retaining resource/plugin lifecycle stories.

- [x] 3.10 Complete feature-boundary imports and manual-codec Clone requirements found by fixture compilation; retain codec composition proof.

## 4. Authorized Consolidation And Refresh

- [x] 4.1 Merge storage facilities into feat/operation-contract; preserve storage/sidecar behaviors and tests, migrate incoming operations, and reconcile affected guides and examples.
- [x] 4.2 Remove unused dependencies through check:rust, upgrade workspace dependencies including majors, and refresh project lockfiles.
- [x] 4.3 Upgrade mise tools and regenerate mise.lock with mise.
- [x] 4.4 Run the requested seven-package focused tests, Wasm/guest builds, consumer check and all formatters; repair failures.
- [x] 4.5 Push the combined candidate, verify remote SHA, open the authorized dev PR and enable squash auto-merge.
- [x] 4.6 Run default ci:simulate once and report each job truthfully; hosted checks are handed off without polling. Source, Python and both Rust checks passed; Product and Tests builds failed on both platforms before compilation with a cache-wrapper RuntimeError; 14 dependent jobs were blocked. Full local proof remains failed; shared tooling repair is excluded by the owner.

## 5. Runtime Follow-Ups

- [x] 5.1 Reconcile the parked changes and attached runtime follow-ups with landed source; retain one-line evidence in the owning designs. Preserve delivery-only obligations.
- [ ] 5.2 Bind an existing executable operation type under a runtime-computed ID through the existing registration path. Type-owned schema, policy, resources and handler remain authoritative; no arbitrary runtime schemas/handlers. Locked inventory: domain operations/{mod,catalog,contract}.rs; SDK operations/registration.rs; runtime operations/{registration,surface,mod}.rs, manager/submission.rs, runtime.rs and plugin/host.rs; focused registration/dispatch and factory validation fixtures; authoring reference, generated copies and product guide. Cover metadata/fingerprint, typed and encoded dispatch, invalid IDs, duplicates and catalog collisions.
- [x] 5.3 Close async future depth headroom without a source change by owner decision (2026-10-29). Reopen only with a failing reproducer or measured consumer budget miss; existing dispatch boxing alone is not evidence of a defect.
- [x] 5.4 Close execution sealing latency without a source change by owner decision (2026-10-29). Reopen only with a failing reproducer or measured consumer budget miss; do not infer a runtime optimization from the earlier storage finish/flush observations.
- [x] 5.5 Accept observer cadence constant expressions with nonzero validation. Updated harmos-macros/src/artifact/observer.rs, harmos-sidecar/src/kit/observer.rs, product guide and authoring plugin reference plus canonical generated copies. The focused sidecar cadence test passes; a scratch consumer checked through build:rust:focused rejects a zero constant with E0080. Rust formatting and strict change validation pass. README claims are unaffected; static cadence and data v1 remain unchanged.
- [ ] 5.6 Admit the plugin owner `host` as an ordinary owner, without a reservation or owner-name privilege (owner decision 2026-10-29). Audit plugin and operation admission; remove owner-name special cases and add a regression proving ordinary admission, duplicate refusal and equivalent permissions. Keep runtime caller authority distinct from a plugin's chosen name.
