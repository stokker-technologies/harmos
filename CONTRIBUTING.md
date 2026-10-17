# Contributing to Harmos

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
mise run sync
```

Run the complete example with `mise run build`, then `target/debug/schematic-cli`. Read the relevant source-level contracts and compiled examples before changing
a public contract.

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

Use Hub's repository-wide authored-test caps: unit 0, integration 128,
acceptance 16, and system 8. Do not add project limit overrides. Keep related
public contracts in cohesive integration scenarios with independent fixture
lifecycles. Private unit assertions must not be relocated to integration tests.
Parameterized variants retain their assertions without adding authored functions.

Run the matching task while iterating, then the repository gates:

```sh
mise run check
mise run test
```

`mise run build` builds the workspace and the schematic example once and stamps
the example's artifacts in `target/fixtures/stamp.json`. Test lanes never compile:
their preflight refuses an absent or stale stamp before any story runs.

Acceptance consumes the exact release binaries and Wasm component staged by
`mise run build:release`. Local `mise run test` prepares them first; native CI transfers
them from the matching platform build job and fails on missing or mismatched assets.
Scenarios run those artifacts directly. Keep scenario timing outside preparation.
Cargo metadata determines custom target directories and platform executable suffixes.

If a platform, Wasm target, or throughput test cannot run locally, say so in
the pull request instead of weakening the gate.

## Benchmarks

Run `mise run bench` for the Rust and Python benchmark suites. `bench:rust` and
`bench:python` select one language. Rust includes managed state, operations,
streams, storage, native sidecar transport, host-to-Wasm-guest round trips,
and the managed native-to-guest-to-native dependency chain.
Native stream consumption covers both in-process and executable sidecars.
The guest is built and activated before its timed calls. Sidecar transport
reports distinguish binary in-memory and process communication. The combined
chain uses in-process `PluginNative` carriers and a Wasmtime component; it does
not claim executable-process latency. Its explicitly selected ignored benchmark
scenario warms up ten calls, then records twenty samples of fifty calls for each
payload size, excluding activation and shutdown. Request construction, encoding,
managed nested calls, response validation and finalization are timed. It keeps
no per-call request history. Throughput counts useful request/response bytes,
not every internal carrier crossing.
The capacity sweep runs its configured warmup and repetitions once; each trial
is saved with payload size, target rate, achieved throughput and elapsed seconds.
The Python suite measures the simulation-oracle workflow, including process
startup after compilation is prepared; it is not a transport latency measurement.

Benchmark evidence is Git-ignored under `.stkr/benchmarks/`, including `latest.json`,
`bench.csv`, and `bench.md`. Native measurements remain in the build tree and the
runner log in `.stkr/state/bench.log`. Benchmark paths in `tools/python/pyproject.toml` and native Cargo bench targets
must produce fresh measurements for the selected suite.
Failed commands retain diagnostics and return failure. Share the relevant evidence,
logs, and native measurements; compare matching profiles, features, workloads, and
hardware.
