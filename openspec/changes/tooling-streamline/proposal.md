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

| Task | Arguments After `--` | Behaviour |
| --- | --- | --- |
| `prepare` | `[--fixtures] [--profile <dev\|release>] [--ci <pack\|extract\|summary\|nightly-gate\|nightly-report\|mirror\|runners\|review\|release-checks\|required>] [-- <arguments...>]` | Synchronize all locked Python groups; optionally join existing fixtures or run CI bookkeeping. Fixture and artifact arguments retain their existing names. |
| `generate` | `[--profile <dev\|release>]` | Regenerate Python clients from existing Product schemas. Retained while the client facility exists. |
| `format` | `[--check] [--language <rust\|python\|toml>]` | Format selected maintained languages, or check their formatting. This repository has no TypeScript task. |
| `lint` | `[--language <rust\|python\|toml>] [--extended]` | Lint selected languages; TOML has formatting checks only. Extended Rust lint covers optional compatibility configurations. |
| `check` | `[--language <rust\|python\|toml>] [--extended] [--repository] [--generated]` | Check formatting, lint, repository configuration and selected generated drift; Python includes tooling tests and consumes generated Product clients. `--repository` or `--generated` selects only that check. |
| `build` | `[--profile <dev\|release>] [--artifact <product\|tests\|bench>] [--package <name> [--check]] [--compatibility] [--cpp] [--extended]` | Product by default; Tests exports an archive; Bench uses the fixed bench profile. Package selection invokes Cargo directly. Compatibility and C++ remain optional while their consumers exist; extended applies to Tests or Bench. |
| `test` | `[--language <rust\|python>] [--layer <unit\|integration\|acceptance\|system>] [--package <name> --target <lib\|TEST_TARGET>] [--exact <name>] [--coverage] [--profile <dev\|release>] [--filter <expression>] [--stress-count <count>] [--starter] [--cpp] [--extended]` | Package selection runs one Cargo target without an archive; exact filtering belongs to that selection. Other Rust lanes consume archives and fixtures. Python has an integration tooling suite. Integration retains filter/stress options; starter retains its filter. Existing starter, C++ and extended facilities remain reachable. |
| `bench` | `[--layer <unit\|integration\|system>] [--refresh-baseline] [--extended]` | Consume existing benchmark executables and matching fixtures; retain baseline refresh and optional feature selection. |
| `package` | `[--language <rust>] [--profile <dev\|release>]` | Assemble existing Rust Product outputs without compiling. |
| `release:prepare` | `[--review]` | Prepare version and lock changes; `--review` also validates the configured runner allowlist, commits prepared changes and opens the release review in the existing hosted flow. |
| `release:tag` | None | Tag the merged release review and fast-forward main; hosted GitHub calls own identity reporting and the exact-commit CI guard. |
| `release:rehearse` | `[--target <native\|TARGET\|all>] [--profile <dev\|release>] [--stage-only]` | Rehearse existing release packages, without publication credentials. |
| `release:publish` | `[--step <dry\|assets\|rust\|finalize\|nightly>]` | Default: draft assets, then finalize; hosted GitHub asset publication owns identity reporting and the exact-commit CI guard. Individual steps preserve dry runs, crate publication and Nightly publication. |
| `release:verify` | None | Verify published release assets and opted-in installers. |

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
- **Tooling stays this project's own copy,** kept harmonized with Dokime's: the Cargo wrapper,
  test runner, release scripts, cache and artifact helpers are fixed here and there alike.
- **CI workflows call tasks.** One job template per kind of work, as a matrix over languages,
  layers and platforms. A job checks out, sets up mise, restores caches, downloads the build it
  consumes, runs one task and uploads results. Tasks own archiving, summaries, the Nightly
  freshness check and report, the release review and the mirror push. The target is about 1,400
  to 1,900 lines.

## Settled

- Tooling is duplicated between Harmos and Dokime on purpose: each project is an independent
  module with its own copy. The copies are kept harmonized, with the same task names, arguments,
  module layout and behaviour, and a fix made in one is made in the other.
- Impact planning is removed. Every pull request runs every job, and its dependency graph is
  deleted.
- The composite actions stay in each repository, reduced to the setup action. The duplication
  between products is accepted, since shared tooling no longer renders CI.

## Impact

- `mise.toml`, `tools/python`, `.github/workflows` and actions, and the contributor guide. No
  product behaviour changes.
