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

## Locked Source Inventory

Inventory locked before implementation on 2026-11-16 at `0b26cb89`.
Public declarations and their transitive reexports in each listed module belong to
that owner; the migration removes escape paths together with all their consumers.

| Owner | Source Inventory | Consumers |
| --- | --- | --- |
| Runtime | `src/lib.rs`, `prelude.rs`, `runtime.rs`, `runtime/**`, `services/**`, public module barrels; whole-crate `domain`, `builtins` and `storage` reexports; public state/record/stream/operation fields; storage and operation manager `Deref` | Schematic app/CLI, runtime stories and benches, conformance host |
| Operations | `src/lib.rs`, `execution/**`, `operations/context.rs`, `storage/native/requests.rs`, `storage/native/{pages,lists,views,snapshot}.rs`; `QueryOutput`, Arrow pages, column/index/projection contracts in public responses | Runtime registration/dispatch, product handlers, runtime storage stories |
| Storage | `src/lib.rs`, `backend.rs`, `state/**`, `manager/**`, `query/**`, `memory/**`; backend DTO closure, catalogs, codecs, manager constructors/readers | Operations, runtime storage/plugin/state owners, storage stories/benches |
| Query | `src/lib.rs`, `accumulate.rs`, `aggregate.rs`, `quantiles.rs`, `order_stats.rs`, `time_buckets.rs`, `statistics/**`, `transform/{aggregate,histogram}.rs`; public readers/tables/Arrow/index values | Storage execution and column scans, operation results, storage benchmark percentile helper |
| Domain | `src/lib.rs`, `model/storage/**`, `schema/**`, `state/**`, `streams/**`; query plans, payload and storage values, codecs and definitions | Query, storage, operations, runtime, derives |
| Rust SDK | `sdk/rust/src/{lib,connection}.rs`, `client/**`, `plugin/{host,context}.rs`, generated resource roots and private protocol barrels | Product clients, native/Wasm plugins, SDK fixtures, conformance, Schematic |
| Other SDKs | `sdk/{python,csharp,cpp}/`; host resources and plugin context resources, generated protocol messages | SDK fixture/example consumers, common conformance |
| Host Exposure | All 33 method options in `proto/harmos/host/v1/{access/*,plugins,plugins/settings,invocations,operations,schemas,status,status/*,streams,time}.proto`; runtime gRPC catalog/listener dispatch | Direct/encoded/child calls, discovery, all four SDKs, conformance |
| Composition | Runtime `runtime.rs`, `runtime/launch.rs`, operations `execution/setup.rs`; Python `build/{resource_roots,python_protocol,cpp}.py`, `csharp.py`, `ci/tasks.py`, `source/protocol.py`; Schematic protocol/codegen and app composition, SDK client fixture composition | Generator caller tests, runtime application-face stories, SDK client-root fixtures |
| Statistics Duplicate | `crates/harmos-storage/src/bench_support/durable_lag.rs` percentile helper | Storage durable-lag benchmark |

Surprises: there is no SQLite reference backend in this repository or conformance
host. The only bundled primary implementation is `harmos-storage::memory::Backend`;
reference-backend changes cannot target a nonexistent owner. The compound primitive
will be a backend-authoring contract with capability refusal and an in-memory
implementation; a product implements its durable transaction against that contract.
Memory module declarations also contain redundant `#[path]` overrides; remove them
when touching the backend. Objects initially had no scope field. Scoped deletion adds optional opaque object scope metadata, matching records and files, rather than assigning prefix semantics to object keys. The earlier prefix inventory assumption is superseded.

Remaining inventory findings: runtime `storage/manager.rs` still embeds consumer
admission and batch defaults, and the runtime/engine manager wrappers duplicate public
access; tasks 9.1 and 9.6 own their replacement. Existing test entry points still share
support through `#[path]`, covered by `plugin-lifecycle` task 6.2. Separate workspace
lockfiles resolve differing dependency revisions, and SDK lint preparation inherits
`VIRTUAL_ENV` and relies on uv ignoring the mismatched environment. Lockfile and SDK
environment cleanup remain outside the native facade slice.

Owner amendments override D3's generic caller example: the public calling shape is
`runtime.for_call(&ctx).storage().append_samples(..)` and
`.plugins().load(PluginSource::Process(..))`, dispatching native operations under
registered policy. Built-ins currently using `call_protobuf` gain native declarations.
The four plugin SDKs, generation, Wasm guest support and conformance remain; only
Harmos host-client resource roots disappear. Revisions belong in tasks.md after
completed groups, overriding its original instruction to omit them.

