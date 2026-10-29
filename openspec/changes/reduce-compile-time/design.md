# Compile-Time Decisions

## Ownership And Workflow

The guest alignment experiment belongs to `harmos.wt/runtime`, branch `chore/guest-alignment`, from `feat/next-release`. The earlier cleanup is already integrated; no pushes are authorized. Other runtime and storage implementation belongs to sibling workers, and combined validation and delivery belong to the integration orchestrator.

Inventory: workspace and nested Cargo manifests/locks, runtime storage/module boundaries, guest and packaging dependency consumers, runtime test harness and fixtures, affected README claims, and this OpenSpec change. Inspect boundaries first, align dependencies second, then measure and validate the combined source. Current phase: implementation. User explicitly authorizes cold measurements and listed gates.

## Storage Decision

Refused under the agreed module-only condition: runtime.rs embeds StorageManager in Runtime and Builder, boot.rs accepts it throughout startup, state/manager.rs carries storage Payload in its dispatch catalog, and streams/manager.rs and streams/reader.rs use storage Cursor. Optional storage requires a broader contract refactor than gating a storage module.

## Profile Decision

The canonical-build-configuration decision supersedes the former line-tables-only profile: use Cargo's default profiles without project overrides. No test-specific debug=0 exception is authorized. Current Cargo.toml retains an empty Profiles skeleton section.

## Dependency Constraints

The current published upgrade candidates do not provide one guest family: Wasmtime 50 uses wasm-tools 0.261, wit-bindgen 0.63 uses 0.262, and wit-bindgen 0.62 uses 0.259. On 2026-10-29 the owner authorized testing the older aligned family, while reserving the final decisions on tasks 1.6 and 2.4. A guest async regression or loss of a security fix rejects the downgrade; retain only documentation commits in that case.

The experiment owns the root dependency declarations, all three Cargo locks, required guest/host API adaptations, and these design/task records. README review covers guest sandbox and async behavior claims. Proof is native Windows targeted guest/sidecar/plugin integration and async system selections, a focused workspace check, and identical cold-build/duplicate-count comparisons against the starting integration revision. No full lanes, umbrella check, CI simulation, benchmarks, pushes, other branches or sibling worktrees are authorized.

No ordinary upstream bump eliminated the requested duplicate families in the original audit: Cranelift and Wasmtime required sha2 0.10 and gimli 0.33; gimli 0.34's hashbrown 0.17 was outside their ranges. Arrow used ahash 0.8/getrandom 0.3; ring required getrandom 0.2. Retain upstream constraints rather than force-pin or downgrade direct dependencies.

## Guest Release Comparison

