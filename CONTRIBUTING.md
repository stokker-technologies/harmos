# Contributing To Harmos

Harmos is open to bug reports, documentation improvements, focused runtime
changes, and examples that clarify how the journal model works in a real
application.

## Before You Start

Use the [issue tracker](https://github.com/stokker-technologies/harmos/issues)
for bugs and proposals. Open an issue before a substantial API, wire-format,
storage-contract, or crate-boundary change. Harmos is pre-1.0, but clean breaks
still need a clear reason and updated callers, docs, and examples.

A useful bug report includes the Harmos revision, operating system, minimal
reproduction, expected order or position, actual result, and relevant logs.

## Development Setup

The repository supports Windows and Linux. Install
[`mise`](https://mise.jdx.dev), then prepare the checkout:

```sh
mise trust
mise install --locked
mise run prepare:tools
```

Run the complete example with `mise run build`, then `target/build/dev/linux-x86_64/bin/schematic-cli` on Linux. Read the relevant source-level contracts and compiled examples before changing
a public contract.

Each native CI lane prepares the locked Python tooling before running commands
with `--no-sync`; runners do not share a prepared environment. Python type checks
run with `lint:python`. Generated-client checks type-check the complete output
and lint Harmos-emitted modules; protoc's `_wire` modules are excluded from Ruff.

Embedded CLI starters live under `crates/cargo-harmos/templates/` with a `.tmpl`
filename suffix so Cargo includes them in the published tool package. The build
removes that suffix from embedded output names. Keep the suffix when editing
authored starters; refresh Hub-owned starter files through `stkr project update`.

## Make A Change

- Preserve the direction of dependencies. Applications and adapters depend on
  Harmos; the core runtime does not learn about their storage engine, UI, or
  transport choices.
- Keep optional facilities behind their named Cargo feature and maintain the
  facade boundaries checked by the census tests.
- Update the README and compiled examples with public API changes.
- Add focused tests at the behavioral boundary. Prefer a complete journal,
  recovery, work, or artifact workflow over many implementation-level tests.
- Keep changes small enough to review and use Conventional Commit subjects,
  such as `fix(journal): retain position across recovery` or `docs: explain
  stream loss accounting`.

Tests follow their behavior: pure unit assertions, in-process integration,
command-line acceptance and three full schematic system walkthroughs. Keep each
story cohesive and independent; preserve coverage when removing superseded
fixtures. Project-wide authored-story limits and lane deadlines live in `stkr.toml`.
Use notifications and controlled clocks rather than sleeps or nested timeout guards.

Finish the combined source cutover before validation, then use the repository flow:

```sh
mise run ci
```

`mise run build` exports packageable products and generated clients.
`mise run build:tests` creates the shared workspace archive and stamps native/Wasm
inputs in `target/build/dev/<target>/fixtures/stamp.json`.
Ordinary test lanes consume those exact development
artifacts and never rebuild fixtures. Their preflight refuses missing, stale or
changed inputs. Narrow authoring acceptance executes the compiler only when
compilation or dev/rebuild is the product behavior being tested, using the
build-staged offline dependency closure. Keep preparation outside scenario timing.

If a platform, Wasm target, or throughput test cannot run locally, say so in
the pull request instead of weakening the gate.

## Benchmarks

Run `mise run build:bench` followed by `mise run bench` for the Rust benchmark suites, or choose `bench:unit`,
`bench:integration` or `bench:system`. Pure codec work, managed runtime/storage
behavior and executable transport have separate owners. Fixture activation and
shutdown stay outside timed calls; report the payload, batching, carrier, profile,
revision and hardware with any result.

The managed runner owns ignored evidence under `.stkr/benchmarks/`. Preserve its
original inputs and measurements when comparing runs. A failed suite remains a
failure; do not weaken a gate to publish a result.
