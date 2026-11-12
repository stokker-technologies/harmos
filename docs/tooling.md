# Repository Tooling

Mise is the public entry point. Run `mise tasks` to see the supported entry points;
pass arguments after `--`. Python tooling lives in `tools/python/src/harmos_tools/`;
Rust executable conformance lives in `tools/conformance`; reusable build, watch,
componentization, inspection, bundle construction and generators live in `tools/packaging`.
Runtime owns verified bundle reading at `harmos_runtime::bundle` and the descriptor
projection required by its transport; its normal dependencies contain no packaging tools.
The root uv workspace owns `.venv/` and `uv.lock`. The standalone SDK owns
`sdk/python/.venv/` and `sdk/python/uv.lock`; tooling does not depend on the SDK.
Run `mise run prepare` once after checkout or dependency changes to synchronize
all locked dependency groups in both projects and install the components and targets
from `rust-toolchain.toml`. Tool versions stay pinned in `mise.toml` and `mise.lock`;
release, CI and package settings live in `tools/project.toml`.
Explicit tool configuration lives in `tools/config/`: git-cliff uses `cliff.toml`
and Taplo uses `taplo.toml`. Tasks pass these paths; editor settings select Taplo
explicitly. Mise puts Ruff caches in `tools/python/.ruff_cache/`.
Contributor guidance lives in [Contributing](CONTRIBUTING.md).
The SDK's development group includes tooling for reference-host test support;
this dependency does not enter the tooling workspace or the SDK runtime.
Every other entry point uses `uv run --locked --no-sync`.

`mise run lint -- --language rust` owns Clippy with warnings denied, including
benchmark targets and their fixtures. `mise run check -- --language rust` includes
it. Add `--extended` for optional compatibility lint; `mise run build -- --compatibility`
checks those configurations without Clippy. Build, test, bench and fixture tasks do
not run Clippy. `check --repository` checks repository configuration without a build;
`check --generated` checks existing generated Python, and ordinary `check` includes both.

## Workstation Builds

Local builds use `RUSTC_WRAPPER=sccache`, `SCCACHE_DIR=<OS user cache>/sccache`
and `SCCACHE_CACHE_SIZE=20G`. Explicit shell settings override these defaults.
`SCCACHE_CLIENT_SIDE=1` keeps compilation in the requesting client instead of
queuing it through the machine-wide server's compiler jobserver. Cargo starts or
reuses the persistent server for cache storage; tasks never restart it per invocation.
`SCCACHE_BASEDIRS` defaults to the checkout root, normalizing cache keys across
worktrees and drives. CI retains its explicitly configured cache settings.

Leave `SCCACHE_LOG` unset during ordinary builds: in the pinned sccache version,
setting it disables client-side mode. Read native statistics separately instead.
Storage location and size apply when the server starts; inspect its actual settings
before attributing misses to a checkout. Do not reset shared statistics or restart
another session's server to collect measurements.

