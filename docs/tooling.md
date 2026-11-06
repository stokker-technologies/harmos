# Repository Tooling

Mise is the public entry point. The project-owned Python task wrappers live in
`tools/python/src/harmos_tools/`; the root uv workspace owns `.venv/` and
`uv.lock`. Run `mise run prepare:tools` to synchronize the locked tools.

## Workstation Builds

Native compile tasks run concurrently across repositories and worktrees, with
no workstation build lock or added worker/thread limits. The task wrappers keep
the standard compiler-output preflight and share the bounded compiler cache.

For non-CI runs, `mise.toml` selects `RUSTC_WRAPPER=sccache`,
`SCCACHE_DIR=<OS user cache>/sccache`, `SCCACHE_CACHE_SIZE=20G` and disables
incremental compilation. Each worktree keeps its own `target/`; matching
cacheable crate compilations can reuse results across worktrees. CI retains
its existing per-profile cache settings. sccache is already a pinned mise tool.
Local wrappers reuse the existing `ci/compiler.py` foreground-server helper with
an owned endpoint, so an older daemon cannot silently select another store.
Only that owned server is stopped after the command; each task prints its cache
hit and miss counts.
If allocation failures recur, disable it for one run by setting
`RUSTC_WRAPPER` to an empty value:

```sh
RUSTC_WRAPPER= mise run build
```

In PowerShell use `$env:RUSTC_WRAPPER = ''; mise run build`, then remove
the override with `Remove-Item Env:RUSTC_WRAPPER`.
`mise run test:tooling` proves concurrent task entry and child exit-code
propagation.

## Focused Integration Tests

The integration task accepts a nextest filter and repeat count while retaining
the configured per-test deadlines. It consumes the existing test archive and fixtures:

```sh
mise run test:rust:integration --filter 'package(=harmos-runtime) & test(=sidecar::observer_demand::demand_survives_reload)' --stress-count 50
```

Select `--profile release` to use an archive and fixtures from a Nightly build.

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