## Goals / Non-Goals

**Goals:**

- A product depends on `harmos-runtime` (host facade) and `harmos` (internal carrier helpers)
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
  that removes every object, record, sample, projection row and file under one exact opaque
  scope, reclaiming files after retained cuts release them. Retained cuts keep reading their
  data until released. Alternative rejected: history pruning, which is retention, not logical
  deletion.
- **Bootstrap/offline:** the builder can produce a storage-only runtime that admits storage
  operations without listeners, plugins or services, for prerequisites (root admission,
  credential journals) and offline maintenance. Maintenance operations copy a consistent
  backend snapshot to a target and dispose of backend data through backend hooks, refusing
  with a capability error when the backend lacks them. Alternative rejected: products calling
  backends directly before start, which is the bypass this change removes.

G1/G3 amendments (2026-11-17): backend-owned retained handles survive session and
native-registry restart; durable backends persist them across process restart.
`RetainSnapshot` qualifies ownership by one exact scope and retains only that
scope's source data. Scope deletion releases those owners atomically, without
revoking independent reader leases. `CreateSnapshot` persists independent
ownership without broadening the captured image; `DeleteSnapshot` releases either handle. Backend conformance includes
a proof that starts before backend close and completes after reopening.

Durable followers scan committed backend images so compound, conditional and
columnar sample writes cannot disappear from the tail. Bounded pages keep their previous complete cursor
until progress reaches a complete mutation frontier, including a whole columnar
append; resume can replay positions.
Applied conditional writes emit revisions; conflicts only advance progress.
Object/scope deletion conservatively terminates crossing watches with resync,
as do expired cuts or lost history. Scoped snapshots cannot publish global
projection outputs. The product seal and deletion policy remain product-owned.

### D2. One Statistics Home (Implemented in Groups 6–7)

Keep `harmos-domain` `Aggregate` and query plan types; kernels live under the private
`harmos-query::statistics` module around `Summary`, shared `Quantiles`, mergeable summaries,
box summaries and bucket summaries. Transform modules delegate to these kernels; the
benchmark helper uses them too. Exposure is only through `storage.list_rows` transforms.

The normal library surface has no statistics namespace. The existing `testing`
feature exports the numeric spool for integration and benchmark evidence; production
consumers use transforms. Statistics checkpoint pins and proof belong in tasks.md.

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
startup errors, shutdown handle); operation calling through the typed facade of D6
(call context, scope, request/response DTOs, cancellation, results, failures, retry
policies, upload/download/watch sessions); backend authoring (`StorageBackend`, `StateBackend`,
`StorageBackends`, their futures, errors and complete DTO closure, object and index
catalogs, payload and schema codecs) through one authoring module. `harmos` keeps
internal carrier helpers and reusable connection plumbing; products own author APIs
under the later `plugin-boundary` owner amendment.

```rust
let runtime = harmos_runtime::Runtime::new()
    .storage(backends, catalog, storage_settings)
    .settings(settings)
    .register(product::catalog())
    .start()
    .await?;
let host = runtime.for_call(&ctx);
let object = host.storage().get_object(GetObject { .. }).await?;
```

`Runtime::new()` returns the unstarted runtime builder; `register` and `start` follow the
host verbs of the `plugin-lifecycle` change.

- Managers are crate-internal owners; plugin activation, clock binding and sample start/stop
  that products did through managers become in-process host operations.
- Operation result ownership is implemented in task 9.3: `RowOutput` carries ordered IPC
  frames, and `IpcBatch` carries column/projection schema and batch bytes. Selectors, index
  and projection definitions, pages and statuses are operation-owned DTOs; engine seek
  cursors stay behind continuation tokens. The remaining facade items are proposed.
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

**Wrapper audience (decided by the owner, 2026-11-16):** the `callers` method option stays in
`proto/harmos/model/v1/callers.proto` as a product authoring contract. A product wrapper
service declares CLIENTS, PLUGINS or ALL on its own methods; runtime admission and generators
read it exactly as today. This keeps invariant 1's purpose (a plugin cannot acquire client
administration) without any Harmos-side narrowing. Alternative: a builder-side audience per
registered wrapper, rejected because generators could no longer derive projections from
descriptors alone.

**Consequences assessed:**