Incremental compilation uses Cargo's default locally; only workflows disable it.
Keep it enabled for edited workspace crates. Sccache reuses matching cacheable
dependencies; it does not cache incremental crates, linked executables or procedural
macro libraries. See [sccache's Rust limits](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md).
Each worktree keeps its own `target/`; Cargo owns its normal compilation lock.
Retain that directory across edits and source revisions. Project tooling never
scans or reclaims compiler output before compilation.

```sh
mise run build -- --package harmos-domain
mise run build -- --package harmos-domain --check
mise run build -- --package harmos-runtime --lib --check
mise run build -- --package harmos-runtime --lib
mise run test -- --package harmos-runtime --target lib --exact grpc::codec::tests::absent_preparation_budget_preserves_native_encoding
mise run test -- --package harmos-runtime --features simulation --target lib --exact simulation::clock::tests::evidence_dates_retain_the_host_epoch_across_reboot
mise run test -- --package harmos-sdk-derive --target lib --exact stored_message::tests::map_field_shapes
```

Focused builds select one package; focused tests can select a library or
named integration target and use Cargo's default test profile, without an archive.
Use `build --package <name> --lib` when editing a library: it skips unrelated package
binaries, including the runtime's benchmark fixture. Omitting `--lib` retains Cargo's
normal package targets. The Tests producer still builds the complete canonical selection.

```sh
mise run test -- --package harmos-storage --target integration --story manager::files::opened_clones_keep_independent_read_offsets
mise run test -- --package harmos-protocol
mise run test -- --paths crates/harmos-storage/tests/integration/manager/files.rs
```

`--story` selects a Rust module prefix; `--exact` selects one complete test name.
Omit `--target` to run a package's test targets. `--paths` conservatively selects
root-workspace test binaries, including shared modules; product changes also select
local dependents and prepared-fixture consumers. These selections use nextest with the
existing hang guards and serial System scheduling. Add `--no-run --timings` to
measure just their compilation with Cargo's HTML timing report. Focused nextest
JUnit reports stay in `target/tests/focused/<package>/<local|system>/project/`.
Compiler-contract and guest stories still need their explicitly prepared fixtures.

Both accept `--manifest-path examples/schematic/Cargo.toml` to select the separate
example workspace while keeping compilation behind the same task interface.
Focused Cargo subprocesses set `CARGO_TARGET_DIR` to the repository's `target/`,
including when selecting a nested manifest, so example and host artifacts share
one compilation directory.
Both accept `--features <list>` with `--package`, forwarding the comma- or
space-separated Cargo feature list unchanged. Default features remain enabled.
Focused builds also accept `--example`. Check the packaging fixture driver with:

```sh
mise run build -- --package harmos-packaging --example fixture-driver --features guest,generate-python --check
```

Focused guest builds accept `--target wasm32-unknown-unknown` and `--messages <path>`
to capture Cargo artifacts. Build each guest through that selection, then use
`prepare:tests:fixtures --focused-messages <native-library-capture> --libraries harmos
--inspection-messages <guest-captures> --inspections <guest-names>
--componentizer target/debug/examples/fixture-driver.exe` to componentize and stamp
those exact artifacts. On Unix, omit `.exe`. Set `HARMOS_FOCUSED_FIXTURES` to the
printed manifest when running the focused guest integration and system targets.
Build the componentizer with the fixture-driver selection above, without `--check`.

Use `test --language python --exact tests.integration.<module>.<case>` for a
focused tooling test.

Focused compilation, Product, Tests and fixture builds write Cargo's native timing
reports under `target/cargo-timings/`. The timestamped reports retain unit durations,
features, dependency stalls and available frontend/code-generation sections.
Wrapper waiting is included in a unit's elapsed time; a long frontend bar alone does
not establish that rustc used that time on the CPU. Correlate it with process CPU
sampling and native cache statistics:

```sh
mise exec -- sccache --show-stats
mise exec -- sccache --show-stats --stats-format json
```

Set `BUILD_PHASE_FILE` to a JSONL output path to record the Tests producer's
compilation/archive, fixture-driver, guest-fixture and offline-input stages separately.
Use focused commands during edits; `build --artifact tests` prepares every test
binary, the guest fixture and vendored/offline authoring inputs for archive consumers.
Its scope is intentionally broader than one test target.
The Tests producer uses Cargo's default test selection, which also compiles normal
examples. It takes the guest fixture driver from those compiler messages instead
of starting a second Cargo build with a different feature graph. Its canonical
feature selection is unchanged; Product still builds its own fully capable driver.

The example and sandbox workspaces use the root workspace's development debug
settings: line tables for workspace members and no dependency debug information.
Their existing optimized guest metadata dependencies retain their settings and
fuel budgets. Release and bench optimization settings are unchanged.

Use two concurrent builders for latency or four for throughput, with eight Cargo jobs
per builder (`CARGO_BUILD_JOBS=8`). Give each independent checkout its own build
directory and share the machine-wide compiler cache. Commands in one checkout
share Cargo's target lock. Do not share one writable cache between cache servers.
Windows measurements and remaining unmeasured cases are recorded in
[Tooling Streamline](../openspec/changes/tooling-streamline/tasks.md#windows-build-measurements).

## Protocol Generation

The checked-in Rust messages and descriptor set come from `proto/harmos/v1`:

```sh
mise run build -- --package harmos-protocol --example generate
mise run generate -- --sdk rust
mise run test -- --package harmos-protocol --target integration --exact generated_code_matches_proto::generated_code_matches_proto
mise run generate -- --sdk python
mise run generate -- --sdk csharp
mise run generate -- --sdk cpp
```

`harmos-protocol` supplies reflection for generated host messages. Lifecycle
handlers register their generated request and response contracts directly.

## Python Plugin SDK

`sdk/python` builds the pure Python `harmos-plugin` distribution for Python 3.13+.
Its runtime dependencies are `grpcio` and `protobuf`; it contains no native code.
`prepare` installs the local package into `sdk/python/.venv/`; SDK tests and type
checks use that environment. Generate application messages with protobuf; author handlers using
those message classes, without an intermediate JSON representation:

```python
from google.protobuf.duration_pb2 import Duration
from harmos import Context, Operation, Plugin, observer, on_stop, operation
from thermal.v1.messages_pb2 import Reading, SetTemperature

READ_SENSOR = Operation("sensor.read", SetTemperature, Reading)

class Chamber:
    def __init__(self):
        self.target = 0.0

    @operation("thermal.set", request=SetTemperature, response=Reading)
    async def set_temperature(self, ctx: Context, request: SetTemperature) -> Reading:
        self.target = request.target
        return await ctx.operation(READ_SENSOR, request)

    @observer("thermal.temperature", row=Reading, interval=Duration(seconds=1))
    async def temperature(self, ctx: Context) -> Reading:
        return Reading(celsius=self.target)

    @on_stop
    async def cleanup(self) -> None:
        self.target = 0.0

plugin = Plugin(Chamber(), id="thermal", version="1.0.0", requires=[READ_SENSOR])
# In the application's async entry point:
await plugin.serve("127.0.0.1:53124")
```

Python plugins are remote endpoints. The author supplies the bind address;
the host never starts their process. Serving uses plain gRPC without TLS or
credentials. Stop awaits cleanup hooks in declaration order, replies, and keeps
the endpoint serving. `start(address)` returns an endpoint with its bound
`address` and an async `close()` for applications that own the server lifecycle.

`ctx.operation(contract, request, policy=...)` sends a typed unary child call on
the parent invocation. Declare every child contract in `requires`. Raise
`harmos.Failure(harmos.v1.plugin_pb2.Failure(...))` for a structured application
failure; failures received from children retain every field. `ctx.cancelled()`
waits for cooperative cancellation. `ctx.set(value)` and `ctx.get(Type)` retain
invocation-local resources; ordinary `self` fields retain the author's instance state.

Declare `harmos.SLEEP` and `harmos.NOW` in `requires` to use
`await ctx.sleep(Duration(...))` and `await ctx.now()`. These are host child calls;
the SDK has no local timer. All SDKs fingerprint clock contracts with the shared
[canonical projection](../proto/harmos/v1/behaviour.md#schema-fingerprints),
which discards file options from imported well-known types. The runtime integration
story `plugin_grpc::python_clock` starts the prepared `sdk/python/.venv/` interpreter,
admits the full conformance catalog and invokes echo on the real host. It then
closes that activation and uses `--clock-only` to drive the same clock handler
through a virtual hour without stream demand: the full fixture's external device
stream correctly prevents virtual-clock advancement. The harness owns process
shutdown through `--shutdown-on-stdin`, after the host has sent Stop.

Background work started independently by the author
is invisible to virtual time. Observers answer host ticks and cannot call
operations or wait on the host clock. An observer may declare `interval_setting`
instead of a default `interval`; the host resolves that setting. Product settings
and contributions travel as typed protobuf values in `Plugin(..., sections=[...])`.

For device-driven streams, decorate an async method with
`@publisher("thermal.device", row=Reading)` and accept `(self, ctx, publisher)`.
Call `await publisher.publish(Reading(...), at=optional_device_timestamp)` when
the device supplies a row. Demand closure cancels the producer or in-progress
observer and awaits its cleanup. Samples without device time are stamped by the
host; interval samples carry the exact tick time. Each call has its own sequence.

```sh
mise run generate -- --sdk python
mise run build -- --package harmos-plugin
mise run check -- --language python --package harmos-plugin
mise run build -- --package harmos-conformance
mise run test -- --language python --package harmos-plugin --target integration --exact tests.integration.test_remote_plugin_passes_every_rule.RemotePlugin.test_remote_plugin_passes_every_rule
mise run test -- --language python --package harmos-plugin --target integration --exact tests.integration.test_host_clock_and_state.HostClockAndState.test_host_clock_and_state
```

The wheel is written to `target/build/<profile>/any/python/`. Checked-in
`harmos.v1` message, type-stub and service modules are generated by the pinned
`grpcio-tools` compiler from `proto/harmos/v1/plugin.proto`; the conformance Value
is generated from `sdk/python/proto/`. The SDK check regenerates into disposable
scratch and compares every generated file byte for byte. Python language checks
and tests include the SDK alongside repository tooling. The reference-host story
consumes an existing host executable from Product's `bin/`, a focused local
`target/debug/` build, or `HARMOS_CONFORMANCE_HOST`; it never builds one in a test.
The standalone fixture can be served with
`python -m harmos.conformance --address 127.0.0.1:53124` and checked with
`harmos-conformance --address 127.0.0.1:53124`. Every current rule applies to remote
plugins, including the remote-specific Stop check; none is skipped.

## C++ Plugin SDK

`sdk/cpp` is a C++20 plugin library over `harmos.v1`; it no longer links the Rust
C ABI. Include `harmos/harmos.hpp` and link the CMake target `Harmos::Cpp` after
adding `sdk/cpp` with `add_subdirectory`. Author request, response, row and section
values as generated protobuf messages. The [conformance plugin](../sdk/cpp/examples/conformance/main.cpp)
demonstrates operation, stream and observer builder values ending in `serve()`.
`HARMOS_BUILD_TESTS` defaults on for a standalone SDK build and off when embedded
with `add_subdirectory`; project tasks explicitly enable it.

```sh
mise run build -- --language cpp
mise run check -- --language cpp
mise run build -- --package harmos-conformance
mise run test -- --language cpp
```

The build uses the existing pinned CMake 4.1.2 in `mise.toml`. Windows requires
Visual Studio 2022 C++ Build Tools and a Windows SDK; Linux requires a C++20
compiler and Make. Git and network access are needed on the first build.
FetchContent pins gRPC 1.76.0 to commit
`f5ffb68d8a2fd603dff16287e90a4ac571e1fec6`, including its immutable protobuf,
Abseil, BoringSSL, c-ares, re2 and zlib submodule revisions. No dependencies are
installed globally. The task enables Git long paths only in its child environment
for nested Windows worktrees. `protoc` and `grpc_cpp_plugin` are built from that
graph and generate the protocol and fixture C++ sources during the build.

On Windows, configure C++ from a short checkout path or a temporary `subst` drive
alias. Git long paths do not remove MSVC's source/include path limit for deeply
nested gRPC files. Keep the same path for subsequent builds and tests because
CMake caches absolute paths; an existing cache must be reconfigured when it changes.

Outputs and dependency sources stay under `target/cpp/<profile>/`, with retained
`configure.log` and `build.log`. `--profile release` selects Release; the default
is Debug. Check configures and validates the CMake dependency/target graph without
compilation. Test consumes the existing C++ executable and reference-host build;
it never compiles. `HARMOS_CONFORMANCE_HOST` may select a separately built host.
Native catalog tests run through CTest before the single managed-plugin story,
which checks every reference-host case, including
cleanup and successful process exit after Stop.

The fixture is `target/cpp/dev/Debug/harmos-cpp-conformance.exe` on Windows and
`target/cpp/dev/harmos-cpp-conformance` on Linux. Without arguments it binds an
ephemeral loopback port and announces `harmos listening 127.0.0.1:PORT`.
`HARMOS_ADDRESS` overrides its bind address. `--address HOST:PORT` selects remote
lifecycle: Stop completes cleanup and leaves the server running. Both modes use
plain gRPC. Host clock waits require declaring `harmos::SLEEP`/`harmos::NOW` with
`requires_operation`; observers answer ticks and cannot wait. Authors must not
start untracked background work if they require virtual-time execution.

## Focused Build Preparation

Before Cargo, mise selects the pinned environment, uv reuses `.venv/`, and a single
Python process validates selectors and sets the warnings policy. Native sccache
keeps its server across invocations. Lower Cargo parallelism if allocations fail.

`build --artifact tests --profile dev|release` produces the shared archive and test
fixtures. The Tests producer and Rust lint selection explicitly enable guest hosting
and Python generation so guest System stories and binding consumers stay in the archive
after package relocation; they do not depend on incidental feature unification.
Test-only gRPC executables belong to the unpublished conformance package;
the Tests producer stages them under `fixtures/bin` and stamps their hashes.
Archived tests resolve these paths in the consumer checkout and restore executable
permissions after transfer. `build --artifact bench` produces benchmarks and their
matching fixtures.
`build --artifact fixtures --profile dev|release` joins already-built Product and Tests inputs.
`test` without `--package` consumes existing archives and fixtures. `package` never
builds missing inputs. `generate --profile dev|release` regenerates Python clients
from the existing Product schemas. Product build also retains its current client
producer while those clients remain in the tree.

The optional selections `build --compatibility`,
`build --artifact tests --extended`, `test --extended`
and `bench --extended` retain existing facilities. `test --extended` owns its
build prerequisites; `bench --extended` consumes `build --artifact bench --extended`.

## Focused Integration Tests

### C# Plugin SDK

The optional C# SDK uses the project-pinned .NET 10 LTS tool through mise. Install it
with `mise install dotnet`; no machine-wide SDK installation is required. The build
restores locked NuGet dependencies, generates `harmos.v1` and fixture C# from protobuf,
and builds the class library, managed fixture and test story. Generated C#, `bin/`,
`obj/` and test results stay untracked under `sdk/csharp/`.

```sh
mise run build -- --language csharp
mise run build -- --package harmos-conformance
mise run test -- --language csharp
mise run check -- --language csharp
```

The test consumes both build outputs without rebuilding. Its one managed-plugin story
runs all 20 reference-host cases in managed and remote modes, including successful
managed process exit and continued remote availability after Stop. A focused unit
test also checks clock requirement descriptors and the Rust-compatible sleep fingerprint.
`--profile release` selects matching Release C# and release Rust outputs. Formatting
uses `dotnet format --verify-no-changes`; apply it with `format --language csharp`.
Other languages' default selections remain unchanged. The C# authoring example lives
in the `grpc-plugins` design's Language SDKs section while that change is implementing.

CI adoption needs native Windows and Linux .NET build/format selections, transfer of
the C# outputs and Rust reference host to the test consumer, and the C# conformance
story in Nightly. No CI jobs are added by this SDK task.

### Rust Integration Tests

The integration task accepts a nextest filter and repeat count while retaining
the configured per-test hang guards from `[conventions.testing.deadlines]` in
`tools/project.toml`. Completed story durations remain measurements. It consumes the existing test archive and fixtures:

```sh
mise run test -- --language rust --layer integration --filter 'package(=harmos-runtime) & test(=plugin_grpc::child_calls::child_calls_reach_host_and_public_peer_without_start_order)' --stress-count 50
mise run test -- --language rust --layer system --filter 'binary_id(=harmos::system) | (binary_id(=harmos-runtime::system) & test(guest))'
```

System also accepts `--filter` and preserves its serial scheduling and hang guards.
Select `--profile release` to use an archive and fixtures from a Nightly build.
The protocol drift test runs the pinned `protoc` from the transferred offline
vendor inputs and restores its executable permission on Unix. Focused Cargo
tests use the pinned compiler in the local Cargo registry.

## Rust SDK Dependency Boundary

The generated-client and plugin SDK share one protocol-only product dependency closure.
The focused tooling test inspects Cargo metadata without compiling or resolving registry updates:

```powershell
$env:PYTHONPATH = (Get-Location).Path
mise run test -- --language python --exact sdk.rust.tests.dependency_closure
Remove-Item Env:PYTHONPATH
```

The client example consumes Schematic's generated protocol package and needs no host dependency:

```sh
mise run build -- --package harmos --example client --check
```

## Validation

Run targeted tasks locally. Hosted CI and Nightly provide full validation.

## Workflow Task Interface

Each job checks out, uses the composite setup action, restores caches, downloads
its inputs, runs one task, and uploads its results. Setup runs `prepare` to
synchronize the locked Python groups. Hidden `ci:<name>` tasks own provider
bookkeeping; `prepare` only prepares the environment.

The task step accepts `CI_INPUT_ARCHIVES` (a JSON array of archive paths) and
`CI_OUTPUT_ARCHIVES` (a JSON array of objects with `archive`, `paths` and `profile`).
The entry point extracts inputs before its work and packs outputs after success,
using the single implementation in `ci/artifacts.py`. Nested tasks never repeat
transport. `--help` performs no transport. Provider steps only download and upload
archives under `target/ci/`. The fixture producer joins Product, the Tests archive
and stamped fixtures into one bundle consumed by integration, acceptance, system,
coverage tests. The bundle includes the test archive, so these jobs
do not download it separately.

Explicit transport is also available as `ci:pack` and `ci:extract`, with
`--profile <profile> -- <arguments>`; `--archive`, hex-encoded JSON `--paths` and
`--target` retain their meanings. `ci:summary` accepts the `CACHE_RESTORES` JSON
input when an actual summary is needed. Compiler cache keys contain OS,
architecture, compilation profile and the Rust version read from `mise.toml`;
the Cargo lock hash is the final restorable component. Other tool versions do
not invalidate compiler caches.

Nightly runs every four hours in UTC and on manual dispatch, only on dev.
Runs never cancel each other. Freshness compares the nightly tag, suppresses duplicate
queued/running revisions only when an older run owns them, and fails closed on lookup
errors. Publication waits only for Product Build and Package; suites, benchmarks and
coverage may fail independently.

`validation.yml` owns the shared validation job bodies, selected by `selection`,
`profile` and `channel`; CI and Nightly call it. `publication.yml` and
`verification.yml` own the shared publish and verify jobs. Both channels use
`release:publish --channel stable|nightly` and `release:verify --channel stable|nightly`.
Stable keeps its review evidence and registry guards; Nightly creates no stable tag
and publishes no registry packages. Registry publication remains in `release.yml`.

Package exports the reference host and plugin archive from matching Product bytes,
with conservative locked Cargo target graphs and the Python SDK lock inventory as
CycloneDX JSON plus notices. Release staging builds protocol and compliance ZIPs,
`release.json` and `SHA256SUMS` from those existing packages. It replaces its owned
staging directories and rejects files or links where a directory is required.
The changelog is stored in `release.json` for the release body and is never attached separately. No legacy
plugin feed, separate download index, example archive or individual compliance file
is added to the upload inventory.

`release:rehearse --target native --profile dev --stage-only` checks existing local
packages without registry dry runs. Hosted rehearsal requires release-profile
packages for both native targets and registry verification. `release:publish --dry`
uses the same staging path; add `--channel nightly` to rehearse the Nightly inventory.
Neither command builds missing inputs.

Nightly publishes initial report downloads even when no suite has finished. The final
report job calls the shared publisher with `report: true` and `results: toJSON(needs)`.
It downloads available `results-*` artifacts (tests, traces, benchmarks and coverage),
then replaces the reports ZIP, `benchmarks.json`, `release.json` and checksums. It appends an idempotent
results table and pipeline URL only when the release's CI Run row identifies this run.
Publication and report updates share noncancelling concurrency and the draft-and-swap
publisher; a failed replacement preserves the previous public inventory. Every draft
and public asset is downloaded, sized and hashed even when the API supplies a digest.
Native verification consumes `release-manifest` and runs the downloaded conformance
host. Benchmark exports retain medians and pre-run baselines, including results
available from a failed benchmark subprocess.

### Dokime Alignment

The shared validation inputs are `channel` and `profile`; Harmos also selects a body
with `selection`. Publication and verification use `build-run`, `build-sha` and
`build-version` for stable review evidence, with archive downloads defaulting to the
current run for Nightly. The report uses the same publication workflow and concurrency
group, retains the original notes, swaps a complete verified inventory and overwrites
`release-manifest` only after publication succeeds. Its immutable downloads come from
the published inventory rather than repeating package staging.

Products keep their own languages, runner matrices, inventories and workflow file
organization. Harmos retains one validation dispatcher; Dokime splits validation by
job. The four-hour Nightly schedule is unchanged.
Dokime should default every package download to `build-run || github.run_id`, including
Windows Rust packages: its shared publish/verify workflows currently select the
publication run for those packages and can lose stable review inputs. Its alignment
notes should reflect the shared contracts above.

### Release Reviews

`ci:runners` preserves the runner allowlist check. `ci:release-mode` emits `mode`
and `sha` to `GITHUB_OUTPUT`: successful release-review CI completion selects its
merged, untagged release squash for tagging; unrelated completions skip. A dispatch
on dev retries that squash, including after later dev merges, or prepares the next
release when none is pending. Release / Tag waits for `ci:review` to validate the
green review and package evidence, then `release:tag` checks exact-commit required
CI and the release rehearsal before tagging and fast-forwarding main. CI closure cancellation excludes
merged release reviews so their validation can finish. Release evidence lookup uses
`ci:review` with `REVIEW_REQUIRED`, `REVIEW_RELEASE`, `REVIEW_PACKAGES` and
`REVIEW_BUILDS`; outputs go to `GITHUB_OUTPUT`. `ci:release-checks` checks the exact
`RELEASE_TARGET_SHA`. `ci:required` reads `RESULTS` JSON and refuses failures,
cancellations and unexpected skips; the release-review rehearsal may skip
outside a release review (`RELEASE_REVIEW=false`). `OPTIONAL_JOBS` explicitly allows
skipped suites, fixture preparation, packaging and dry publication on dev pushes.
Dev pushes run only checks and Product/Tests builds; they never run those consumers.
The dev Python job selects the existing format/lint tasks because the complete
Python check also runs SDK and tooling tests. Generated-client checks consume
Product outputs, including `target/generated/python/src`; they do not build or
invent missing clients. Local complete Python checks require the reference host
(`mise run build -- --package harmos-conformance`) and matching Product outputs
(`mise run build`). The complete Python check reports this prerequisite before checking
clients when `target/generated/python/src` is absent.

Every pull request runs the full verification graph. Packaging consumes Product
without waiting for checks. Unit tests download only Tests and wait only for its build.
Integration, acceptance and system tests download the joined fixture bundle, which
contains their test archive, and wait only for its producer. Coverage tests also consume that bundle. Benchmarks consume only Bench. The required CI gate
collects all verification results; release rehearsal runs only on release reviews.
Nightly freshness and release evidence are decision inputs; Nightly's final
report consumes suite statuses as well as the publication result.

`release:prepare --review` prepares changes, checks the runner allowlist when
configured, then creates the release review. Hosted GitHub `release:tag` and
`release:publish` own identity reporting and the exact-commit CI guard. Publication uses
`release:publish --dry` for a dry run or `release:publish --stage assets|rust|finalize`
for one real publication stage. These selections are mutually exclusive. Default publication still drafts
assets then finalizes. `release:rehearse` accepts `--target`, `--profile` and
`--stage-only`; no release command builds missing products. Workflow and task changes are delivered together.

Broad CI compilation tasks retain the configured backend verification and measured
build summary through `ci/build.py` and `ci/compiler.py`. Local invocations and
focused selections do not enter that path. Workflows call the entry point directly;
the old `ci.build` command and `BUILD_SCRIPT` adapter are retired. Current setup
cache environment inputs retain their meanings.


## Focused Storage Benchmarks

```sh
mise run bench -- --storage --filter baseline
mise run bench -- --storage --filter native
```

This selection builds only `harmos-storage`'s `integration-multi-day` benchmark in the bench
profile. It measures the current generic row-query path over the deterministic test backend:
2,000,000 samples spanning three days and 24 plots, including late arrivals. Workload generation
is outside timing; projected row count and value sum are checked against the generated inputs.
The baseline selection records a single timed traversal for full, first-window and next-window
reads. These measurements include snapshot and paging costs and are not Criterion medians.
Raw output and compiler metadata remain in `target/benchmarks/storage/`. Without the baseline
filter, the same executable additionally uses ordinary Criterion sampling, whose native files
remain under `target/benchmarks/storage/criterion/`. The storage-interface design owns workload
identity and proposed acceptance budgets; this command does not establish a performance claim.

The `native` selection constructs the same deterministic source in the optional durable SQLite
backend and measures native full/window scans, exact-frontier projection windows/replay and
index construction/seek reads. It does not construct the row-query baseline fixture. Ingest,
catch-up and index construction report individual elapsed measurements; read cases retain ten
Criterion samples. Verification runs outside timing. The owned temporary database is removed
after close; `candidate.txt`, compiler metadata and Criterion samples remain for comparison.
The existing baseline capture is not a durable-backend baseline; preserve its compiler metadata
as `compiler-baseline.json` before a later run replaces `compiler.json`.

## Storage Consumer Proof

```sh
mise run test -- --package harmos-runtime --target integration --story storage::generated_native_journey
mise run test -- --package schematic-app --manifest-path examples/schematic/Cargo.toml --target storage --exact saved_preview_survives_atomic_replacement
mise run generate -- --sdk rust
mise run generate -- --sdk python
mise run generate -- --sdk csharp
mise run generate -- --sdk cpp
```

Storage protobuf generation refreshes messages, service stubs and reflection together. Rust
contracts are exposed as `harmos::storage`; the other SDKs generate storage clients from the
same `proto/harmos/v1/storage.proto`. See [Generated Storage Clients](storage.md#generated-storage-clients).