The newest matching stable host is [Wasmtime 49.0.4](https://github.com/bytecodealliance/wasmtime/blob/v49.0.4/Cargo.toml), whose wasm-tools requirements start at 0.258.3. [wit-bindgen 0.61.1](https://github.com/bytecodealliance/wit-bindgen/blob/v0.61.1/Cargo.toml) accepts that patch through its 0.258.0 requirements. Wasmtime 50.0.0 requires 0.261; wit-bindgen 0.62.0 requires 0.259 and 0.63.0 requires 0.262. These later minor families cannot unify normally.

| Downgrade | Newer Behavior Lost |
|---|---|
| Wasmtime 50.0.0 to 49.0.4 | Updated component cancellation/stream/future semantics; cooperative yields preventing executor starvation; lazy async host tasks; recursive concurrency across stores; borrowed-resource conversion fixes; concurrent resource dropping; compact imports; newer host callback/spawn APIs. |
| wit-bindgen 0.63.0 to 0.61.1 | Dynamic-library async vtable linking repair; immediate-yield `block_on` panic repair; spawned-task handles and revised cancellation; destruction-time wake suppression; no-std WASIp3 task TLS repair; accessor support and Rust map-lowering fixes. |

Sources: [Wasmtime 50 release notes](https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.0), [wit-bindgen 0.62 release notes](https://github.com/bytecodealliance/wit-bindgen/releases/tag/v0.62.0), [wit-bindgen 0.63 release notes](https://github.com/bytecodealliance/wit-bindgen/releases/tag/v0.63.0). These are upstream changes; whether Harmos regresses requires native execution, not an inference from the notes.

Security review on 2026-10-29: the [published Wasmtime advisories](https://github.com/bytecodealliance/wasmtime/security/advisories) identify 49.0.2/49.0.3/49.0.4 backports for September/October fixes. In particular, [async callback stack overwrite](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-32h6-97mm-8q3c) is fixed in 49.0.2, [cancelled async call panic](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-3r89-v5mq-j5xf) in 49.0.3, and [stream payload-type panic](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-9pvw-hwfg-92h5) plus [AArch64 branch-table miscompilation](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-mghf-wjfp-p5xm) in 49.0.4. The matching patched host does not lose those fixes. The wit-bindgen repository publishes no security advisories; that is not a guarantee of safety. Harmos directly enables async component hosting and uses `spawn_local` plus inter-task wakeups, making native async proof necessary. It declares no `wasmtime-wasi` dependency.

## Windows Alignment Experiment

Triage: task 1.6 was still required at the starting integration revision `6d2c8ae4` (`Cargo.toml:51,81-84` pinned Wasmtime 50.0.0, wit-bindgen 0.63.0 and wasm-tools 0.262.0). Tasks 1.1-1.5 and 2.1-2.3 were already closed and were not reimplemented. Tasks 1.6 and 2.4 remain reserved for the owner's final decision.

Both cold samples used Rust 1.99.1 on native Windows x86_64, the same `CARGO_TARGET_DIR=target/guest-alignment-cold`, `SCCACHE_DISABLE=1`, and Cargo's default profiles. Before each sample, native `cargo clean --target-dir target/guest-alignment-cold` removed only that experiment's compiler output. Timing includes the focused task's storage/Clippy preflight and test compilation; dependency downloads were completed beforehand. These are single samples on the shared machine.

```powershell
mise run build:rust:focused -- test --locked -p harmos-guest -p harmos-sidecar -p harmos-runtime --features harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc --test integration --test system --no-run
cargo metadata --locked --format-version 1 --features harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc
```

| Metric | Starting Integration | Older Aligned Candidate |
|---|---:|---:|
| Cold Task Elapsed Seconds (Passed) | 835.47 | 665.62 |
| Resolved Crates | 357 | 352 |
| Names With Multiple Versions | 20 | 15 |
| Excess Versions Beyond One Per Name | 23 | 18 |
| wasm-tools Families In Each Of The Three Locks | 0.261, 0.262 | 0.258.3 |

The candidate measured 169.85 seconds (20.3%) faster. Duplicate counts group the metadata `packages` by crate name and count distinct versions, avoiding false duplicates from Cargo host/target compilation units. `wasip2 1.0.4+wasi-0.2.12` and its runtime-only wit-bindgen 0.57.1 remain unchanged; they add no second wasm-tools family. Root and both nested locks resolve normally after changing the five existing direct pins; no unused direct dependency or transitive override was added.

`mise run build:rust:focused -- check --locked -p harmos-guest --features kit -p harmos-packaging` and `mise run build:rust:focused -- check --locked --workspace --all-features` passed. The first baseline attempt omitted `grpc`, which the guest contract system target requires; that failed selection was discarded and the clean comparison above includes `grpc` on both sides. No Rust source adaptation has been needed. README, guide and consumer-skill review found no version-specific claims to migrate.

Native fixture producers used `mise run build:rust:focused -- build --locked --profile dev` for `cargo-harmos`, the sandbox guest, and Schematic guest/sidecar (the two guests additionally select `--target wasm32-unknown-unknown` and their nested manifests). The built CLI collected their Cargo JSON, metadata and SHA-256 producer receipts without compilation. `HARMOS_FOCUSED_FIXTURES` selected copied, collected artifacts for execution; no shipped artifacts, deadlines, budgets or tests were weakened.

```powershell
$selection = @('--locked', '-p', 'harmos-guest', '-p', 'harmos-sidecar', '-p', 'harmos-runtime', '--features', 'harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc')
mise run build:rust:focused -- test @selection --test integration -- plugin:: sidecar:: attach:: kit:: transport:: world_hash_unchanged inventory_owner_isolation
mise run build:rust:focused -- test @selection --test integration --no-fail-fast -- plugin::activation::embedded_wire_contracts sidecar::grants::external_request_grants attach:: kit:: transport:: --test-threads 1
mise run build:rust:focused -- test @selection --test system -- plugin:: guest:: guest_contract:: sidecar:: --test-threads 1
```

Required proof failed. Unique candidate integration results were guest 2 passed, runtime 118 passed/2 failed, and sidecar 24 passed. Both runtime failures reproduced serially: `plugin::activation::embedded_wire_contracts` (`tests/plugin/activation.rs:172`) and `sidecar::grants::external_request_grants` (`tests/sidecar/grants.rs:258`) receive gRPC Internal with preparation-credit exhaustion. Their test setup selects 4096 preparation bytes; no assertion or budget was changed to suppress the failures. Whether they also fail on the original family has not been established by execution.

Candidate system results were guest SDK lifecycle 1 passed and runtime 26 passed/1 failed. Async guest calls, dependency cancellation, child retirement, fuel/memory/deadline limits, gRPC, publications, cleanup and sidecar composition passed. `guest_contract::operations::exact_live_bindings` (`tests/system/guest_contract/operations.rs:236`) fails because initialization returns `Ok(())` where refusal is required. The sandbox has no special `"mismatch"` configuration branch.

After restoring the original manifest and all three locks, rebuilding the original wit-bindgen 0.63.0 fixtures, and running `mise run build:rust:focused -- test @selection --test system -- guest_contract::operations::exact_live_bindings --test-threads 1` against Wasmtime 50.0.0, the same guest test failed identically. Thus this failure is established on both families, not evidence of a newly introduced async regression. It remains a failed required proof; the native gRPC failures likewise remain unresolved and unwaived.

Decision: do not commit the downgrade because every required proof must pass. Only this experiment's documentation is retained; all five dependency pins and three locks are restored, no Rust source or test is changed, and no push is authorized. Native Cargo cleanup removes only the experiment's cold compiler output; collected evidence is preserved separately. Tasks 1.6 and 2.4 remain unchecked. Owner question: retain the newer family (recommended until proof is green), or assign integration repairs for the three failing proofs and then retry the older family? Those repairs exceed dependency-alignment/source-breakage scope.

## Harness Structure

The integration target declares no required features. It combines twelve domain roots, with plugin, sidecar and simulation domains conditional on features. Shared support source is included repeatedly under domain-local namespaces. The fixture dependency is schematic-domain; executable guest/sidecar artifacts are produced separately, not linked fixture crates to remove from this harness. A shared support namespace is a possible structural follow-up, not an authorized mechanical edit. The one-integration-binary policy remains in force.

Cargo-machete now runs through mise.toml's check:rust task. The subsequent workspace dependency/tool refresh is integrated; it does not resolve the pending guest-family choice. The guest's serde_json finding was a false positive for tests: included runtime fixture-support source uses it, so it belongs in native dev-dependencies rather than production dependencies. Guest semver and serde and schematic guest serde/serde_json have no source consumers.

## Acceptance

Preserve existing feature proofs, runtime unit behavior and storage integration stories. Dependency changes must resolve normally without forced transitive pins. The completed cleanup's cold before/after proof remains in its implementation commits. New work uses project mise tasks and their standard preflight, without worker limits or redirected compiler output. This assignment permits targeted proof only; the integration orchestrator owns combined validation. Do not stop external processes.

## Hub CI Assignment

Check emits metadata only using all features/all targets plus minimal-feature proofs. Product compiles each shipping artifact's explicit feature set. Tests Build compiles the selected test feature set and fixtures once into its profile/target archive; test lanes consume that archive. Preserve native Linux and Windows coverage for guest hosting. No CI edits belong here.
