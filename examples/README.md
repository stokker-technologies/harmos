# Harmos Examples

Start with the [schematic walkthrough](./schematic/README.md). It creates a document,
places and connects symbols, asks a plugin for advice, applies a route, and saves
and reopens the result.

| Backend | Command | Reach |
|---|---|---|
| Native Sidecar | `target/debug/schematic-cli` | Shared analysis and routing, plus native directory access |
| Sandboxed Guest | `target/debug/schematic-cli --guest target/fixtures/schematic-guest/schematic_guest.component.wasm` | The same analysis and routing, with no filesystem access |

Run `mise run build` first; it builds the example once beside the workspace.

The portable contract, host app, CLI, guest, and sidecar have separate crates under
`schematic/`, a Cargo workspace of its own. The short advanced scenario adds streamed
analysis and cancellation. The example's JSONL persistence adapter is `schematic/storage/`.
