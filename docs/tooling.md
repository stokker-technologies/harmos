# Repository Tooling

Mise is the public entry point. The project-owned Python task wrappers live in
`tools/python/src/harmos_tools/`; the root uv workspace owns `.venv/` and
`uv.lock`. Run `mise run prepare:tools` to synchronize the locked tools.

## Workstation Builds

Compiling task wrappers in `tools/python/src/harmos_tools/build/machine.py`
share `native-build.lock` directly under the OS user cache directory:
`%LOCALAPPDATA%` on Windows, `$XDG_CACHE_HOME` (or `~/.cache`) on Unix.
Dokime and Harmos use the same path. Windows `LockFileEx` and Unix `flock`
serialize native compilation across repositories and worktrees. Waiting tasks
print the holder PID, project and task. The lock spans the child command;
nested commands inherit ownership. Process exit releases the OS lock,
including crashes; stale holder metadata is replaced after acquisition.
Never delete the lock file. The lock also remains enabled in CI, where hosted
jobs have separate machines; native local CI simulation jobs share the lock.
No worker or compiler thread limits are added.

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
the override with `Remove-Item Env:RUSTC_WRAPPER`. The build lock remains active.
`mise run test:tooling` proves serialization and OS release after a holder dies.

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
