# Harmos Examples

Source: `examples/`.

Start with the [schematic walkthrough](schematic.md). It creates a document,
places and connects symbols, asks a plugin for advice, applies a route, and saves
and reopens the result.

| Backend | Command | Reach |
|---|---|---|
| Native Sidecar | `target/build/dev/linux-x86_64/bin/schematic-cli` | Shared analysis and routing, plus native directory access |

Run `mise run build` first; it builds the example once beside the workspace.
On Windows, use `windows-x86_64` and the `.exe` suffix for native executables.

Shared models, vertical operations, host app, CLI and sidecar have separate crates under
`examples/schematic/`, a Cargo workspace of its own. The short advanced scenario adds streamed
analysis and cancellation. The example's JSONL persistence adapter is `examples/schematic/storage/`.
