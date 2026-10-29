# Compile-Time Decisions

## Ownership And Workflow

The guest alignment experiment belongs to `harmos.wt/runtime`, branch `chore/guest-alignment`, from `feat/next-release`. The earlier cleanup is already integrated; no pushes are authorized. Other runtime and storage implementation belongs to sibling workers, and combined validation and delivery belong to the integration orchestrator.

Inventory: workspace and nested Cargo manifests/locks, runtime storage/module boundaries, guest and packaging dependency consumers, runtime test harness and fixtures, affected README claims, and this OpenSpec change. Inspect boundaries first, repair failed proofs, align dependencies, then measure and validate the combined source. User explicitly authorizes cold measurements and listed gates.

## Storage Decision

Refused under the agreed module-only condition: runtime.rs embeds StorageManager in Runtime and Builder, boot.rs accepts it throughout startup, state/manager.rs carries storage Payload in its dispatch catalog, and streams/manager.rs and streams/reader.rs use storage Cursor. Optional storage requires a broader contract refactor than gating a storage module.

## Profile Decision

The canonical-build-configuration decision supersedes the former line-tables-only profile: use Cargo's default profiles without project overrides. No test-specific debug=0 exception is authorized. Current Cargo.toml retains an empty Profiles skeleton section.

## Dependency Constraints

The published upgrade candidates do not provide one guest family: Wasmtime 50 uses wasm-tools 0.261, wit-bindgen 0.63 uses 0.262, and wit-bindgen 0.62 uses 0.259. On 2026-10-29 the owner approved adopting the older aligned family after root repairs for the failed required proofs. A lost upstream fix that cannot be preserved by adapting Harmos remains a stop condition.

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

## Windows Alignment Proof

Triage: task 1.6 was still required at the starting integration revision `6d2c8ae4` (`Cargo.toml:51,81-84` pinned Wasmtime 50.0.0, wit-bindgen 0.63.0 and wasm-tools 0.262.0). Tasks 1.1-1.5 and 2.1-2.3 were already closed and were not reimplemented. The owner's subsequent adoption decision requires both root repairs and passing proof before tasks 1.6 and 2.4 close.

The final cold pair uses the repaired tree at `4217b48b` before changing dependencies, after merging `feat/next-release` in `087641cf`. Both samples used Rust 1.99.1 on native Windows x86_64, the same `CARGO_TARGET_DIR=target/guest-alignment-final-cold`, `SCCACHE_DISABLE=1`, and Cargo's default profiles. The target was empty for the first sample; native `cargo clean --target-dir target/guest-alignment-final-cold` removed only that output before the second. Timing includes the focused task's storage/Clippy preflight and test compilation; dependency downloads were completed beforehand. These are single samples on the shared machine.

```powershell
mise run build:rust:focused -- test --locked -p harmos-guest -p harmos-sidecar -p harmos-runtime --features harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc --test integration --test system --no-run
cargo metadata --locked --format-version 1 --features harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc
```

| Metric | Repaired Wasmtime 50 | Aligned Wasmtime 49 |
|---|---:|---:|
| Cold Task Elapsed Seconds (Passed) | 1003.58 | 1068.31 |
| Resolved Crates | 357 | 352 |
| Names With Multiple Versions | 20 | 15 |
| Excess Versions Beyond One Per Name | 23 | 18 |
| wasm-tools Families In Each Of The Three Locks | 0.261, 0.262 | 0.258.3 |

The final pair measured the aligned task 64.73 seconds (6.45%) slower. Cargo reported test compilation at 12m57s versus 12m07s, but Clippy at 3m43s versus 5m36s. The earlier pre-repair experiment at integration revision `6d2c8ae4` measured 835.47 versus 665.62 seconds (20.3% faster) with the same command and a separate emptied target. Shared-machine single samples do not establish a repeatable speedup; the reduction in resolved duplicate families is deterministic.

Duplicate counts group the metadata `packages` by crate name and count distinct versions, avoiding false duplicates from Cargo host/target compilation units. `wasip2 1.0.4+wasi-0.2.12` and its runtime-only wit-bindgen 0.57.1 remain unchanged; they add no second wasm-tools family. Root and both nested locks resolve normally after changing the five existing direct pins; no unused direct dependency or transitive override was added. All 39 changed crate names belong to the guest/compiler family or its macro dependencies.

`mise run build:rust:focused -- check --locked --workspace --all-features` passed on the aligned family. No downgrade-specific Rust source adaptation was needed. README, guide and consumer-skill review found no version-specific claims to migrate.

