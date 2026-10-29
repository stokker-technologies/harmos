# Schematic Editor

One terminal application, with interchangeable native and sandboxed plugins.
The algorithms are deliberately small so the Harmos workflow stays visible.

## Run It

From the repository root, after `mise install --locked`, `mise run prepare:tools` and `mise run build`:

```sh
target/build/dev/linux-x86_64/bin/schematic-cli
target/build/dev/linux-x86_64/bin/schematic-cli --guest
```

Read [walkthrough.rs](cli/src/walkthrough.rs) alongside the output. It creates a
fresh temporary document, places and connects two symbols, moves one, validates
the schematic, applies a proposed right-angle route, undoes/redoes it, then saves
and reopens it. Both backends produce the same shared results. The final line is:

```text
Reopened: 2 symbols, 1 net, 3 route points
```

Startup emits `schematic.plugin.initializing` and `schematic.plugin.ready` on
stderr for both backends. Guest component compilation finishes before
initialization starts; the readiness interval covers plugin initialization.

## Follow The Ownership

| Directory | Owns |
| --- | --- |
| [domain](domain/src/lib.rs) | Shared models, stream and stored-object contracts. |
| [operations](operations/src/lib.rs) | Vertical operation declarations, separate requests, optional implementations and catalogs for both carriers. |
| [app](app/src/app.rs) | Document state, transactions, persistence, runtime composition, and plugin lifecycle. |
| [cli](cli/src/walkthrough.rs) | The readable teaching sequence and terminal output. |
| [guest](guest/src/lib.rs) | Sandboxed Wasm composition of the shared operation catalog. |
| [sidecar](sidecar/src/lib.rs) | Native composition, directory access and progress publishing dependencies. |

The plugins return advice. The host applies it through `ApplyRoute`, preserving
one owner for edits, undo/redo, and recovery. Neither plugin imports the app.
The operations crate exposes contracts with default features. Its `runtime` feature
enables common SDK implementations; each carrier supplies its own dependencies.

`SchematicEditor` implements the shared `Lifecycle`. Its async `prepare` opens
the recovery journal and recordings, configures `Setup<Self>`, and places the
operation catalog. `app::launch` calls `Runtime::launch`; shutdown drains work
before owner stop and cleanup. Tests can replace prepared resource allocations
through `Runtime::launch_with` while keeping that preparation path.

Both plugins depend on the `harmos` facade, with its `sidecar` or `guest`
feature. Their constant `CATALOGS` selects the same descriptors for inert
inspection and executable binding; prepare supplies resources through setup.

## Streaming And Native Access

After running the build tasks above, add `--advanced` to the CLI:

```sh
target/build/dev/linux-x86_64/bin/schematic-cli --advanced
target/build/dev/linux-x86_64/bin/schematic-cli --guest --advanced
```

[advanced.rs](cli/src/advanced.rs) drains three analysis findings, joins the
response, then starts another analysis and cancels it with settlement.

The native catalog also exposes `AnalyzeUpload { title }` (client stream of
snapshots) and `AnalyzeExchange { title }` (snapshot input and finding output).
Both retain one shared analysis permit until input EOF or cancellation. Blank or
oversized titles and invalid snapshots return an invalid-input failure. Each
accepted snapshot publishes `AnalysisProgress { title, checked }` on the
sidecar-declared stream `schematic.analysis_progress`. The domain's
`ValidatedSchematic` object rejects missing identities and invalid connections
at storage encode and decode boundaries.

Only the sidecar exposes `ListDirectories`. Its empty request returns sorted
names of immediate subdirectories in the caller's current working directory,
which the host explicitly supplies to the child. The guest catalog omits this
operation because this guest has no filesystem grant. WebAssembly can support
filesystem access when a host provides it; this example does not.

The CLI accepts `--sidecar <executable>` or `--guest <component>`. Without either,
it selects `plugins/schematic-sidecar` relative to its own executable, falling
back to a sibling executable for Cargo development. `--guest` without a path
selects `plugins/schematic-guest.wasm` relative to the CLI. Explicit paths remain
relative to the caller's working directory.

## Downloaded Examples And Schemas

Native CI provides an example ZIP with `examples/schematic/`, containing the CLI,
both ready-to-load plugins, and `grpc/<producer>/operations/`. Run the extracted
CLI from any working directory; native directory listing uses that directory.
Windows executables have the `.exe` suffix.

The CLI, sidecar and guest build scripts derive their schema bundles from the
shared operations crate in its contract-only configuration. Each bundle contains `manifest.txt`, `descriptors.bin` and
package-relative `.proto` files, preserving imports. The native catalogs include
`ListDirectories`; the guest catalog includes only portable operations. The
`schematic.v1.Schematic` schema describes known methods. Exporting it does not
start a gRPC server; runtime reflection describes the methods actually exposed
by a running host, including dynamic plugins.

CI packages retain `cargo-harmos`, the schematic CLI and its native and guest plugins.
Examples and test reports remain GitHub Actions artifacts; Rust crates publish to crates.io.

## Tests And Persistence

Product Build exports the CLI and plugins under `target/build/dev/<target>/bin/`.
Fixture preparation joins the Product and Tests manifests and stamps those exact bytes in
`target/build/dev/<target>/fixtures/stamp.json`, and the system stories in
`crates/*/tests/system/` drive those artifacts.

The [JSONL adapter](storage/) is the example's persistence format. A document's recovery journal is distinct from a native
plugin sidecar. Saving uses the adapter's checkpoint protocol after runtime
settlement and publishes the document through an atomic file replacement.

## Transfer Benchmarks

The `benchmark` feature on the sidecar and guest enables shared record, sample
and file-byte transfer endpoints. Their [declarations and implementations](operations/src/transfer.rs) are reused by both example executables.
`Put`/`Take` transfer one owned batch per unary call; `Upload`/`Download` carry
bounded batches through streams. Transfer IDs isolate concurrent datasets.
The feature also exposes a `Hash` bridge to the host `HashFile` operation. Each
carrier benchmark verifies a local-file range and digest through this bridge
before timing transfers.

`mise run build:rust:bench` builds and stamps these feature-enabled artifacts.
The host benchmarks transfer 10,000 records, 30 MiB of file bytes and 1,000,000
samples in both directions, running the datasets concurrently on one reused
host/plugin instance per carrier. Setup and verification are outside timing.
The transfer store is in memory; storage benchmarks separately measure disk I/O.
