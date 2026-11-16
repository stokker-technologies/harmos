# Operation Boundary Design

## Context

See proposal.md for motivation and the locked owner decisions. This design records how they
land and what they cost. Paths are relative to the repository root; cite symbols, not lines.

Current surface (landed, verified 2026-11-16):

- `harmos-runtime` publicly re-exports all of `harmos_storage` (`src/storage/mod.rs`), the
  domain crate and the operations crate; `Runtime` and service contexts expose state, record,
  stream, operation and plugin managers. Runtime `StorageManager` and `OperationManager`
  implement `Deref` (`src/storage/manager.rs`, `src/operations/manager/mod.rs`).
- `harmos-operations/src/storage/native/requests.rs` re-exports the engine `QueryOutput` and
  returns `Vec<QueryOutput>` in a public response.
- Storage native operations are already Private and in-process (docs/architecture.md, Native
  Type Ownership). Host operations under `proto/harmos/host/v1` carry the `callers` method
  option and are served on the listener; `Builder::composition(&str)` narrows them.
- The plugin SDK host context (`sdk/rust/src/plugin/host.rs`, and its Python, C# and C++
  equivalents) calls `harmos.host.v1` Time Get/Sleep, `plugins.Settings` List, Invocations
  Get/Cancel/Watch and Operations List/Get through child requests on the plugin carrier.
- Statistics kernels are split across `harmos-query/src/{accumulate,quantiles,order_stats,
  time_buckets}.rs`, `statistics/summary.rs`, `transform/{aggregate,histogram}.rs` and a
  benchmark-only percentile helper in `harmos-storage/src/bench_support/durable_lag.rs`.
- No storage operation deletes objects or scopes, mutates several objects and records in one
  commit, or runs without the completed runtime's `ExecutionContext`
  (`harmos-operations/src/storage/native/registration.rs`).

The architecture contract requires quoting a touched invariant and its purpose. This change
replaces "The product-owned `protocol/composition.toml` is the single declarative composition
input for both runtime assembly and SDK generation" (Product Composition and Narrowing). Its
purpose, keeping plugins from acquiring client administration and keeping SDK surfaces equal
to the deployed exposure, is preserved by product wrappers declaring their own audience.

## Goals / Non-Goals

**Goals:**

- A product depends on `harmos-runtime` (host facade) and `harmos` (plugin and client SDK)
  only; Rust privacy enforces the facade and a manifest check enforces the dependency fence.
- Every storage need a product has is one storage operation; no product keeps a parallel
  persistence or statistics path.
- Nothing leaves the process unless a product declared it, except the plugin protocol.

**Non-Goals:**

- No generic execution, case, cycle or seal policy enters Harmos; compound mutation and
  scoped deletion are generic storage verbs.
- No exact-integer RMS, standard deviation or radix percentile machinery.
- No glob or allow/deny exposure filters, no static exposure projection exporter.
- Operation visibility (Private, Protected, Public) is not removed or renamed. "Visibility
  removal" in the task list means removing Rust `pub` escape paths only.
- No wrapper crate around `harmos-runtime`; the existing crate becomes the facade.

## Decisions

### D1. Storage Operation Gaps Close First (Proposed)

Order from the research, unchanged by the owner: compound atomic mutation; object and scoped
deletion; bootstrap/offline storage operations; expression transforms; statistics
consolidation; bounded mergeable bucket summaries. They are additive and land before any
breaking group so products can adopt them on an otherwise unchanged surface.

- **Compound mutation:** one native operation carrying an ordered list of object create, CAS
  set, delete and record append items, admitted under one mutation identity, committed in one
  backend commit, producing one cut and one replayable receipt. Any failed condition refuses
  the whole request without effects. Backends declare the capability; the reference and
  in-memory backends implement it. Alternative rejected: sequential operations with
  compensation, which cannot preserve atomic publication.
- **Deletion:** typed object deletion (tombstone revision, CAS-guarded) and scoped deletion
  that removes every object, record, sample, projection row and file under one opaque scope
  prefix, reclaiming files after retained cuts release them. Retained cuts keep reading their
  data until released. Alternative rejected: history pruning, which is retention, not logical
  deletion.
