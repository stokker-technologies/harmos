# Repository Tooling

Mise is the public entry point. Run `mise tasks` to see the supported entry points;
pass arguments after `--`. Python tooling lives in `tools/python/src/harmos_tools/`;
the root uv workspace owns `.venv/` and `uv.lock`. The standalone SDK owns
`sdk/python/.venv/` and `sdk/python/uv.lock`; tooling does not depend on the SDK.
Run `mise run prepare` once after checkout or dependency changes to synchronize
all locked dependency groups in both projects.
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
and `SCCACHE_CACHE_SIZE=20G`. Cargo starts or reuses sccache's background server;
tasks do not start, verify or stop a server for each local invocation. Incremental
compilation uses Cargo's default locally; only workflows disable it. Each worktree
keeps its own `target/`; Cargo owns its normal compilation lock. Project tooling
never scans or reclaims compiler output before compilation.

```sh
mise run build -- --package harmos-domain
mise run build -- --package harmos-domain --check
mise run build -- --package harmos-runtime --features grpc,simulation --check
mise run test -- --package harmos-runtime --features simulation --target lib --exact simulation::clock::tests::evidence_dates_retain_the_host_epoch_across_reboot
mise run test -- --package harmos-macros --target lib --exact operation::tests::portable_domain_operation
```

Focused builds select one package; focused tests select exactly one library or
named integration target and use Cargo's default test profile, without an archive.
Both accept `--manifest-path examples/schematic/Cargo.toml` to select the separate
example workspace while keeping compilation behind the same task interface.
Both accept `--features <list>` with `--package`, forwarding the comma- or
space-separated Cargo feature list unchanged. Default features remain enabled.
Focused builds also accept `--example`. Check the packaging fixture driver with:

```sh
mise run build -- --package harmos-packaging --example fixture-driver --features guest,generate-python --check
```

Use `test --language python --exact tests.integration.<module>.<case>` for a
focused tooling test.

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
the SDK has no local timer. Background work started independently by the author
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
fixtures. `build --artifact bench` produces benchmarks and their matching fixtures.
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
runs all 16 reference-host cases in managed and remote modes, including successful
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
mise run test -- --language rust --layer integration --filter 'package(=harmos-runtime) & test(=sidecar::observer_demand::demand_survives_reload)' --stress-count 50
```

Select `--profile release` to use an archive and fixtures from a Nightly build.
The protocol drift test runs the pinned `protoc` from the transferred offline
vendor inputs and restores its executable permission on Unix. Focused Cargo
tests use the pinned compiler in the local Cargo registry.

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

Nightly uses `ci:nightly-gate` (`GITHUB_OUTPUT` receives `build`),
`release:publish --stage nightly`, then `ci:nightly-report` with `SUITE`
(`passed` or `failed`) and optional `RUN_URL`. Reports preserve a newer run's notes.
Mirror uses `ci:mirror` with its SSH key, known hosts and GitHub ref/SHA inputs.
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
cancellations and unexpected skips; only the release-review rehearsal may skip
outside a release review (`RELEASE_REVIEW=false`).

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
`release:publish --dry` for a dry run or `release:publish --stage assets|rust|finalize|nightly`
for one real publication stage. These selections are mutually exclusive. Default publication still drafts
assets then finalizes. `release:rehearse` accepts `--target`, `--profile` and
`--stage-only`; no release command builds missing products. Workflow and task changes are delivered together.

Broad CI compilation tasks retain the configured backend verification and measured
build summary through `ci/build.py` and `ci/compiler.py`. Local invocations and
focused selections do not enter that path. Workflows call the entry point directly;
the old `ci.build` command and `BUILD_SCRIPT` adapter are retired. Current setup
cache environment inputs retain their meanings.
