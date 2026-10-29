# Compile-Time Decisions

## Ownership And Workflow

The remaining assigned work belongs to `harmos.wt/next`, branch `feat/next-release`. The earlier cleanup is already integrated; no pushes are authorized. Runtime and storage implementation belongs to sibling workers, and combined validation and delivery belong to the integration orchestrator.

Inventory: workspace and nested Cargo manifests/locks, runtime storage/module boundaries, guest and packaging dependency consumers, runtime test harness and fixtures, affected README claims, and this OpenSpec change. Inspect boundaries first, align dependencies second, then measure and validate the combined source. Current phase: implementation. User explicitly authorizes cold measurements and listed gates.

## Storage Decision

Refused under the agreed module-only condition: runtime.rs embeds StorageManager in Runtime and Builder, boot.rs accepts it throughout startup, state/manager.rs carries storage Payload in its dispatch catalog, and streams/manager.rs and streams/reader.rs use storage Cursor. Optional storage requires a broader contract refactor than gating a storage module.

## Profile Decision

The canonical-build-configuration decision supersedes the former line-tables-only profile: use Cargo's default profiles without project overrides. No test-specific debug=0 exception is authorized. Current Cargo.toml retains an empty Profiles skeleton section.

## Dependency Constraints

The current published upgrade candidates do not provide one guest family: Wasmtime 50 uses wasm-tools 0.261, wit-bindgen 0.63 uses 0.262, and wit-bindgen 0.62 uses 0.259. An older aligned family requires wit-bindgen 0.61.1 and wasm-tools 0.258, dropping newer async fixes; adoption awaits the user's decision. Remove source-confirmed unused guest dependencies independently.

No ordinary upstream bump eliminated the requested duplicate families in the original audit: Cranelift and Wasmtime required sha2 0.10 and gimli 0.33; gimli 0.34's hashbrown 0.17 was outside their ranges. Arrow used ahash 0.8/getrandom 0.3; ring required getrandom 0.2. Retain upstream constraints rather than force-pin or downgrade direct dependencies.

## Harness Structure

The integration target declares no required features. It combines twelve domain roots, with plugin, sidecar and simulation domains conditional on features. Shared support source is included repeatedly under domain-local namespaces. The fixture dependency is schematic-domain; executable guest/sidecar artifacts are produced separately, not linked fixture crates to remove from this harness. A shared support namespace is a possible structural follow-up, not an authorized mechanical edit. The one-integration-binary policy remains in force.

Cargo-machete now runs through mise.toml's check:rust task. The subsequent workspace dependency/tool refresh is integrated; it does not resolve the pending guest-family choice. The guest's serde_json finding was a false positive for tests: included runtime fixture-support source uses it, so it belongs in native dev-dependencies rather than production dependencies. Guest semver and serde and schematic guest serde/serde_json have no source consumers.

## Acceptance

Preserve existing feature proofs, runtime unit behavior and storage integration stories. Dependency changes must resolve normally without forced transitive pins. The completed cleanup's cold before/after proof remains in its implementation commits. New work uses project mise tasks and their standard preflight, without worker limits or redirected compiler output. This assignment permits targeted proof only; the integration orchestrator owns combined validation. Do not stop external processes.

## Hub CI Assignment

Check emits metadata only using all features/all targets plus minimal-feature proofs. Product compiles each shipping artifact's explicit feature set. Tests Build compiles the selected test feature set and fixtures once into its profile/target archive; test lanes consume that archive. Preserve native Linux and Windows coverage for guest hosting. No CI edits belong here.
