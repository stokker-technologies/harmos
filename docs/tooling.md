# Repository Tooling

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
incremental compilation. Each worktree keeps its own `target/`. The temporary
sccache configuration sets `basedirs` to the current checkout, sibling Git
checkouts and Git worktrees under sibling `*.wt/` directories. These absolute
roots are discovered at runtime; the longest matching prefix is stripped from
cache keys, with case-insensitive matching on Windows. CI still writes an empty
configuration and retains its existing per-profile cache settings.

The pinned [sccache 0.19.0 configuration](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Configuration.md)
supports this normalization for C/C++ compilation. Its
[Rust cache implementation](https://github.com/mozilla/sccache/blob/v0.19.0/src/compiler/rust.rs)
still hashes the original working directory, compiler arguments and `CARGO_*`
values, including checkout paths. Configuring `basedirs` therefore does not
provide Rust workspace-crate hits between different worktrees in this version.
Registry and Git dependency hits do not prove workspace-crate reuse. Compare
two previously unused worktree paths at the same commit with
`mise run build:rust:focused -- build --locked -p harmos-domain`; keep the shared
cache intact and record each task's hits and misses. Cleaning `target/` alone
does not make a previously used worktree path cold in sccache.

No `--remap-path-prefix` is added: Cargo supplies relative workspace source
paths, and no cross-worktree Rust artifact reuse has been established here.
sccache's `basedirs` changes keys, not embedded artifact paths; cached native
objects can retain the producing checkout's debug paths.

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
