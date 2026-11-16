# Operation Boundary

## Why

Products reach Harmos through six crates, public manager fields, two manager `Deref`
implementations, whole-crate re-exports and engine types inside operation results. A product
can therefore bypass storage operations and re-implement statistics beside the query engine,
and every Harmos host operation is served on the wire unless a product narrows it. The owner
locked one boundary on 2026-11-16: products build a runtime and call operations; everything
else is internal, and nothing a product does not wrap leaves the process.

## What Changes

Owner decisions locked on 2026-11-16. Status per row: **proposed** until its tasks land;
**landed** only where stated.

| Decision | Direction |
| --- | --- |
| Storage only through operations | A product registers its storage backend and calls storage operations for every other persistence need. Harmos closes the operation gaps that force direct access. |
| One statistics home | All aggregation and statistics kernels live in `harmos-query::statistics` and are reachable only through `storage.list_rows` transforms. Floating-point RMS and standard deviation are accepted; no exact-integer statistics machinery is added. |
| Idiomatic public surface | Public embedding APIs are runtime building, typed operation calls and backend authoring; carrier helpers stay internal to product SDKs. No manager getters, manager `Deref` or engine types in results. |
| Typed in-process facade | Products call Harmos in-process through a typed Rust facade over native operations (`runtime.for_call(&ctx).storage().append_samples(..)`, `.plugins().load(..)`). Each facade method dispatches its operation with its registered policy; native in-process calls pass typed values without encoding; built-ins that use `call_protobuf` get native declarations. No managers or engine types are exposed. |
| SDK scope (later owner amendment: plugin-boundary) | Harmos keeps four-language carrier helpers, shared generation, supported guest ABI and generic conformance. Products own contracts, the only author-facing SDKs and starters; no Harmos type leaks or import aliases. Host-client roots disappear; connection plumbing stays. |
| Wrapper and plugin protocol defaults | Product wrappers declare their audience with the `callers` option; plugin-to-host calls the plugin SDK makes become plugin protocol facilities; wrappers reuse Harmos request and response messages. |
| Private host operations | **BREAKING.** Every Harmos host operation is in-process only and carries no gRPC exposure marker. A product exposes one by declaring its own wrapper operation that calls it in-process. Caller narrowing (`composition`) is deleted; glob exposure filters are rejected. |
| Plugin protocol stays on the wire | The plugin carrier (instance, invocation, windows, samples, child calls), plugin-declared operations and their Private/Protected/Public visibility remain unchanged. |

- Add storage operations for compound atomic mutation, object and scoped deletion, and
  bootstrap/offline maintenance without a fully started runtime.
- Add query transforms for scale, offset and contains; consolidate shared multi-percentile,
  box summaries and mergeable summaries; add a bounded mergeable bucket-summary transform.
- **BREAKING:** the builder takes backends, catalog and settings and owns every manager;
  `harmos-runtime` narrows to the facade; operation results use operation-owned DTOs.
- **BREAKING:** client SDK resources over Harmos host operations are deleted; product
  generators project the product's own wrappers. Plugin-to-host facilities the plugin SDK
  needs move into the plugin protocol (decided 2026-11-16, see design.md D4).

Precedent, landed: "Storage remains Private and in-process. Products own typed file, upload,
query, record and object wire operations over those native calls" (docs/architecture.md,
Native Type Ownership). This change applies the same rule to every host operation.

`plugin-boundary` further moves installation, registry, bundles, persisted settings
and management operations/projections to products. The internal plugin manager
retains live lifecycle/routing; products register kinds with admission/settings
hooks and explicitly call load through the typed facade, without `PluginBackend`.
Generic payload reuse stays internal to product author SDKs; product management
messages are product-owned. Regenerate once with that change and `plugin-lifecycle`.

Supersedes: the caller narrowing contract in docs/architecture.md (Product Composition and
Narrowing), the `storage-interface` statement that no cross-method transaction is assumed
(compound mutation is now an operation), the direct `StorageManager` construction
contract in docs/product-guide.md, and in the `operation-grammar` change: the Audience
Separation and One SDK per Language decisions as far as they serve Harmos host operations to
clients and plugins, the client-facing rows of its Complete Face Inventory and Decided
Built-In Visibility Mapping, its SDK Ownership client roots and the public manager surface in
Boundary and Manager Ownership. Its three visibility levels stay.

## Capabilities

### New Capabilities

- `host-facade`: the public Rust surface of the host runtime and its dependency fence.
- `host-operation-exposure`: private-by-default host operations, product wrappers and the
  plugin protocol facilities that stay on the wire.
- `storage-operation-coverage`: compound mutation, deletion and bootstrap/offline storage
  operations.
- `query-statistics`: expression transforms and the single statistics kernel home.

### Modified Capabilities

- `runtime-registration`: registration verbs live on the runtime and builder; managers are
  internal owners, not public registration surfaces.

## Impact

`crates/harmos-runtime` (typed facade, builder, operation dispatch), `crates/harmos-operations`
(storage native operations, result DTOs, host catalog), `crates/harmos-storage` (compound
mutation, deletion, maintenance hooks), `crates/harmos-query` (statistics, transforms),
`crates/harmos-domain` (query plan contracts), `proto/harmos/{host,plugin,model}/v1`,
all four SDKs and their generators, conformance, the Schematic example, runtime and storage
stories, `docs/architecture.md`, `docs/product-guide.md` and `docs/storage.md`.
Products adopt each landed revision in their own change; development storage policy allows
clean breaks without migrations.
