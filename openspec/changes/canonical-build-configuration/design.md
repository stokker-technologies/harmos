# Canonical Build Design

## Parallel Producer Decision

Product and Tests now use separate parallel provider jobs, superseding single-job consolidation. Independent feature sets are permitted, but CI remains canonical-only and extended configurations remain local opt-in. Tests Build must take no longer than Product Build on the same candidate/platform/profile; measured breaches require investigation and repair of redundant compilation, feature sprawl or testing structure, not silent coverage removal or using correctness-test deadlines as performance criteria. Preserve actual artifact dependencies and report relocated work in the full critical path. Repository owner must reconcile obsolete single-job clauses before closure. The latest user directive permits superseding existing pipelines after the new candidate passes its local gate.

## Superseding User Decision

All provider CI contexts cover only the canonical shipping configuration. Simulation and other noncanonical feature/target compatibility coverage are local-only, explicitly requested extended validation. Preserve shipping behavior and all test sources. Keep canonical-capable tests in CI rather than excluding whole packages. Repository-owned `build:rust:compatibility` is a local compiler-proof helper; `test:rust:extended` is the optional local behavioral suite and arranges its needed builds. Ordinary installers and CI never invoke extended tasks implicitly. Existing pipelines may be superseded by a locally proven candidate under the latest user directive.

## Locked Decisions

Clippy runs before compilation with matching feature, target and profile settings and child-scoped warning denial. Independent parallel Product and Tests jobs use separate checkout-local Cargo directories; cache transport may reuse compatible outputs but is not an artifact dependency. Installer tasks request Product only. Test and Package consume exported artifacts without compilation.

Mise owns canonical configurations per platform/profile; independent producer feature sets are permitted. Preserve shipping defaults; do not expand SDK APIs or dependencies to accommodate tests. Root Product and Tests use workspace defaults. Use default Cargo profiles: Product dev/release, Bench bench, and every Tests compilation and matching Clippy invocation test, including release pipelines. Remove manifest, package and build-environment optimization overrides; graph profile namespaces remain dev/release independently of compiler profile. Product exports libraries already compiled for the shipping schematic's two carriers while its compiler capture paths exist. Tests independently builds its archive, the default-feature Cargo CLI needed by sandbox collection, the sandbox component, and offline inputs. The optional fixture preparation joins both manifests and copies inert inspection bytes without compilation. Avoid a new generic framework or duplicating feature lists in CI. Preserve noncanonical stories and matching Clippy in the optional extended suite.

The exported shipping library closure measures 986,433,545 bytes uncompressed on the existing Windows development build. This transfer moves from Tests to Product and is included in the actual critical path. Fixture preparation is a real manifest join after both producers; it is not independent of Product or a way to hide compiler time. Unit tests can use the Tests-only base fixture stamp. Other fixture consumers need the prepared combination.

Tests Build must not exceed Product Build under comparable cache conditions. The complete warm-cache pipeline must finish below 45 minutes, with 30 minutes advisory. Cold timings are reported separately. Warmth requires observed successful restoration/compiler reuse evidence, never the assumption that an earlier run warmed the cache. A breach requires reassessment without silent coverage removal; material architecture or feature changes require a user decision.

## Ownership And Delivery

The existing harmos owner implements in its current worktree, updates the inventory, and owns final local proof after source closure. Root coordinates Hub dependencies. The previous branch merged. Preserve its owned specification retirement changes, then use an authorized new fix/canonical-build-configuration branch based on current dev in this same worktree; never discard changes. Changes require protected review into dev. Do not publish releases.

Correctness tests have a minimum 30-second deadline, with larger existing bounds preserved. Historical measured runtimes never tighten these deadlines; performance is measured through benchmarks. This does not waive the separate objective that application work completes within 10 seconds. Investigate all eight observed System timeouts individually, separating fixture I/O, validation/JIT, startup and actual operations; do not assume a shared cause. Product lifecycle deadlines, fuel limits and Cargo defaults remain unchanged.

## Source Inventory

