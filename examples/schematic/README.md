# Schematic Editor

One terminal application, with interchangeable native and sandboxed plugins.
The algorithms are deliberately small so the Harmos workflow stays visible.

## Run It

From the repository root, after `mise install --locked` and `mise run sync`:

```sh
mise run run:schematic:sidecar
mise run run:schematic:guest
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
| [contract](contract/src/operations/mod.rs) | Portable requests, responses, and struct-owned operation declarations. |
| [app](app/src/app.rs) | Document state, transactions, persistence, runtime composition, and plugin lifecycle. |
| [cli](cli/src/walkthrough.rs) | The readable teaching sequence and terminal output. |
| [guest](guest/src/operations/mod.rs) | Validation, routing, and streamed analysis in a sandboxed Wasm component. |
| [sidecar](sidecar/src/operations/mod.rs) | The same operations in a native child process, plus directory access. |

The plugins return advice. The host applies it through `ApplyRoute`, preserving
one owner for edits, undo/redo, and recovery. Neither plugin imports the app.
The contract and SDKs compile without the host runtime.

## Streaming And Native Access

After running the build tasks above, add `--advanced` to the CLI:

```sh
cargo run -p schematic-cli -- --advanced
cargo run -p schematic-cli -- --guest target/artifacts/schematic-tools.wasm --advanced
```

[advanced.rs](cli/src/advanced.rs) drains three analysis findings, joins the
response, then starts another analysis and cancels it with settlement.

Only the sidecar exposes `ListDirectories`. Its empty request returns sorted
names of immediate subdirectories in the caller's current working directory,
which the host explicitly supplies to the child. The guest catalog omits this
operation because this guest has no filesystem grant. WebAssembly can support
filesystem capabilities when a host provides them; this example does not.

The CLI accepts `--sidecar <executable>` or `--guest <component>`. Without either,
it selects `schematic-sidecar` beside its own executable.

## Tests And Persistence

The [app tests](app/tests/app/) cover edits, recovery, streams, and deterministic
simulation. Acceptance compares both backends and checks native directory access
against a temporary directory. Detailed shutdown, backlog-flush, and process
settlement stress belongs in the [lifecycle fixture](../../tests/fixtures/lifecycle/).

The [JSONL adapter](../../tests/support/jsonl-adapter/) is test infrastructure
shared with this example. A document's recovery journal is distinct from a native
plugin sidecar. Saving uses the adapter's checkpoint protocol after runtime
settlement and publishes the document through an atomic file replacement.
