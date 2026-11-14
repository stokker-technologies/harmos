# Harmos Examples

Source: `examples/`.

Start with the [schematic walkthrough](schematic.md). It creates a document,
places and connects symbols, asks a plugin for advice, applies a route, and saves
and reopens the result.

| Backend | Command | Reach |
|---|---|---|
| Native Sidecar | `target/build/dev/linux-x86_64/bin/schematic-cli` | Shared analysis and routing, plus native directory access |
| Sandboxed Guest | `target/build/dev/linux-x86_64/bin/schematic-cli --guest` | The same analysis and routing, with no filesystem access |

Run `mise run build` first; it builds the example once beside the workspace.
On Windows, use `windows-x86_64` and the `.exe` suffix for native executables.

Shared models, vertical operations, host app, CLI, guest and sidecar have separate crates under
`examples/schematic/`, a Cargo workspace of its own. The short advanced scenario adds streamed
analysis and cancellation. Application contracts live in `examples/schematic/proto/schematic/v1/operations.proto`;
`examples/schematic/protocol` owns their generator and generated-code drift test.
The example's JSONL persistence adapter is `examples/schematic/storage/`.
Its own `Files` and `Uploads` contracts use shared `harmos.v1.File` and `Upload`
messages over private native storage operations. Both plugins declare typed routing
settings and read them through caller-scoped plugin-host `Settings.List`.
Client roots use `harmos.client.v1`; plugin implementations use
`harmos.plugin.v1`, and their `context` calls use `harmos.plugin.host.v1`.

The shared client-composition fixture is
`sdk/tests/fixtures/client/proto/application/v1/plugins/annotations.proto`.
It adds application-owned `Annotations.Get` under the library `Plugins` parent,
alongside the sibling application `Jobs` root:

```text
client.jobs.start
client.jobs.progress.watch
client.plugins.annotations.get
client.plugins.settings.list
```

The library plugin entry is the sole plugin record. Application annotations remain
application-owned; cleanup follows the in-process deletion watch and startup
reconciliation and is eventual. Library-stored settings are deleted atomically
with the plugin entry. In-place `Plugins.Replace` preserves the entry name,
settings and application data.


Each operation file owns its request, response, policy and execution. Shared data
lives in `domain`; plugin lifecycle preparation owns resources. Calls descend
from higher-level operations to lower-level operations without cycles. Driver
results cross the operation boundary as plugin-owned messages.

Orchestration reads top to bottom under Title Case action comments: construct
`let request = ...;`, then invoke `ctx.operation(OPERATION, request).await?;` in a native function handler. Repeat actions
in the flow; keep reusable library logic with its owner. Compute named checks
before combining them, and suffix physical quantities with their units. Callees
own timeouts, delays and retries; callers override policy only for one call.
