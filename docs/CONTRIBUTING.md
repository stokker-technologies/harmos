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

Windows and Linux are supported. The [Schematic example](schematic.md)
is a complete application for understanding composition, plugins and storage.

## Development Workflow

Preserve clear boundaries between domain contracts, runtime resources and
application-owned persistence. Update affected consumers and examples when
changing a contract; flag unrelated stale examples rather than hiding them.

Use Conventional Commit subjects and keep generated files with their source.
The root uv workspace owns the Python environment and lockfile. Rust compile
and coverage tasks use the machine-wide cache-directory lock, including focused
checks, and share a capped local sccache across independent worktree targets.

Harmos owns its workflows, composite actions and CI/release tooling. Pins, CI, release layout, binding generation and documentation are project-owned.
PRs run selected suites. Dev pushes run only the quick checks and Product and
Tests compilation; they package and publish nothing. Merges to dev remain
ungated.

Git dependencies are allowed on dev and ordinary PRs. Release PRs (`release/v*`
into dev), tagging and publication reject resolved git dependencies, including
transitive ones. Nightly warns and skips incompatible crate dry publication
while retaining its checks, builds, packages, suites, coverage and benchmarks.

Nightly is the provisional release channel: one mutable GitHub prerelease
titled `Nightly` under the `nightly` tag, built from dev in the release profile.
It runs every four hours in UTC and on manual dispatch, without cancelling
earlier runs; a scheduled run skips when the `nightly` tag already points at
dev HEAD. A manual run bypasses the tag comparison. Both skip duplicate queued
or running revisions; tag lookup errors other than a missing tag fail closed.
Publication depends only on the build and package jobs and consumes that run's
native tool archives, Schematic ZIPs and plugin bundles from both platforms;
plugin download names include their producer target. Tests, benchmarks and
coverage run in parallel and may fail, so a nightly can be broken: the final
`Nightly / Report` job replaces this run's suite result and run URL in the release
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
`Mirror / Push`; `CI / Required` is the aggregate gate. Release starts for
manual dispatch, completed `CI` runs and `v*` tag pushes. Successful validation
of a merged `release/v*` review starts tagging its prepared, untagged release;
unrelated CI completions skip release jobs. A dispatch on dev retries tagging
that prepared release, even after later dev merges, or prepares the next release
otherwise.

## Tests and Checks

Run targeted tasks locally. Hosted CI and Nightly provide full validation.
Every pull request runs all configured verification jobs; release rehearsal is
limited to release reviews. Impact planning is removed. Each matrix job names one
project task; see [Workflow Task Interface](tooling.md#workflow-task-interface).
Merges into dev remain ungated. Required hosted checks gate main and stable
releases. Report and investigate local failures.

Product and Tests compile independently. Build, test, bench and fixture tasks never
run Clippy. `lint --language rust` runs Clippy with warnings denied; `check --language rust` includes it
and CI Check / Rust gates it. Use `mise run lint -- --language rust --extended` for local compatibility lint coverage.
Tests use the default unoptimized test profile; optional extended tests remain
explicit local work. Nightly runs ordinary verification with release Product
builds and downloadable unsigned packages, plus coverage and benchmarks.
Lane deadlines are hang guards read from `[conventions.testing.deadlines]` in
`tools/project.toml`. Native runners terminate hung stories; script lanes also
terminate their process tree at the lane deadline. Completed story durations are
measurements and never change the runner's pass/fail result. Use benchmarks for
performance measurements.

For a focused Rust compiler check or selected test, use the package and target arguments:

```sh
mise run build -- --package harmos-domain --check
mise run test -- --package harmos-sdk-derive --target lib --exact stored_message::tests::map_field_shapes
```

`prepare` synchronizes the locked Python environment.
`mise run build -- --artifact fixtures --profile dev` joins existing Product and
Tests inputs. Unit tests consume only the Tests archive; integration, acceptance,
system and coverage tests consume the joined fixture bundle. Benchmarks
consume only the Bench build. Other entry points use
`uv run --no-sync`. Focused selections invoke Cargo directly, without archives,
compiler-output scans or locks. The compiler cache's background server persists
across invocations; each worktree keeps its own `target/`. See
[Workstation Builds](tooling.md#workstation-builds).

`mise run test -- --language python` tests task selection, release evidence and atomic
channel publication without compiling products or contacting publication APIs.
It also runs as part of `check --language python`.

Storage-engine stories exercise `harmos-storage` without a host runtime; its
real `WorkerOwner` owns the canonical worker and settlement. Runtime storage
stories cover host composition, managed journal projection and operation dispatch.
Use the public memory adapter and its deterministic control gates for manager
fault stories. Disk parsing, rollback and root leasing belong to product backend
stories. The specialized integration entrypoints are
`crates/harmos-domain/tests/message/main.rs` and `sdk/rust/tests/wasm/main.rs`.

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
retains producer identity and byte hashes. Acceptance selects the libraries and both plugin Product artifacts; system selects the Schematic CLI
and both plugins.

Compiler execution for macro diagnostics belongs to acceptance; integration
consumes already-built artifacts. Build exports a Cargo vendor directory and a
hash-checked Python wheelhouse for the root tooling. Python binding interoperability is
Rust-driven acceptance using the root `.venv`; generated Python packages remain
consumer-owned.

## Pull Requests

Target `dev` and explain the user-visible problem, resulting behavior and
verification. Keep required checks enabled and include no private data or
credentials. Contributions use the [Apache-2.0 license](../LICENSE).

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

Release / Prepare opens the `release/v<version>` review and squashes it into
dev immediately. Its `CI / Validation` continues; successful completion starts
Release / Tagging automatically, targeting the release squash even after later
dev merges. A dispatch on dev retries a prepared, untagged release. The tag push
starts publication and verification. Only the initial dispatch and `release`
environment approvals require manual action. Before tagging or publishing,
`release.yml` checks the exact target's required CI; the green release-review
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