| Area | Consequence |
| --- | --- |
| Client SDKs (Rust, Python, C#, C++) | Decided by the owner on 2026-11-16 (D7): the Harmos host-client resource roots and host RPC client surfaces (generated `Client` resources over Access, Invocations, Operations, Plugins, Schemas, Status, Streams, Time) are deleted once every host operation is in-process only. Reusable connection plumbing stays: connection, credentials, call options, failures, transfer helpers and the generators that project a product's descriptors into typed client and plugin resources. |
| Plugin-to-host calls | Decided by the owner on 2026-11-16: every call the plugin SDK context makes today independent of product (time get/sleep, own settings list, Get/Cancel/Watch of the plugin's own invocations, admitted discovery List/Get) becomes a plugin protocol host facility served on the carrier, versioned with `harmos.plugin.v1`, not a host operation. `ChildDemand`/`ChildCancel` control child calls on the same carrier, but whether they make the invocation facilities redundant is not established; none is removed in this change. Removing any of them later requires establishing its purpose first and an owner decision. |
| Requirement calls | Unchanged on the wire: child requests to Public plugin operations under declared requirements and fingerprints. A child request naming a Harmos host operation is refused as unknown; a product wrapper with PLUGINS or ALL is reachable. |
| Discovery gap | gRPC plugin origins carry `scope: None` (`src/operations/caller.rs`), so List/Get can reveal entries child dispatch refuses. Not moot: the plugin discovery facility and product wrappers forwarding plugin callers both use in-process discovery, which must apply the dispatch predicate. |
| Host messages | Decided by the owner on 2026-11-16: Harmos host request/response messages stay exported from the SDK roots as payload values, so product wrappers reuse them without declaring twins; only the Harmos services lose their wire binding. |
| Conformance | Wrong-caller rules move to a fixture product in `tools/conformance` and `sdk/tests/fixtures` that wraps the operations it exposes. New rules: a Harmos host operation is unreachable over gRPC and over a child request; a wrapper is reachable for its declared audience only. Plugin protocol rules are unchanged. |
| What stays on the wire | `harmos.plugin.v1` Instance Get/Invoke/Samples and the plugin lifecycle calls, windows, sample batches, child frames, the new plugin host facilities, plugin-declared operations with Protected/Public visibility and their client gRPC projections, and product-declared wrappers. |
| Schematic example | Declares wrappers for the host operations its client uses. |

**Later owner amendment (`plugin-boundary`, 2026-11-16):** the rows above
describe generic capabilities, not author-facing Harmos types. Helpers implement
time, own-invocation and admitted discovery facilities behind the minimal carrier's
host-call frames; own settings come from the product kind's resolver. Product SDKs
expose those capabilities through product-owned APIs. No capability is silently
removed. Plugin installation/registry/settings management and their messages move
to products; they are excluded from the generic host-message reuse row. Remaining
Harmos payload reuse is internal and cannot leak types to plugin authors. Remove
superseded management services rather than retaining an in-process install API.

Alternatives rejected: runtime glob filters with allow/deny sets (research proposal), caller
narrowing kept beside private defaults, and products wrapping time and settings for plugins
(that would bind product-neutral plugins to product operation identities).

### D5. Registration Verbs (Proposed)

`runtime-registration` keeps register/unregister, but on the builder and running runtime,
which route to the internal owning manager. See the delta spec.

### D6. Typed In-Process Facade (Decided by the Owner, 2026-11-16)

Products that embed Harmos call it in-process through a typed Rust facade over native
operations, not through raw operation calls and not through managers.

```rust
let host = runtime.for_call(&ctx);                       // binds caller, scope, cancellation
let receipt = host.storage().append_samples(AppendSamples { mutation, scope, batch }).await?;
let plugin = host.plugins().load(Load {
    kind, placement: Managed { command }, settings,
}).await?;
```

- Each facade method dispatches exactly one native operation with that operation's registered
  policy; a one-call override travels on the call context. Admission, cancellation, retries,
  tracing and receipts are those of the operation, so the facade adds no second policy
  authority or scheduler.
- Native in-process dispatch passes typed values; there is no encoding round trip.
  Built-ins that currently dispatch through `call_protobuf` (an encode/decode path even
  in-process) receive native declarations so the facade never encodes.
- The facade exposes capability groups (`storage()`, `plugins()`, `streams()`, `invocations()`,
  `time()` and the like, one per host operation family) and call handles; no manager, engine,
  query or reader type and no scheduler is reachable through it.
- Product wire handlers authorize product actions, map product values, then call the same
  facade, so product wrappers (D4) and product-internal calls share one path.
- Alternatives rejected: typed APIs beside operations with their own policy (two policy
  authorities), and products calling operations by untyped identity (no compile-time contract).

### D7. Carrier Helpers and Product SDKs (Owner Amendment, 2026-11-16)

`plugin-boundary` supersedes the earlier library-owned author SDK assignment.
Harmos retains thin Rust/Python/C#/C++ carrier helpers, cancellation/windows,
supported Wasm guest ABI, shared descriptor/codegen tooling and generic conformance.
Products own plugin contracts, the only author-facing SDKs and starters, and hide
Harmos types, attributes, errors and escape hatches without import aliases.
Products supply registrars and translate public values internally; they do not
copy carrier engines. Consolidate the consuming product's richer renderers/fixes
upstream and regenerate once with this change and `plugin-lifecycle`.

Harmos host-client roots and host RPC clients are deleted when host operations
become in-process only. Reusable connection plumbing stays. Generic host payload
reuse is internal to product SDKs where necessary; author APIs expose product
values. Plugin administration, installed registry, bundles and persisted settings
move to products, so they have product operations rather than generic wrappers.

The internal plugin manager remains. Products register kinds with admission and
settings-resolution hooks, explicitly select placements and call `load`, `bind`
and `start_samples` through D6's facade with opaque handles. No `PluginBackend`
trait. Admission mechanics stay in Harmos; policy and startup selection stay in
the product. See `plugin-boundary` for the ownership table and bounded inspection.

## Risks / Trade-Offs

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
- [Facade dispatch overhead on hot paths] → batching is preserved (one operation per batch,
  never per sample) and native dispatch does not encode; costs are reported from existing
  benchmarks, budgets unchanged.

## Migration Plan

1. Additive groups (D1, D2) land and are published as one usable revision; products adopt
   them while their surface is unchanged.
2. Facade group (D3) lands; Harmos's own consumers (Schematic, stories, fixtures, docs)
   migrate in the same group.
3. Exposure group (D4) lands with SDK, conformance and generator changes.
4. Products pin each breaking revision in the task that removes their use of the removed
   surface. Development storage policy permits clean breaks; there are no migrations or
   compatibility shims. Rollback is the previous pin.

## Deletion Implementation Decisions

Group 3 gives objects optional explicit opaque scope metadata, replacing the inventory's
prefix assumption. Scoped deletion uses the existing atomic mutation receipt and accepted
work queue; the queue fences new file creation through caller cancellation, avoiding a
second lock or scheduler. Current object ownership removes all earlier revisions of that
identity so another owner cannot resurface. Retained snapshots share an earlier image
when another reader opens the same cut; file bytes and disposable old projection outputs
are reclaimed after the last earlier cut releases. Physical reclamation is backend-owned.

The public backend conformance module now proves both compound mutations and scoped
deletions; products add their own reopen and physical reclamation proofs. No durable
product backend or plugin installation/registry/bundle implementation moved into Harmos.

## Storage-Only Lifecycle Decision

The owner selected drain and reopen for group 4. The minimal assembly opens the same
registered backends under a separate manager and worker, admits native storage operations
without listeners, plugins or application services, then drains and closes. Full assembly
reopens those backend handles under a new manager and worker. No live-session retention or
worker ownership-handoff contract is introduced. A borrowing session prevents the builder
from continuing assembly while minimal operations are in use.

Primary-backend maintenance takes a retained source cursor and an empty offline target
backend. The backend owns its copy format and supported target kinds. The reference memory
backend copies committed data and captured control metadata, retaining history identity;
it refuses nonempty or active targets. Disposal explicitly removes an offline primary
backend's data and refuses live sessions or retained images. These operations do not copy
or dispose the separate recovery journal; that remains owned by its recovery backend.

## Host Wire Cutover Findings

Host protobuf declaration validation read service metadata from the payload's pool.
Private host declarations now validate their protocol-owned payload graph, private
visibility and absence of a gRPC binding without a host service descriptor. Plugin facilities validate their own protocol services.
Product wrappers reusing a library payload can supply `ProtobufOperation::service()`
so their audience comes from their product descriptor rather than the payload owner.
The reference product fixture owns its wrapper routes and generated roots; Harmos
retains generic transport helpers and the internal payloads used by that fixture.
