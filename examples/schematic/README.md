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

## Follow The Ownership

| Directory | Owns |
| --- | --- |
| [domain](domain/src/operations/mod.rs) | The model and the operation catalogs: portable `schematic` operations and native `directories` access. |
| [app](app/src/app.rs) | Document state, transactions, persistence, runtime composition, and plugin lifecycle. |
| [cli](cli/src/walkthrough.rs) | The readable teaching sequence and terminal output. |
| [guest](guest/src/operations/mod.rs) | Validation, routing, and streamed analysis in a sandboxed Wasm component. |
| [sidecar](sidecar/src/operations/mod.rs) | The shared operations in a native child process, directory access, and upload/exchange analysis. |

The plugins return advice. The host applies it through `ApplyRoute`, preserving
one owner for edits, undo/redo, and recovery. Neither plugin imports the app.
The domain crate and SDKs compile without the host runtime.

## Streaming And Native Access

After running the build tasks above, add `--advanced` to the CLI:

```sh
cargo run -p schematic-cli -- --advanced
cargo run -p schematic-cli -- --guest target/artifacts/schematic-tools.wasm --advanced
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
shared Rust domain crate. Each bundle contains `manifest.txt`, `descriptors.bin` and
package-relative `.proto` files, preserving imports. The native catalogs include
`ListDirectories`; the guest catalog includes only portable operations. The
`schematic.v1.Schematic` schema describes known methods. Exporting it does not
start a gRPC server; runtime reflection describes the methods actually exposed
by a running host, including dynamic plugins.

CI packages retain `cargo-harmos`, the schematic CLI and its native and guest plugins.
Examples and test reports remain GitHub Actions artifacts; Rust crates publish to crates.io.

## Tests And Persistence

Product Build exports the CLI and plugins under `target/build/dev/<target>/bin/`.
Tests Build stamps those exact Product bytes in
`target/build/dev/<target>/fixtures/stamp.json`, and the system stories in
`crates/*/tests/system/` drive those artifacts.

The [JSONL adapter](storage/) is the example's persistence format. A document's recovery journal is distinct from a native
plugin sidecar. Saving uses the adapter's checkpoint protocol after runtime
settlement and publishes the document through an atomic file replacement.