- The user directs the four existing `harmos-packaging::acceptance` `authoring::*` compiler stories into nightly-only `test:rust:starter`, using the same Tests archive, prepared fixtures and Product inputs. Ordinary acceptance excludes only that module; nightly coverage retains it. Starter tool installation runs only in the explicit starter preflight. No test source or assertion is removed. README claims are unaffected; CONTRIBUTING owns the changed validation cadence.
- `mise.toml`: canonical Product/Tests selectors, optional `build:rust:compatibility` and `test:rust:extended`, with Product-only installer dependencies unchanged.
- The authored `ci:simulate` task replaces the duplicated sequential `ci` recipe with the rendered provider graph, using the canonical pipeline/platform/executor/resource flags. The authored pull-request template and contributor instructions name that gate.
- `stkr.toml`: named Product, Tests archive and fixture outputs; preparation consumes Product plus Tests fixtures, Unit consumes only the archive, Integration adds prepared fixtures, and Acceptance/System also consume Product. Tests exports only the exact archive and compiler metadata paths, excluding retained local listing diagnostics.
- Explicit inputs also cover generated Python checking, canonical coverage/benchmarks, package assembly and existing package-consuming release jobs. `crates/cargo-harmos/templates/{guest,sidecar}/stkr.toml.tmpl` declares the corresponding minimal Product/Tests/unit/coverage/package artifact contracts so generated starter CI remains complete.
- The same file's benchmark producer/category selectors use shipping defaults in CI; optional `bench:rust:extended` retains existing all-feature benchmark execution locally. Benchmark execution is not a delivery gate.
- `tools/python/src/harmos_tools/build/__main__.py`: the existing root `build` task's Product command. Export actual shipping libraries with their hashes while compiler paths exist.
- `tools/python/src/harmos_tools/build/rust/tests.py`: `build:rust:tests`, independent archive, narrow default CLI, sandbox and offline-input production; no Product reads.
- `tools/python/src/harmos_tools/prepare/tests/fixtures.py`: `prepare:tests:fixtures`, checksummed manifest join after both artifacts arrive, with no Cargo invocation. Acceptance preflight installs only the union of tools declared by the embedded guest/sidecar starter manifests, excluding host-global tools while retaining verification settings.
- `tools/python/src/harmos_tools/build/fixtures.py`: shared capture/staging helpers with no public task CLI. Keep existing package and preflight imports valid. Retain schematic and sandbox selectors and all assertions.
- Managed `test/{policy,nextest,run}.py` and both embedded starter copies enforce the 30-second minimum and retain larger deadlines, without historical-duration deadline tightening; no authored caller retains the retired nextest durations argument.
- Hub-owned generated guidance, native Build workflows and embedded guest/sidecar starter copies: canonical-only CI policy, regenerated after the shared owner hands off. `cargo-harmos/build.rs` embeds starter template bytes into the shipping executable, so any starter template change invalidates Product proof as well as Tests/configuration proof. The simulator preserves Git-index executable/symlink modes and declared artifact file/directory modes across native/Docker transport; generated workflows and release rehearsal use the pnpm 12 cache environment name. The generated step-environment helper imports bootstrap ENV/PATH as data for persistent native/Docker job shells. Staging preserves effective Git line-ending settings, and hosted planner/required-gate bootstrap selects only Python with the action's boolean install input. Parallel jobs share the mise cache directory that owns install locks alongside shared installs/downloads. Native Windows caches use a short per-repository root; diagnostic test reports are retained before successful or failed native/Docker workspace cleanup.
- Authored Product uses shared `build.cargo.artifacts` and `build.cargo.record_target`; Tests uses `build.cargo.archive`. The retired `build.run` producer and `test.archive.build` compiler entry point have no authored callers. `test.archive` remains the fixture stamp/archive reader owner. Shared generation retires obsolete managed modules; it preserves authored task modules.
- `README.md`, `CONTRIBUTING.md`, `docs/product-guide.md`, `examples/schematic/README.md`, this change's requirements, and maintained `openspec/specs/native-ci/spec.md`: canonical CI configuration, current released assets and optional local extended coverage. Preserve the detailed authored examples while correcting verified executable paths and fixture-producer ownership.
- Remove profile overrides from `Cargo.toml`, `examples/schematic/Cargo.toml`, `crates/harmos-guest/tests/fixtures/sandbox/Cargo.toml`, and authored guest/sidecar starter Cargo manifests. Preserve story coverage and feature defaults. Extend native `BuildProfile` parsing and captured-output debug-directory recognition to the default Cargo test profile, retaining the actual compiler profile in receipts.
- Preserve the completed `shared-native-build` retirement metadata and leave `reduce-compile-time` untouched.

## Windows System Investigation

The existing canonical archive's eight failures are individually inventoried below. The seven guest stories compile their selected component before initialization. The current single-story probe preserves the 30-second correctness guard and fails at 30.022 seconds, with one Nextest test thread and an eight-CPU Job Object rate limit. Dokime validation was active concurrently, so this is diagnostic failure evidence rather than a standalone performance measurement. No application deadline or compiler profile changed.

