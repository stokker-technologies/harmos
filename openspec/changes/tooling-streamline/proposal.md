# Tooling Streamline

Status: direction agreed with the owner on 2026-11-10; implementation has not started and waits
for the owner's go. The same change exists in Dokime; shared tooling is fixed once and adopted by
both.

## Why

`mise.toml` defines 50 tasks and `tools/python` holds about 9,700 lines behind them, of which 78
percent is line for line the same as Dokime's. Several tasks exist only for facilities that are
being removed. Incremental compilation is switched off for local builds. The CI workflows are
about 6,000 lines, most of it checkout, setup, cache and artifact steps written out per job and
per operating system.

## What Changes

- **A small set of entry points that need no knowledge of the languages in the project:**
  `prepare`, `format`, `lint`, `check`, `build`, `test`, `bench`, `package` and the release
  steps. Language and layer are arguments.
- **Incremental compilation is disabled by one variable set in CI** and nowhere else. The
  compiler cache stays for local builds.
- **The storage preflight is gone** from the shared build tooling; this project adopts that
  tooling and drops the wrapper code that existed only for it.
- **Tasks for removed facilities are deleted:** `build:cpp`, `test:rust:cpp`,
  `test:rust:starter`, the generated Python client check and its producer, and the tooling that
  collects and packages through the authoring tool.
- **Compatibility and extended selections are pruned** to what still proves something after the
  removals.
- **Shared tooling is fixed in its shared source,** not here: the Cargo wrapper, test runner,
  release scripts, cache and artifact helpers. Only product-specific fixture code stays local.
- **CI workflows call tasks.** One job template per kind of work, as a matrix over languages,
  layers and platforms. A job checks out, sets up mise, restores caches, downloads the build it
  consumes, runs one task and uploads results. Tasks own archiving, summaries, the Nightly
  freshness check and report, the release review and the mirror push. The target is about 1,400
  to 1,900 lines.

## Open Points

1. Impact planning: removed, so every pull request runs everything, or kept with one owner.
2. Where the composite actions live, since they are 99 percent identical in both products.

## Impact

- `mise.toml`, `tools/python`, `.github/workflows` and actions, and the contributor guide. No
  product behaviour changes.
