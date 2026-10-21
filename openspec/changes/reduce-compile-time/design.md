# Compile-Time Decisions

## Ownership And Workflow

One writer owns this change in `harmos.wt/budget`, branch `perf/compile-time`, based on published `origin/dev`. The coordinator adopts the local signed commits into the managed-workflow review. No pushes are authorized. The unrelated release change remains untouched.

Inventory: workspace and nested Cargo manifests/locks, runtime storage/module boundaries, guest and packaging dependency consumers, runtime test harness and fixtures, affected README claims, and this OpenSpec change. Inspect boundaries first, align dependencies second, then measure and validate the combined source. Current phase: implementation. User explicitly authorizes cold measurements and listed gates.

## Storage Decision

Refused under the agreed module-only condition: runtime.rs embeds StorageManager in Runtime and Builder, boot.rs accepts it throughout startup, state/manager.rs carries storage Payload in its dispatch catalog, and streams/manager.rs and streams/reader.rs use storage Cursor. Optional storage requires a broader contract refactor than gating a storage module.

## Profile Decision

Keep the existing development profile: company stkr-rust-architecture requires line-tables-only debug information. No test-specific debug=0 exception is authorized.

## Dependency Constraints

The current published upgrade candidates do not provide one guest family: Wasmtime 50 uses wasm-tools 0.261, wit-bindgen 0.63 uses 0.262, and wit-bindgen 0.62 uses 0.259. An older aligned family requires wit-bindgen 0.61.1 and wasm-tools 0.258, dropping newer async fixes; adoption awaits the user's decision. Remove source-confirmed unused guest dependencies independently.

No ordinary upstream bump eliminates the requested duplicate families: current Cranelift and Wasmtime still require sha2 0.10 and gimli 0.33; gimli 0.34 would use hashbrown 0.17 but is outside their ranges. Arrow still uses ahash 0.8, whose latest release requires getrandom 0.3; ring requires getrandom 0.2. Retain these constraints rather than force-pin or downgrade direct dependencies.

## Harness Structure

The integration target declares no required features. It combines twelve domain roots, with plugin, sidecar and simulation domains conditional on features. Shared support source is included repeatedly under domain-local namespaces. The fixture dependency is schematic-domain; executable guest/sidecar artifacts are produced separately, not linked fixture crates to remove from this harness. A shared support namespace is a possible structural follow-up, not an authorized mechanical edit. The one-integration-binary policy remains in force.

Cargo-machete is absent from the current Harmos check task and the inspected Hub Rust sources. Its independent scan identifies unused dependencies; unrelated crate findings remain outside the guest removal scope. The guest's serde_json finding is a false positive for tests: included runtime fixture-support source uses it, so it belongs in native dev-dependencies rather than production dependencies. Guest semver and serde and schematic guest serde/serde_json have no source consumers.

## Acceptance

Preserve existing feature proofs, runtime unit behavior and storage integration stories. Dependency changes must resolve normally without forced transitive pins. Measure cold cargo build --locked --timings with six jobs and cleared compiler wrappers; remove the owned target-timings directory afterwards. Gates use the user-designated shared target directory. Do not stop external processes.

## Hub CI Assignment

Check emits metadata only using all features/all targets plus minimal-feature proofs. Product compiles each shipping artifact's explicit feature set. Tests Build compiles the selected test feature set and fixtures once into its profile/target archive; test lanes consume that archive. Preserve native Linux and Windows coverage for guest hosting. No CI edits belong here.