| Story | Fixture | Known Phase And Remaining Measurement |
|---|---|---|
| `publication_crosses_row` | Sandbox | File read 8ms; separate validation 746ms; standalone JIT did not finish within the earlier 10–16s probes. Current unchanged story exceeds 30s. Initialization and operations remain unmeasured. |
| `registration_precedes_init` | Sandbox | Same `instantiate` path at `crates/harmos-guest/tests/system/main.rs:217`; individual JIT, initialization and operation timing remains unmeasured. |
| `refusal_preserves_reason` | Sandbox | Same `initialize`/`instantiate` path; individual phase timing remains unmeasured. |
| `shutdown_current_context` | Sandbox | Same `instantiate` path before shutdown; individual phase timing remains unmeasured. |
| `submitted_children_controls` | Sandbox | Same initialization path before submitted-child operations; individual phase timing remains unmeasured. |
| `unbound_operations_refused` | Schematic | Same `instantiate` path with the separate Schematic component; individual phase timing remains unmeasured. |
| `plugin::sdk_lifecycle_retained` | Schematic | Test-local Wasmtime `Component::new` in `crates/harmos-guest/tests/system/plugin.rs`, followed by SDK lifecycle calls; individual phase timing remains unmeasured. |
| `schematic::cli_backends_reopen` | Schematic Sidecar And Guest | Runs the real CLI with both carriers; individual CLI startup, guest JIT and operation timing remains unmeasured. |

The simulator currently controls aggregate native CPU rate and RAM, but not processor affinity or Nextest/Rayon worker discovery. A reserved eight-CPU job can still discover all host processors; that shared infrastructure finding belongs to Hub. It does not establish the cause of any individual timeout. Keep 1.7 and the under-10-second application objective open until measurements distinguish default-profile host JIT from worker contention and actual application work. The original complete simulation is `c8b98a022a2d`; its summary records failed Linux builds and Windows System, so retained successes require the affected repair closure rather than a claim of a green gate.

## Compatibility Selection

Move the existing three Wasm helper Clippy checks, workspace no-default-feature all-targets Clippy, workspace all-feature examples/benches Clippy, and schematic all-targets Clippy into `build:rust:compatibility --profile dev|release`. Preserve their exact selectors. `test:rust:extended` runs Product, this helper, the existing all-feature archive/fixtures producer with matched Clippy, fixture preparation, and all test lanes. This replaces the local archive selection rather than adding a second archive protocol. CI never calls either optional task. Use the default test profile for test-oriented compatibility checks.

## Extended Coverage Inventory

Existing Rust feature guards select the following runtime coverage only in the all-feature extended suite:

- Integration `plugin`, `sidecar` and `simulation` story modules; state and stream `listeners`; storage `operations::plugins`.
- Operations wire `artifact`, `host` and `plugin` modules require carrier/plugin features; `dynamic`, `grpc` and `grpc_manager` require gRPC.
- Operations wire endpoint stories `half_close_joins`, `settlement_never_wait`, `failure_preserves_cleanup`, `input_drop_cancels`, `deadline_settles_peer`, `grpc_exchange_polls`, `failure_retains_cleanup`, `exchange_drop_cancels` and `opening_admission_exchange` require gRPC. Their neighboring canonical endpoint stories remain in CI.
- Acceptance `storage` requires gRPC. System `guest`, `guest_contract` and `sidecar` require their respective carrier features; canonical runtime and shipping schematic system stories remain in CI.
- Existing conditional plugin assertions in stream macro tests execute in extended validation; the canonical portions of those tests remain in CI.
- Runtime `integration-sidecar`, `system-sidecar` and `integration-simulation` benchmark targets retain their existing required-feature guards and execute through local extended benchmarks rather than ordinary nightly benchmarks.

No test source, assertion or whole package is excluded. Macro authoring stories retain their full fixture selection using the shipping schematic's compiler capture.

## Product Transport Closure

Product exports both `target/generated/python/pyproject.toml` and `target/generated/python/src`. The package manifest is part of the checksummed Product catalog consumed by fixture preparation and packaging; exporting only the source subtree leaves those consumers incomplete on a fresh job. Every catalog hash and artifact/library/build selector must resolve using only declared Product outputs. An actual declared-output archive pack/extract verified all 296 catalog hashes through both consumer preflight functions, without a checkout or compiler outputs outside the exported paths.

