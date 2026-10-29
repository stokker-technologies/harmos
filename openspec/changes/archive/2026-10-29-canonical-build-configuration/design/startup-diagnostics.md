## Schematic Initialization Diagnostics

The unchanged candidate's Windows System replay passed nine stories and failed
`schematic::cli_streams_settle` at 23.177 seconds, before its thirty-second
watchdog. The CLI rendered only `Display`, hiding the source held by
`Error::OperationFailed`; both carriers also share the `schematic-tools` owner.
This evidence cannot identify the underlying failure or even its carrier.
The example CLI must render the existing canonical `harmos::failure::Report`,
and each system assertion must retain its launched carrier label. These
changes preserve process count, assertions, operation budgets and watchdogs.
A focused actual CLI diagnostic uses the retained Product plugin inputs;
passing a narrow story never replaces the failed full-lane verdict.

The isolated nine-failure diagnostic retains the exact Product plugin and CLI
hashes and the passed Linux Tests archive from `repair-5103accc693c`. With no
other compiler workload, the canonical runner selected all seven guest stories
and both Schematic stories concurrently, excluding only the already-green
runtime baseline. It saw 32 CPUs, 37,732,948 KiB available memory, and unbounded
cgroup CPU/memory values. The guest stories passed in 25.543–28.601 seconds;
both Schematic stories still exceeded 30 seconds. External compilation is
therefore not the sole source of the Linux failure. This diagnostic remains a
failed verdict and does not replace final full System validation.

A single quiet Windows diagnostic used the new error-rendering CLI with the
unchanged retained Product plugins and ran both advanced carriers concurrently.
Sidecar completed in 5.115 seconds and guest in 5.060 seconds with matching
shared output and every stream/cancellation/reopen assertion intact. The
previous Windows initialization cause was not reproduced and cannot be
recovered from the old truncated `Display` chain; it remains unknown. The
canonical source-chain rendering is required for the next occurrence.

### Approved System Scheduling

The diagnostic-only write recorder observed the unchanged Linux Product CLI
under the same quiet nine-story load. The advanced native carrier reached its
first edit milestone after 29.883 seconds (6.154 user CPU seconds, 0.310 system
CPU seconds, no major page faults). Validation, routing, streaming,
cancellation and reopen then completed in approximately 31 milliseconds. The
other three CLI children emitted no edit milestone before the watchdog.
All seven standalone guest stories passed in 26.227–27.841 seconds; both
Schematic stories timed out at 30.003 seconds. This establishes shared cold
startup pressure before actual operation work, including the native carrier.
It does not establish the precise source of the previous Windows initializer
error, whose old error display discarded its source.

The user explicitly approved running these System cases serially. The existing
authored `test:rust:system` task forwards native Nextest `--test-threads 1`
through the shared runner's existing argument passthrough. All ten Rust System
stories, including the negligible baseline, use serial scheduling. This avoids
adding a parallel project configuration or a shared test-group framework.
Every original cold process launch, assertion and thirty-second watchdog is
preserved. Other test lanes, build workers, application/runtime threads and
memory remain unrestricted. No cache, compiler profile, production deadline or
application architecture changes are introduced by this scheduling decision.

### Shared Tooling Adoption

The same repair candidate adopts installed Hub `0.1.8+dev.34afa72941` using
`stkr project update --no-provider`. Root tooling and the declared
`crates/cargo-harmos/templates/{guest,sidecar}` scaffold copies adopt generated
build-output storage preflight and Docker cleanup, together with their shared
portable validation and impact owners. The root prose impact rule renames
`only` to required `include` with identical patterns, exclusions and selected
repository check. The authored System serial argument and four-story nightly
partition are preserved. Provider credentials remain disabled for the render.

The locked source inventory for this repair is:

| Owner | Exact Files |
|---|---|
| Authored | `mise.toml`, `stkr.toml`, `examples/schematic/cli/src/main.rs`, `crates/harmos-runtime/tests/system/schematic.rs`, this change's `tasks.md` and `design.md` |
| Root Generated | `.github/workflows/ci.yml`; under `tools/python/src/harmos_tools/`: `build/cargo.py`, `build/storage.py`, `ci/containers.py`, `ci/impact.py`, `ci/mise.py`, `ci/simulate.py` |
| Guest Scaffold Generated | Under `crates/cargo-harmos/templates/guest/`: `mise.toml.tmpl`; under `tools/python/src/project_tools/`: `build/cargo.py.tmpl`, `build/storage.py.tmpl`, `ci/containers.py.tmpl`, `ci/impact.py.tmpl`, `ci/mise.py.tmpl`, `ci/simulate.py.tmpl` |
| Sidecar Scaffold Generated | The same seven relative files under `crates/cargo-harmos/templates/sidecar/` |

The root authored repository-check prefix includes the portable Mise validator
immediately after the existing TOML format check, retaining ShellCheck and
actionlint afterward. Both scaffold check prefixes come from the shared
renderer. README and CONTRIBUTING have no affected scheduling or transfer
claim to update.
