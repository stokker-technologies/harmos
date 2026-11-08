# Repository Tooling

`lint:rust` owns Clippy with warnings denied. `check:rust` includes it, and CI
Check / Rust gates it. Build, test, bench, fixture and focused compilation tasks
retain compiler preflight without running Clippy. Use an explicit
`mise run build:rust:focused -- clippy <arguments>` for a selected Clippy run.

Benchmark targets and their matching fixtures are covered by `lint:rust`.

`mise run lint:rust --extended` lints the optional compatibility selections;
`build:rust:compatibility` checks them without Clippy. Extended lint remains local-only.

Mise is the public entry point. The project-owned Python task wrappers live in
`tools/python/src/harmos_tools/`; the root uv workspace owns `.venv/` and
`uv.lock`. Run `mise run prepare:tools` to synchronize the locked tools.

## Workstation Builds

Compiling tasks run through `tools/python/src/harmos_tools/build/machine.py`, which starts the
shared compiler cache for the command. Nothing serializes compilation: tasks in
any repository or worktree run at the same time, and no worker or compiler
thread limits are added.

For non-CI runs, `mise.toml` selects `RUSTC_WRAPPER=sccache`,
`SCCACHE_DIR=<OS user cache>/sccache`, `SCCACHE_CACHE_SIZE=20G` and disables
incremental compilation. Each worktree keeps its own `target/`; matching
cacheable crate compilations can reuse results across worktrees. CI retains
its existing per-profile cache settings. sccache is already a pinned mise tool.
Local wrappers reuse the existing `ci/compiler.py` foreground-server helper with
an owned endpoint, so an older daemon cannot silently select another store.
Concurrent tasks each own a server on the same store. Only that owned server is
stopped after the command; each task prints its cache hit and miss counts.
If allocation failures recur, disable it for one run by setting
`RUSTC_WRAPPER` to an empty value:

```sh
RUSTC_WRAPPER= mise run build
```

In PowerShell use `$env:RUSTC_WRAPPER = ''; mise run build`, then remove
the override with `Remove-Item Env:RUSTC_WRAPPER`.

## Focused Integration Tests

The integration task accepts a nextest filter and repeat count while retaining
the configured per-test hang guards from `[conventions.testing.deadlines]` in
`tools/project.toml`. Completed story durations remain measurements. It consumes the existing test archive and fixtures:

```sh
mise run test:rust:integration --filter 'package(=harmos-runtime) & test(=sidecar::observer_demand::demand_survives_reload)' --stress-count 50
```

Select `--profile release` to use an archive and fixtures from a Nightly build.

## Simulation Environment

`mise run ci:simulate` isolates mise's global configuration and lockfile from
the workstation. Each job uses an empty global configuration location and trusts
only its staged workspace through `MISE_TRUSTED_CONFIG_PATHS`, including Linux
containers before bootstrap. Machine `MISE_*` overrides are not inherited by
jobs; repository provenance verification remains enabled. `CARGO_BUILD_JOBS`
is forwarded to both native and container jobs when supplied.

## Simulation Scratch Recovery

Simulation scratch stays in the system temporary directory. Each workspace is
published with a self-contained ownership marker before it receives source or
build outputs. A native lease outside the disposable directory protects the
owner through cleanup. Normal completion and failures preserve diagnostics in
`target/ci/simulation/` and remove owned scratch. The next simulation recovers
unlocked scratch after a crash, even when the original checkout's records are gone.

Legacy, unmarked, linked, malformed or live scratch is retained with its path and
an explanation. Inspect those paths and establish that their owner has stopped
before removing them manually. New versioned stable paths allow simulations to
start while unverifiable legacy directories remain. Never delete a native lease
file or redirect TEMP into the checkout.
