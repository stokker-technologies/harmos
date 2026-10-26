# Contributing

Help improve Harmos with focused fixes, clear API feedback and useful examples.
Discuss changes to public contracts before implementing them.

## Development Setup

Install [mise](https://mise.jdx.dev), then prepare the checkout:

```sh
mise trust
mise install --locked
mise run prepare:tools
```

Windows and Linux are supported. The [Schematic example](examples/schematic/README.md)
is a complete application for understanding composition, plugins and storage.

## Development Workflow

Preserve clear boundaries between domain contracts, runtime resources and
application-owned persistence. Update affected consumers and examples when
changing a contract; flag unrelated stale examples rather than hiding them.

Use Conventional Commit subjects and keep generated files with their source.
The root uv workspace owns the Python environment and lockfile.

## Tests And Checks

Run targeted checks while implementing. After all agreed source work is
complete and targeted proof passes, push the authorized candidate and immediately
run local validation in parallel with hosted CI:

```sh
mise run ci:simulate
```

The default run covers all declared required platforms. Changes limited to skills,
agent guidance, README or CONTRIBUTING files select no execution jobs; the
required status gate still completes. Other documentation uses its declared
checks, while mixed source or policy changes retain full selection.
Required hosted CI controls integration. Keep auto-merge enabled; it may merge
independently of local completion. Report and investigate local failures.

Product and Tests compile independently with matching Clippy invocations.
Tests use the default unoptimized test profile; optional extended tests remain
explicit local work. Nightly runs ordinary verification with release Product
builds and downloadable unsigned packages, plus coverage, benchmarks and
`test:rust:starter` for generated guest/sidecar compilation and development lifecycles.
Ordinary acceptance retains the other product stories without compiling starters.
Correctness tests have a minimum 120-second deadline, preserving larger bounds.
Use benchmarks for performance measurements rather than tightening test deadlines.

For a focused Rust compiler check or selected test, use `mise run build:rust:focused`
with the Cargo operation and selection arguments. This task applies the standard
compiler-output preflight; it does not replace the final validation graph.

Storage-engine stories exercise `harmos-storage` without a host runtime; its
real `WorkerOwner` owns the canonical worker and settlement. Runtime storage
stories cover host composition, managed journal projection and operation dispatch.
Inject file journal faults at the public `FileBackend` boundary, retaining its
parsing, rollback and leasing. The specialized integration entrypoints are
`harmos-domain/tests/message/main.rs` and `harmos-guest/tests/kit/main.rs`, both
under `crates/`.

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
retains producer identity and byte hashes. Acceptance selects the Cargo
subcommand and both plugin Product artifacts; system selects the Schematic CLI
and both plugins.

Compiler execution for generated projects, macro diagnostics and development
rebuild/cancellation belongs to acceptance; integration consumes already-built
artifacts. Generated-project stories use starter manifests and locks, the
build-owned Cargo vendor directory and a hash-checked Python wheelhouse covering
root tooling and both starter locks. Python binding interoperability is
Rust-driven acceptance using the root `.venv`; generated Python packages remain
consumer-owned.

## Pull Requests

Target `dev` and explain the user-visible problem, resulting behavior and
verification. Keep required checks enabled and include no private data or
credentials. Contributions use the [Apache-2.0 license](LICENSE).

Release preparation and publication follow the separate protected release
workflow; an ordinary pull request does not publish packages.

## Reporting Issues

Use the [issue tracker](https://github.com/stokker-technologies/harmos/issues).
Include the revision, operating system, minimal reproduction, expected result,
actual behavior and relevant sanitized logs.
