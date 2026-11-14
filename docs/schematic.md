# Schematic Editor

Source: `examples/schematic/`, a Cargo workspace of its own.

One terminal application, with interchangeable native and sandboxed plugins.
The algorithms are deliberately small so the Harmos workflow stays visible.

## Run It

From the repository root, after `mise install --locked`, `mise run prepare:tools` and `mise run build`:

```sh
target/build/dev/linux-x86_64/bin/schematic-cli
target/build/dev/linux-x86_64/bin/schematic-cli --guest
```

Read [walkthrough.rs](../examples/schematic/cli/src/walkthrough.rs) alongside the output. It creates a
fresh temporary document, places and connects two symbols, moves one, validates
the schematic, applies a proposed right-angle route, undoes/redoes it, then saves
and reopens it. The native walkthrough's final line is:

```text
Reopened: 2 symbols, 1 net, 3 route points
```

The guest compiles, but its walkthrough currently refuses `Settings.List`: the
frozen library `PluginPlacement` supports native bundles and remote endpoints,
with no entry-backed guest placement. It cannot obtain another plugin's settings
or replace the library store with an application fallback.

Startup emits `schematic.plugin.initializing` and `schematic.plugin.ready` on
stderr for both backends. Guest component compilation finishes before
initialization starts; the readiness interval covers plugin initialization.

## Follow the Ownership

| Directory | Owns |
| --- | --- |
| [domain](../examples/schematic/domain/src/lib.rs) | Shared models, stream and stored-object contracts. |
| [protocol](../examples/schematic/protocol/src/lib.rs) | Generated application messages and descriptors shared by host handlers and SDK clients. |
| [operations](../examples/schematic/operations/src/lib.rs) | Generated native declarations and handlers. |
| [app](../examples/schematic/app/src/app.rs) | Document state, transactions, persistence, runtime composition, and plugin lifecycle. |
| [cli](../examples/schematic/cli/src/walkthrough.rs) | The readable teaching sequence and terminal output. |
| [guest](../examples/schematic/guest/src/lib.rs) | Sandboxed Wasm composition of the shared operation catalog. |
| [sidecar](../examples/schematic/sidecar/src/lib.rs) | Unary protobuf advice and native directory access over gRPC. |

The plugins return advice. The host applies it through `ApplyRoute`, preserving
one owner for edits, undo/redo, and recovery. Neither plugin imports the app.
The protocol crate exposes generated contracts without the host stack.
The library's shared messages live in `harmos.v1`, client calls in
`harmos.client.v1`, plugin implementations in `harmos.plugin.v1`, and calls from a
plugin to its host in `harmos.plugin.host.v1`. The SDK handler remains `context`.
Both plugins declare the typed `horizontal_first` flag, defaulting to true, and
read its effective library-stored value through plugin-host `Settings.List` when
routing. A client updates complete plugin settings through
`client.plugins.settings.{list,set}`; the plugin's caller identity scopes its read.
The native host launches its own sidecar process and creates a remote entry through
the library manager. The manager stores typed settings in the example's shared
in-memory registry for that host lifetime. Entry deletion settles the activation before
the application reaps its process. `LaunchConfig.plugins` selects this management
assembly; focused storage hosts can supply their own storage assembly instead.
Its `generate` Cargo example owns regeneration from `examples/schematic/proto/`,
and its `generated_code_matches_proto` test checks source and descriptor drift.
Buf formats and lints the example contracts alongside `proto/harmos/`. The operations
crate owns native implementations without a feature gate. The guest uses the same generated
messages with SDK operation builders. Native analysis publishes
progress through its host-owned callback resource.

`SchematicEditor` implements the shared `Lifecycle`. Its async `prepare` opens
the in-memory storage and recording consumer, configures `Setup<Self>`, and places the
operation catalog value. The application composes `transactions_state_catalog()`,
`records_registry()` and stream definitions as values. Transaction entries explicitly
select `.reversible::<Inverse>()` or `.barrier()`. `app::launch` calls `Runtime::launch`; shutdown drains work
before owner stop and cleanup. Tests can replace prepared resource allocations
through `Runtime::launch_with` while keeping that preparation path.

The native plugin depends on `harmos`; its unary protobuf contracts and
operation builder values live in `sidecar/src/operations.rs`, using messages generated
from `examples/schematic/proto/schematic/v1/operations.proto`. It ends in `serve()`.
The host uses managed gRPC placement and checks its `Instance.Get` identity.
The guest uses `harmos/wasm` and one `harmos_register` function returning SDK builders.

## Streaming and Native Access

After running the build tasks above, add `--advanced` to the CLI:

```sh
target/build/dev/linux-x86_64/bin/schematic-cli --advanced
target/build/dev/linux-x86_64/bin/schematic-cli --guest --advanced
```

[advanced.rs](../examples/schematic/cli/src/advanced.rs) drains three analysis findings, joins the
response, then starts another analysis and cancels it with settlement.