Generated-starter acceptance commands remove the parent lane's `MISE_TASK_SKIP` alongside the existing tool and test environment isolation. The outer CI lane still skips its completed producer tasks; each independent starter must execute its own explicit Product and Tests builds before running its unit tests.

The shared Test and Coverage runner sets child-only `STKR_TEST_PROFILE` from its parsed graph-profile flag, preserving other child variables and the parent environment. Authored mise tasks pass only the existing profile argument; task-level environment templates cannot read usage arguments. Runtime fixtures validate dev/release and use that namespace independently of the harness's always-unoptimized compiler profile. Direct Cargo invocations default to dev. Compiler debug assertions never select transported fixture paths.

Default-profile guest startup must fit the unchanged host fuel limit. The default-dev Schematic guest exhausted fuel in discarded lifecycle descriptor reconstruction while the original guest passed on the same shipping host with an identical operation inventory. `Descriptor` has private fields, validated constructors and deserialization, and immutable accessors; inventory validation therefore retains lifecycle order without reconstructing those typed descriptors. Guest initialization retains and reuses its single markers result, preserving `black_box` retention. The SDK moves its consumed lifecycle descriptor into the handler's owned contract through `From<Descriptor> for Contract`, avoiding a discarded deep clone after the action check. Two private `OnceLock<Contract>` values publish immutable canonical initialization/shutdown templates; caller policy is validated and applied only to an owned clone. Permanent cache allocation is bounded to those two templates, and raw-contract comparisons remain unchanged. The collected unoptimized Schematic guest completes its validation, routing, undo/redo and reopen workflow on the unchanged shipping host. Raw-input validation and fuel/deadline settings remain intact.

## Extended Case Partition

The previous 900-case selection partitions into 637 canonical cases and the following 263 cases selected by the existing noncanonical feature guards. A namespace ending in `::*` includes every existing testcase below that namespace; individual entries identify one testcase. These cases remain in `test:rust:extended`, with their original assertions. No pre-existing canonical case is removed; new default-profile regression stories do not alter this feature partition.

| Binary | Namespace Or Case | Cases |
|---|---|---|
| `harmos-runtime` | `operations::*` | 1 |
| `harmos-runtime::acceptance` | `storage::*` | 1 |
| `harmos-runtime::integration` | `application::macros::unified_plugins::*` | 5 |
| `harmos-runtime::integration` | `observers::execution::*` | 8 |
| `harmos-runtime::integration` | `observers::host_accessor::*` | 3 |
| `harmos-runtime::integration` | `observers::lifecycle::*` | 11 |
| `harmos-runtime::integration` | `observers::native::*` | 6 |
| `harmos-runtime::integration` | `operations::execution::reboot_reruns_initialize` | 1 |
| `harmos-runtime::integration` | `operations::execution::simulation_reboot_rebinds` | 1 |
| `harmos-runtime::integration` | `operations::wire::artifact::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::authored::response_awaits_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::authored::response_drop_retains` | 1 |
| `harmos-runtime::integration` | `operations::wire::authored::response_keeps_owner` | 1 |
| `harmos-runtime::integration` | `operations::wire::dynamic::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::endpoints::deadline_settles_peer` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::exchange_drop_cancels` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::failure_preserves_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::failure_retains_cleanup` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::grpc_exchange_polls` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::half_close_joins` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::input_drop_cancels` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::opening_admission_exchange` | 1 |
| `harmos-runtime::integration` | `operations::wire::endpoints::settlement_never_wait` | 1 |
| `harmos-runtime::integration` | `operations::wire::grpc::*` | 8 |
| `harmos-runtime::integration` | `operations::wire::grpc_manager::*` | 15 |
| `harmos-runtime::integration` | `operations::wire::host::*` | 2 |
| `harmos-runtime::integration` | `operations::wire::plugin::*` | 15 |
| `harmos-runtime::integration` | `plugin::*` | 26 |
| `harmos-runtime::integration` | `runtime::plugin::*` | 6 |
| `harmos-runtime::integration` | `sidecar::*` | 72 |
| `harmos-runtime::integration` | `simulation::*` | 15 |
| `harmos-runtime::integration` | `state::listeners::*` | 8 |
| `harmos-runtime::integration` | `storage::operations::plugins::*` | 2 |
| `harmos-runtime::integration` | `streams::listeners::*` | 9 |
| `harmos-runtime::system` | `guest::*` | 1 |
| `harmos-runtime::system` | `guest_contract::*` | 30 |
| `harmos-runtime::system` | `sidecar::*` | 1 |
