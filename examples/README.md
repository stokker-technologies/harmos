# Harmos Examples

Start with the [schematic walkthrough](./schematic/README.md). It creates a document,
places and connects symbols, asks a plugin for advice, applies a route, and saves
and reopens the result.

| Backend | Command | Capabilities |
|---|---|---|
| Native Sidecar | `mise run run:schematic` | Shared analysis and routing, plus native directory access |
| Sandboxed Guest | `mise run run:schematic:guest` | The same analysis and routing, with no filesystem capability |

The portable contract, host app, CLI, guest, and sidecar have separate crates under
`schematic/`. The short advanced scenario adds streamed analysis and cancellation.
Regression fixtures and JSONL persistence support live under `tests/`.
