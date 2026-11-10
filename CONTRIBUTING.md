# Contributing

Help improve Harmos with focused fixes, clear API feedback and useful examples.
Discuss changes to public contracts before implementing them.

## Development Setup

Install [mise](https://mise.jdx.dev), then prepare the checkout:

```sh
mise trust
mise install --locked
mise run prepare
```

Windows and Linux are supported. The [Schematic example](docs/schematic.md)
is a complete application for understanding composition, plugins and storage.

## Development Workflow

Preserve clear boundaries between domain contracts, runtime resources and
application-owned persistence. Update affected consumers and examples when
changing a contract; flag unrelated stale examples rather than hiding them.

Use Conventional Commit subjects and keep generated files with their source.
The root uv workspace owns the Python environment and lockfile. Rust compile
and coverage tasks use the machine-wide cache-directory lock, including focused
checks, and share a capped local sccache across independent worktree targets.

Harmos owns its workflows, composite actions and CI/release tooling. The Hub
renders editable baselines; only explicit `stkr project ci render` overwrites CI.
PRs run selected suites. Dev pushes run only the quick checks and Product and
Tests compilation; they package and publish nothing. Merges to dev remain
ungated.

Git dependencies are allowed on dev and ordinary PRs. Release PRs (`release/v*`
into dev), tagging and publication reject resolved git dependencies, including
transitive ones. Nightly warns and skips incompatible crate dry publication
while retaining its checks, builds, packages, suites, coverage and benchmarks.

Nightly is the provisional release channel: one mutable GitHub prerelease
titled `Nightly` under the `nightly` tag, built from dev in the release profile.
It runs on its schedule and on manual dispatch; a scheduled run skips when the
`nightly` tag already points at dev HEAD, while a manual run always builds.
Publication depends only on the build and package jobs and consumes that run's
native tool archives, Schematic ZIPs and plugin bundles from both platforms;
plugin download names include their producer target. Tests, benchmarks and
coverage run in parallel and may fail, so a nightly can be broken: the final
`Nightly / Report` job appends the suite result and run URL to the release
notes. Benchmark results stay CI artifacts, never release assets.

One serialized publisher drafts the release, uploads and verifies every asset
including SHA256SUMS, moves the tag once, then publishes and verifies the
release and every asset at `releases/tags/nightly`. Downloads are unavailable
while drafted; interrupted uploads remain drafted for retry instead of exposing
incomplete downloads. Stable releases retain their separate download inventory.
A mutable prerelease must stay replaceable: with GitHub release immutability
enabled, the publisher fails and names the owner setting (Settings > General >
Releases > Enable release immutability) instead of publishing a frozen
release.

Every project uses the workflows `CI`, `Nightly`, `Release` and `Mirror`. Run names read `<Workflow> / <Purpose>`: `CI / Validation`,
`CI / Draft Review`, `CI / Closed Review`, `Nightly / Validation`,
`Release / Preparation`, `Release / Tagging`,
`Release / Publication` and `Mirror / Synchronization`. Job names read
`<Stage> / <Subject>[ / <Kind>] [(<platform>)]`, such as `Check / Python`,
`Test / Rust / Unit (windows)`, `Release / Tag`, `Nightly / Publish` or
`Mirror / Push`; `CI / Required` is the aggregate gate. Release starts only for
manual preparation, `v*` tag pushes and merged PRs into dev that change
`tools/project.toml`, which every release review does; its jobs still require
a `release/v*` head, so ordinary merges start no Release run.

## Tests and Checks

Run targeted tasks locally. Hosted CI and Nightly provide full validation.
Every pull request runs all configured verification jobs; release rehearsal is
limited to release reviews. Impact planning is removed. Each matrix job names one
project task; see [Workflow Task Interface](docs/tooling.md#workflow-task-interface).
Merges into dev remain ungated. Required hosted checks gate main and stable
releases. Report and investigate local failures.

Product and Tests compile independently. Build, test, bench and fixture tasks never
run Clippy. `lint --language rust` runs Clippy with warnings denied; `check --language rust` includes it
and CI Check / Rust gates it. Use `mise run lint -- --language rust --extended` for local compatibility lint coverage.
Tests use the default unoptimized test profile; optional extended tests remain
explicit local work. Nightly runs ordinary verification with release Product
builds and downloadable unsigned packages, plus coverage, benchmarks and
`test --starter` for generated guest/sidecar compilation and development lifecycles.
Ordinary acceptance retains the other product stories without compiling starters.
Lane deadlines are hang guards read from `[conventions.testing.deadlines]` in
`tools/project.toml`. Native runners terminate hung stories; script lanes also
terminate their process tree at the lane deadline. Completed story durations are
measurements and never change the runner's pass/fail result. Use benchmarks for
performance measurements.

For a focused Rust compiler check or selected test, use the package and target arguments:

```sh
mise run build -- --package harmos-domain --check
mise run test -- --package harmos-macros --target lib --exact operation::tests::portable_domain_operation
```

`prepare` synchronizes the locked Python environment.
`mise run build -- --artifact fixtures --profile dev` joins existing Product and
Tests inputs. Unit tests consume only the Tests archive; integration, acceptance,
system, coverage and starter tests consume the joined fixture bundle. Benchmarks
consume only the Bench build. Other entry points use
`uv run --no-sync`. Focused selections invoke Cargo directly, without archives,
compiler-output scans or locks. The compiler cache's background server persists
across invocations; each worktree keeps its own `target/`. See
[Workstation Builds](docs/tooling.md#workstation-builds).

To reproduce one generated compiler story from the existing test archive:

```sh
mise run test -- --starter --profile release --filter 'test(=authoring::sidecar_starter_works)'
```

`mise run test -- --language python` tests task selection, release evidence and atomic
channel publication without compiling products or contacting publication APIs.
It also runs as part of `check --language python`.

Storage-engine stories exercise `harmos-storage` without a host runtime; its
real `WorkerOwner` owns the canonical worker and settlement. Runtime storage
stories cover host composition, managed journal projection and operation dispatch.
Inject file journal faults at the public `FileBackend` boundary, retaining its
parsing, rollback and leasing. The specialized integration entrypoints are
`harmos-domain/tests/message/main.rs` and `harmos-guest/tests/kit/main.rs`, both
under `crates/`.

Runtime system tests compose the host baseline with the Schematic guest and
sidecar artifacts from the example workspace. The sandbox guest covers guest
policy and cleanup absent from Schematic. Wasmtime component instantiation
belongs to system tests; inert component metadata and WIT inspection belong to
integration. Sidecar stories distinguish environment clearing, pre-readiness
failure, cleanup, withdrawal and fresh-generation recovery; pre-readiness
failure does not prove a crash after readiness.

Tests-owned fixtures contain hash-verified inert Product plugin artifacts;
sandbox inspection reuses Tests-owned sandbox bytes. Integration never executes
these components. The Tests stamp selects artifacts, libraries and offline inputs
with hashes; runtime fixture support resolves them, and collected Cargo metadata
retains producer identity and byte hashes. Acceptance selects the Cargo
subcommand and both plugin Product artifacts; system selects the Schematic CLI
and both plugins.

Compiler execution for generated projects, macro diagnostics and development
rebuild/cancellation belongs to acceptance; integration consumes already-built
artifacts. Generated-project stories use starter manifests and locks, the
build-owned Cargo vendor directory and a hash-checked Python wheelhouse covering
root tooling and both starter locks. Python binding interoperability is
Rust-driven acceptance using the root `.venv`; generated Python packages remain
consumer-owned.

## Pull Requests

Target `dev` and explain the user-visible problem, resulting behavior and
verification. Keep required checks enabled and include no private data or
credentials. Contributions use the [Apache-2.0 license](LICENSE).

Release preparation and publication follow the separate protected release
workflow; an ordinary pull request does not publish packages.

`mise run release:publish -- --dry` stages the available native packages and dry-runs
crate publication. `mise run release:rehearse` uses existing release-profile
packages for every declared release target, strips publication credentials and
checks the same archive and crate inventories without creating a provider
release. Neither command builds missing Product packages. Generated changelogs
are included temporarily in crates and restored afterwards.

Before the first release with this workflow layout, the owner must register
crates.io Trusted Publishing against `release.yml`, environment `release`, for
`stokker-technologies/harmos`, replacing the previous workflow identity.

Release reviews squash into dev. Before tagging or publishing, `release.yml`
queries GitHub for the exact target SHA's latest dev push run, its successful
`CI / Required` check and any additional required contexts in main's ruleset.
Missing or pending proof waits up to 30 minutes; failed proof refuses release.
Rerun Release after repairing CI if this gate times out. The green release-review
rehearsal and artifact checks remain required before tagging and fast-forwarding main.

The repository owner must configure an active branch ruleset targeting `main`
with **Require Status Checks To Pass** and `CI / Required` from GitHub Actions.
Retain deletion and force-push protections and the release identity's narrow
fast-forward permission without a blanket bypass of required checks. Do not add
this gate to dev. Workflow changes do not configure repository settings.

## Reporting Issues

Use the [issue tracker](https://github.com/stokker-technologies/harmos/issues).
Include the revision, operating system, minimal reproduction, expected result,
actual behavior and relevant sanitized logs.