The native host also exposes `AnalyzeUpload { title }` (client stream of
snapshots) and `AnalyzeExchange { title }` (snapshot input and finding output).
Both retain one shared analysis permit until input EOF or cancellation. Blank or
oversized titles and invalid snapshots return an invalid-input failure. Each
accepted snapshot publishes `AnalysisProgress { title, checked }` on the
host-declared stream `schematic.analysis_progress`. `AnalyzeSchematic`,
`AnalyzeUpload` and `AnalyzeExchange` execute in the host so their client-facing
request and result streams remain available; plugin `Invoke` calls are unary.
Progress uses the host's typed stream manager and remains available after the
plugin stops. `Put`, `Take`, `Upload` and `Download` share the host's transfer store.
The domain's
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

## Downloaded Examples and Schemas

Native CI provides an example ZIP with `examples/schematic/`, containing the CLI,
both ready-to-load plugins, and `grpc/<producer>/operations/`. Run the extracted
CLI from any working directory; native directory listing uses that directory.
Windows executables have the `.exe` suffix.

The CLI build script derives its schema bundle from the
native operations crate without launching handlers. Each bundle contains `manifest.txt`, `descriptors.bin` and
package-relative `.proto` files, preserving imports. The native catalogs include
`ListDirectories`; the guest catalog includes only portable operations. The
`schematic.v1.Schematic` schema describes known methods. Exporting it does not
start a gRPC server. Client generation consumes the CLI host's captured schema;
native plugin discovery uses `Instance.Get` instead of an embedded inventory or an export flag.
Native packages obtain identity through `Instance.Get`, call `Stop`, and contain the
executable without framed-sidecar registrations. Bundle inspection and extraction
remain inert; inspecting a standalone native executable starts it to read its catalog.

CI packages retain the schematic CLI and its native and guest plugins.
Examples and test reports remain GitHub Actions artifacts; Rust crates publish to crates.io.

## Tests and Persistence

Product Build exports the CLI and plugins under `target/build/dev/<target>/bin/`.
Fixture preparation joins the Product and Tests manifests and stamps those exact bytes in
`target/build/dev/<target>/fixtures/stamp.json`, and the system stories in
`crates/*/tests/system/` drive those artifacts.

The application owns [document serialization](../examples/schematic/app/src/document.rs).
Save settles the runtime and atomically replaces a JSON document containing the
state and position. Open reads that explicit document; there is no automatic
JSONL recovery or recording sidecar. Unsaved state is process-lifetime state.

The [storage composition](../examples/schematic/app/src/services/queries.rs)
constructs the public in-memory adapter with the application catalog and caller
budgets. Managed state, plugin registry, file operations and the stream recording
consumer share that manager. [Storage stories](../examples/schematic/app/tests/storage/main.rs)
cover retained scans, conditional outcomes and resumable application uploads.
A product needing process-restart durability supplies its own primary and recovery
backends through `StorageBackends`; the example carries no disk backend.

## Application File Contracts

[files.proto](../examples/schematic/proto/schematic/v1/files.proto) declares
`schematic.v1.Files.Read` and `Hash`;
[uploads.proto](../examples/schematic/proto/schematic/v1/uploads.proto) declares
`schematic.v1.Uploads.Create`, `Get`, `Resume`, `Append`, `Publish` and `Delete`.
These are application contracts over private native storage operations. Shared
`harmos.v1.File` and `Upload` carry content references and fenced production
progress; the application owns request shaping, access decisions and policy.
`Uploads.Get` without an upload selects the replay window and storage chunk size.
Mutations carry that issued window plus a request key. `Resume` fences the old
writer, and `Publish` checks length and SHA-256 before returning an immutable file.
`Append` and `Resume` report their accepted prefix without claiming it is durable;
`Get` reads the upload's current accepted and durable lengths.
The application uploads a managed bundle through this resource and passes its
file reference to client `Plugins.Create`. The library has no storage wire service.
The existing transfer `Put`/`Take` and streaming `Upload`/`Download` operations
carry the example's benchmark batches and retain their separate application meaning.

## Transfer Benchmarks

The guest's `benchmark` feature enables unary record, sample and file-byte
`Put`/`Take` operations in [guest transfer handlers](../examples/schematic/guest/src/transfer.rs).
Transfer IDs isolate concurrent datasets. Its `Hash` bridge calls the application's `schematic.v1.Files/Hash` contract and
verifies a local-file range and digest before timing. The application owns the wire
request and delegates execution to the library's private in-process hash operation.

`mise run build -- --artifact bench` builds and stamps the guest with `benchmark`.
The native gRPC plugin is a Product artifact; its framed transport benchmarks are removed.
The host benchmarks transfer 10,000 records, 30 MiB of file bytes and 1,000,000
samples in both directions, running the datasets concurrently on one reused
host/guest instance. Setup and verification are outside timing.
The transfer store is in memory; library storage benchmarks separately measure manager and memory-adapter costs.

The CLI writes a structured `schematic-walkthrough-<pid>.json` document and analysis result
when `STKR_TEST_REPORT_DIR` is set. System and packaged journeys inspect this data alongside
the existing traces. Normal CLI output remains readable walkthrough prose.
