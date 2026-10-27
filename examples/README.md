# Harmos Examples

Start with the [schematic walkthrough](./schematic/README.md). It creates a document,
places and connects symbols, asks a plugin for advice, applies a route, and saves
and reopens the result.

| Backend | Command | Reach |
|---|---|---|
| Native Sidecar | `target/build/dev/linux-x86_64/bin/schematic-cli` | Shared analysis and routing, plus native directory access |
| Sandboxed Guest | `target/build/dev/linux-x86_64/bin/schematic-cli --guest` | The same analysis and routing, with no filesystem access |

Run `mise run build` first; it builds the example once beside the workspace.
On Windows, use `windows-x86_64` and the `.exe` suffix for native executables.

Shared models, vertical operations, host app, CLI, guest and sidecar have separate crates under
`schematic/`, a Cargo workspace of its own. The short advanced scenario adds streamed
analysis and cancellation. The example's JSONL persistence adapter is `schematic/storage/`.