- **Bootstrap/offline:** the builder can produce a storage-only runtime that admits storage
  operations without listeners, plugins or services, for prerequisites (root admission,
  credential journals) and offline maintenance. Maintenance operations copy a consistent
  backend snapshot to a target and dispose of backend data through backend hooks, refusing
  with a capability error when the backend lacks them. Alternative rejected: products calling
  backends directly before start, which is the bypass this change removes.

### D2. One Statistics Home (Proposed)

Keep `harmos-domain` `Aggregate` and query plan types; consolidate kernels under
`harmos-query::statistics` around `Summary`, shared `Quantiles`, mergeable summaries,
box summaries and bucket summaries. Transform modules delegate to these kernels; the
benchmark helper uses them too. Exposure is only through `storage.list_rows` transforms.

- Multi-percentile requests share one spool and rank selection per field
  (`Quantiles::values` already does this; aggregate execution adopts it).
- A box-summary transform returns quartiles, Tukey whiskers and a bounded, deterministic
  outlier selection.
- Mergeable summaries carry count, sum, extrema with time and position, first and last with
  time and position, and moments, so ordered first/last becomes an aggregate measure.
- Expression transforms add `Scale`, `Offset` and `Contains` with existing null semantics.
- A bounded bucket-summary transform keeps a constant number of buckets by doubling width
  and merging adjacent pairs, preserving first/last/min/max points, count, mean and moments,
  and exposes merge of a stored summary with new rows. Products persist the result through
  object or file operations; Harmos does not store it. The owner decided on 2026-11-16 that
  the product keeps its stored envelope, so this kernel is required.
- Alternatives rejected: a public `statistics` namespace (facility-trim removed it), and a
  Harmos-owned envelope store (the owner kept the envelope in the product).

### D3. Idiomatic Facade (Proposed)

`harmos-runtime` exports exactly: builder and initialization (`Application`, `Setup`,
`Lifecycle`, builder registration, catalogs, operation factories and definitions, settings,
startup errors, shutdown handle); operation calling (an opaque operation caller, invocation,
context, scope, request/response DTOs, cancellation, results, failures, retry policies,
upload/download/watch sessions); backend authoring (`StorageBackend`, `StateBackend`,
`StorageBackends`, their futures, errors and complete DTO closure, object and index
catalogs, payload and schema codecs) through one authoring module. `harmos` keeps plugin
authoring and the client connection.

```rust
let runtime = harmos_runtime::Runtime::builder()
    .storage(backends, catalog, storage_settings)
    .settings(settings)
    .catalog(product::catalog())
    .start()
    .await?;
let caller = runtime.operations();
let reply = caller.call(storage::GetObject { .. }, &invocation).await?;
```

- Managers are crate-internal owners; plugin activation, clock binding and sample start/stop
  that products did through managers become in-process host operations.
- Operation results use operation-owned DTOs (an Arrow IPC batch payload plus schema) instead
  of `QueryOutput`.
- Cross-crate internals stay `pub` only inside unexposed implementation crates; products may
  not depend on `harmos-domain`, `harmos-query`, `harmos-storage`, `harmos-operations` or
  `harmos-protocol` directly. A repository check rejects those dependencies in examples.
- Alternative rejected: making everything `pub(crate)`, which breaks Harmos's own
  cross-crate use (native handlers and plugin persistence use the engine manager).

### D4. Host Operations Private by Default (Proposed, Owner Direction)

Remove the `callers` option from every RPC under `proto/harmos/host/v1`. Harmos host
operations are registered in-process only, have no gRPC binding and are absent from wire
discovery. `Builder::composition`, the `[libraries]` and `[callers]` composition input and
the generator `--composition` option are deleted.

**Wrapper audience (proposed default):** the `callers` method option stays in
`proto/harmos/model/v1/callers.proto` as a product authoring contract. A product wrapper
service declares CLIENTS, PLUGINS or ALL on its own methods; runtime admission and generators
read it exactly as today. This keeps invariant 1's purpose (a plugin cannot acquire client
administration) without any Harmos-side narrowing. Alternative: a builder-side audience per
registered wrapper, rejected because generators could no longer derive projections from
descriptors alone.

**Consequences assessed:**