Native fixture producers used `mise run build:rust:focused -- build --locked --profile dev` for `cargo-harmos`, the sandbox guest, and Schematic guest/sidecar (the two guests additionally select `--target wasm32-unknown-unknown` and their nested manifests). The built CLI collected their Cargo JSON, metadata and SHA-256 producer receipts without compilation. `HARMOS_FOCUSED_FIXTURES` selected copied, collected artifacts for execution; no shipped artifacts, deadlines, budgets or tests were weakened.

```powershell
$selection = @('--locked', '-p', 'harmos-guest', '-p', 'harmos-sidecar', '-p', 'harmos-runtime', '--features', 'harmos-runtime/plugin-guest,harmos-runtime/plugin-sidecar,harmos-runtime/grpc')
mise run build:rust:focused -- test @selection --test integration -- plugin:: sidecar:: attach:: kit:: transport:: world_hash_unchanged inventory_owner_isolation
mise run build:rust:focused -- test @selection --test system -- plugin:: guest:: guest_contract:: sidecar:: --test-threads 1
```

Final aligned Windows results: integration guest 2, runtime 120 and sidecar 24 passed (146 total); system guest SDK lifecycle 1 and runtime 27 passed (28 total). This includes both gRPC admission repairs, exact live bindings, async guest calls, dependency cancellation, child retirement, fuel/memory/deadline limits, gRPC, publications, cleanup and sidecar composition. All required gates passed with freshly collected aligned fixtures; tasks 1.6 and 2.4 close under the owner's adoption decision. No upstream dependency prevented either repair, and no reviewed published security fix is lost.

The initial candidate exposed three failed proofs. Both gRPC failures (`plugin::activation::embedded_wire_contracts` and `sidecar::grants::external_request_grants`) reproduced serially with preparation-credit exhaustion. `guest_contract::operations::exact_live_bindings` initialized successfully despite requesting mismatch mode. All three failures also reproduced on Wasmtime 50.0.0 with wit-bindgen 0.63.0 fixtures; none was waived as pre-existing.

Owner decision after the experiment: merge `feat/next-release`, repair the inert sandbox mismatch mode and reproduce/fix the two gRPC admission failures on the current family before alignment. The locked repair inventory is the sandbox fixture, its system assertion description, the raw gRPC encoder and inline deterministic admission tests, all three dependency locks, root pins, and this change's records. No assertion, budget or deadline may be weakened. Repairs and alignment are separate commits; tasks 1.6 and 2.4 close only after all required proof passes. No push is authorized.

Repair 1.7 restores the sandbox's inert `"mismatch"` mode by staging a real extra handler during preparation. Production exact-catalog validation rejects it; the system test preserves its refusal condition and updates only the diagnostic description. The repaired story and SDK lifecycle smoke passed on Wasmtime 50.0.0 with freshly collected wit-bindgen 0.63.0 fixtures. No public API, deadline, budget, README claim or consumer-skill contract changes.

Repair 1.8 reproduced both native gRPC failures on Wasmtime 50.0.0 after the merge. `grpc/native.rs` used the cooperative encoding fallback, which admits the entire wire ceiling even when copying a small, already encoded payload. Raw payloads and native response envelopes now use the existing admitted copy/framing helpers for actual allocation peaks. The original 4096-byte budgets are unchanged; both transport reproductions and two pure tests for bounded success, exact wire bytes and refusal before allocation pass. This is a Harmos admission defect independent of upstream guest versions.

## Harness Structure

The integration target declares no required features. It combines twelve domain roots, with plugin, sidecar and simulation domains conditional on features. Shared support source is included repeatedly under domain-local namespaces. The fixture dependency is schematic-domain; executable guest/sidecar artifacts are produced separately, not linked fixture crates to remove from this harness. A shared support namespace is a possible structural follow-up, not an authorized mechanical edit. The one-integration-binary policy remains in force.

Cargo-machete now runs through mise.toml's check:rust task. The subsequent workspace dependency/tool refresh is integrated; the owner-approved alignment resolves its guest-family duplication. The guest's serde_json finding was a false positive for tests: included runtime fixture-support source uses it, so it belongs in native dev-dependencies rather than production dependencies. Guest semver and serde and schematic guest serde/serde_json have no source consumers.

## Acceptance

Preserve existing feature proofs, runtime unit behavior and storage integration stories. Dependency changes must resolve normally without forced transitive pins. The completed cleanup's cold before/after proof remains in its implementation commits. New work uses project mise tasks and their standard preflight, without worker limits or redirected compiler output. This assignment permits targeted proof only; the integration orchestrator owns combined validation. Do not stop external processes.

## Hub CI Assignment

Check emits metadata only using all features/all targets plus minimal-feature proofs. Product compiles each shipping artifact's explicit feature set. Tests Build compiles the selected test feature set and fixtures once into its profile/target archive; test lanes consume that archive. Preserve native Linux and Windows coverage for guest hosting. No CI edits belong here.