| Area | Consequence |
| --- | --- |
| Client SDKs (Rust, Python, C#, C++) | Generated `Client` resources over Harmos host operations (Access, Invocations, Operations, Plugins, Schemas, Status, Streams, Time) are deleted. The SDKs keep connection, credentials, call options, failures, transfer helpers and the generators that project a product's descriptors into typed client and plugin resources. |
| Plugin-to-host calls | Proposed default: every call the plugin SDK context makes today independent of product (time get/sleep, own settings list, Get/Cancel/Watch of the plugin's own invocations, admitted discovery List/Get) becomes a plugin protocol host facility served on the carrier, versioned with `harmos.plugin.v1`, not a host operation. `ChildDemand`/`ChildCancel` control child calls on the same carrier, but whether they make the invocation facilities redundant is not established; none is removed in this change. Removing any of them later requires establishing its purpose first and an owner decision. |
| Requirement calls | Unchanged on the wire: child requests to Public plugin operations under declared requirements and fingerprints. A child request naming a Harmos host operation is refused as unknown; a product wrapper with PLUGINS or ALL is reachable. |
| Discovery gap | gRPC plugin origins carry `scope: None` (`src/operations/caller.rs`), so List/Get can reveal entries child dispatch refuses. Not moot: the plugin discovery facility and product wrappers forwarding plugin callers both use in-process discovery, which must apply the dispatch predicate. |
| Host messages | Harmos host request/response messages stay exported from the SDK roots as payload values, so product wrappers reuse them without declaring twins; only the Harmos services lose their wire binding. |
| Conformance | Wrong-caller rules move to a fixture product in `tools/conformance` and `sdk/tests/fixtures` that wraps the operations it exposes. New rules: a Harmos host operation is unreachable over gRPC and over a child request; a wrapper is reachable for its declared audience only. Plugin protocol rules are unchanged. |
| What stays on the wire | `harmos.plugin.v1` Instance Get/Invoke/Samples/Stop, windows, sample batches, child frames, the new plugin host facilities, plugin-declared operations with Protected/Public visibility and their client gRPC projections, and product-declared wrappers. |
| Schematic example | Declares wrappers for the host operations its client uses. |

Alternatives rejected: runtime glob filters with allow/deny sets (research proposal), caller
narrowing kept beside private defaults, and products wrapping time and settings for plugins
(that would bind product-neutral plugins to product operation identities).

### D5. Registration Verbs (Proposed)

`runtime-registration` keeps register/unregister, but on the builder and running runtime,
which route to the internal owning manager. See the delta spec.

## Risks / Trade-offs

- [Breaking every product at once] → Breaking groups land last, each with its own Harmos
  revision; products pin a breaking revision only in the task that removes their usage.
- [Compound mutation on backends without transactions] → capability error, never a weaker
  sequential emulation.
- [Scoped deletion racing retained cuts] → deletion is logical at its cut; reclamation waits
  for cut release, preserving read-at-cut semantics.
- [Plugin host facilities widen the plugin protocol] → limited to the calls the plugin SDK
  context already makes; any further plugin need is a product wrapper.
- [Losing SDK client convenience] → generators keep producing typed resources from the
  product's descriptors; only the Harmos-owned resource roots go.

## Migration Plan

1. Additive groups (D1, D2) land and are published as one usable revision; products adopt
   them while their surface is unchanged.
2. Facade group (D3) lands; Harmos's own consumers (Schematic, stories, fixtures, docs)
   migrate in the same group.
3. Exposure group (D4) lands with SDK, conformance and generator changes.
4. Products pin each breaking revision in the task that removes their use of the removed
   surface. Development storage policy permits clean breaks; there are no migrations or
   compatibility shims. Rollback is the previous pin.

## Open Questions

1. **Embedded products and operations (owner assessment running, 2026-11-16).** Should a
   product that embeds Harmos as a Rust library call Harmos through a public Rust API rather
   than through operations? The tasks stay as written; the answer may later change D3 and D4.
2. **Language SDK justification (owner assessment running, 2026-11-16).** If Harmos is a
   building block of an application, do the Python, C# and C++ SDKs remain justified, or do
   products own their language SDKs? The tasks stay as written.
3. **Wrapper audience and plugin host facilities.** D4 records proposed defaults (keep the
   `callers` option for product wrappers; time, own settings, own invocation Get/Cancel/Watch
   and admitted discovery as plugin protocol facilities). The tasks implement them; the owner
   may override before group 10 starts. Whether the invocation and discovery facilities are
   redundant with child frames and admission is unestablished; removing them is a separate
   owner decision after their purpose is established.
