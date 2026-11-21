# gRPC Plugins Design

## Nightly Repair Decisions

The Schematic example uses the existing managed registry placement, including its
verified bundle and immutable content path, so the runtime both spawns and stops
the sidecar. Delete the example's manual launcher and child-exit wait. Keep
registry-backed settings, the caller's directory and walkthrough output; retain
every existing operation deadline and stop grace. A command bundle passes the
directory as an explicit sidecar argument instead of a launcher environment variable.
The removed launcher was the application's only direct Tokio consumer. Remove
that normal dependency and retain the existing test-only Tokio declaration.

Delete the obsolete segmented-recovery benchmark and its registration. Durable
compaction superseded its full-journal assumption; the native
`tests/storage/state/snapshots/segmented_snapshot_matches_full_replay.rs` test
remains the correctness proof.

All focused Rust selections, including exact package/target selections, use the
existing Nextest runner and its native hang guards. Delete the direct Cargo test
dispatch branch. Remove the unused RecoverySnapshot fixture export. Compile-only
`--no-run` selections preserve prior JUnit evidence and omit the execution-only
`--no-tests` option that Nextest ignores during compilation. An executing selection still
removes its stale report before running so old evidence cannot satisfy new proof.

Run `lint:rust` once before repairs and once at final handoff. This explicitly
extends the earlier one-run budget and authorizes the lint task's declared
compiler selections. Repair findings at their source; allow an exception only
for a demonstrated lint false positive, with its exact reason recorded here.
Native Linux proof remains outstanding. Documentation under `docs/` is owner-written.

The initial lint run found only native observer callback/count reads absent on
WebAssembly. Guest metadata explicitly rejects observers in
`sdk/rust/src/wasm/registration.rs`; the shared authoring values remain valid API
input for that rejection. These two fields receive WebAssembly-only dead-code
exceptions with reasons; native warnings and every other warning remain enabled.
This is a platform lint false positive, not permission to add guest observer support.

The requested `--changed` test selector and `check:rust:supply` task do not exist
in this checkout: the maintained selector is `--paths`, and dependency analysis
is `check:rust`. Workspace-qualified feature lists cannot be forwarded wholesale
to the focused runner's separate package invocations. The workspace test-target
compile therefore uses the existing `build.cargo focused test` entry with
`--workspace --no-run` and the lint task's declared feature selections; no alias
or second test executor is added. All three tracked Cargo locks are refreshed
through native offline metadata resolution, retaining their resolved versions.

## Durable Plugin State

The plugin host owns one opaque, revisioned value per persisted plugin entry.
This is outstanding-effect recovery state, independent of application undo and
settings. No durable `plugin.state` API exists in the starting source: context
resources are process-local; the registry persists entries and settings only.
The existing StateManager writer records library-owned system facts and folds
them into a library-owned map alongside `A::State`. Recovery replays those facts;
the existing complete snapshot includes the map. Thus `State = ()` receives the
same facility without application transaction or record declarations. No second
journal, storage queue or persistence backend is introduced.

```rust,ignore
// Host Contract (Calling Identity Comes From the Carrier)
ctx.host().plugins.state.get().await?; // { value: bytes, revision: u64 }
ctx.host().plugins.state.put(expected_revision, value).await?; // new revision
// Rust Accessor (Prost, Like Existing Settings Wire Values)
let state = ctx.state();
let saved = state.get::<OutstandingEffects>().await?;
state.put(&effects).await?;
```

Revision zero means no value has been written. Empty bytes clear the opaque value
while advancing its revision, preventing ABA conflicts. Put compares the expected
revision inside the single journal writer and returns `state_revision_conflict`
on mismatch. The SDK handle retains the revision from Get or Put; callers must Get
before updating an existing value. A clone shares the same revision observation.
Put completes only after the recovery journal has been flushed durably. Missing
managed recovery storage refuses rather than acknowledging memory-only durability.
A plugin records intent before an external mutation and clears it only after undo.
Initialize may read state through the same admitted host calls.

The messages live in `harmos.host.v1.plugins`; the plugin carrier exposes
`harmos.plugin.v1.plugins.State/{Get,Put}` through the existing facility adapter.
The manager's `PluginStateSettings.max_bytes` defaults to one MiB. A stable
entry key survives replacement and restart; deleting and recreating the same name
issues a fresh key. Removal durably retires that key, and startup reconciliation
retires orphaned keys after an interrupted deletion. The registry remains the
single entry owner; it stores only that identity, not a second copy of the value.

The plugin state operation owns a configurable maximum opaque value size with a
library default, as explicitly requested for this feature; callers supply no
literal bounds. This task decision overrides the general consumer-owned budget
rule for this one setting. The managed registry supplies entry identity and
removal; state survives disable, replacement and plugin/host restart, and is
retired with its entry. Get and Put accept no selector: the trusted calling entry
is their sole authority. Direct attachments without a persisted entry refuse.

The architecture already permits covered-entry deletion after snapshot publication
(`docs/project/architecture.md:912`), but StateBackend had no explicit retirement step or
manager invocation. Recovery compaction is now an explicit backend contract: after publishing and flushing a
complete snapshot, retain that newest complete snapshot and only journal entries
after its covered recovery position. The storage manager invokes the explicit
backend compaction step after successful durable snapshot publication. Compaction
must be idempotent, preserve the uncovered suffix, and leave recovery readable if
interrupted. A product implements that trait method in its existing StateBackend;
there is no product-side plugin persistence facility to add.

A compacted snapshot is required recovery input: discarding it or disabling its
read policy cannot promise full-journal replay. The former test that discards the
newest snapshot then replays covered entries is superseded. Segmented equivalence
and payload-boundary fixtures preserve independent replay input before compaction;
their state, undo, receipt, tail and segmentation assertions remain unchanged.

The existing registry's object persistence remains entry/settings ownership, not
an alternative plugin-state store. The task explicitly extends the retained host
service family despite the broader product-policy transfer still pending under
`plugin-boundary`; it does not implement that unrelated migration. Generated
protocol bindings remain tracked. Documentation changes are reported to the owner.

## Existing Overlaps Requiring Owner Direction

- `crates/harmos-runtime/src/state/capture.rs` has Callback and Managed journal
  capture. `Builder::require_capture` serves callback consumers;
  `Builder::managed_storage` serves StateBackend consumers. Both acknowledge
  persisted journal groups. Prefer retiring Callback after its consumers migrate
  to StateBackend; this delivery retains both pending owner direction.
- `crates/harmos-runtime/src/plugin/grpc/mod.rs` and
  `crates/harmos-operations/src/execution/operations/access.rs` both define
  CallingPlugin. Runtime invocation/publication uses the connection-bearing type;
  built-in Settings and State use the name-only projection installed by
  `operations/authored.rs` and `operations/manager/plugin_grpc.rs`. Prefer one
  operations-owned trusted identity and delete the parallel name-only type.
- `crates/harmos-runtime/tests/support/snapshot_store.rs` Store and
  `crates/harmos-storage/tests/support/fixture.rs` Fixture both compose in-memory
  primary/recovery backends. Snapshot tests/benches use Store; storage tests use
  Fixture. Prefer a shared Fixture with snapshot access, retiring Store.
- `sdk/cpp/cmake/Protocol.cmake` lists protocol inputs manually, while
  `crates/harmos-protocol/codegen/mod.rs` recursively discovers the same protocol
  tree. The C++ build and Rust generator consume these separate inventories.
  Prefer generating the C++ inventory from the authoritative descriptors and
  deleting its hand-maintained list.
- `examples/schematic/app/src/services/{sidecar,guest}.rs` each registers the
  same seven host analysis/transfer operations and progress publisher. Each
  backend's start entry calls its copy. Prefer one host registration owner and
  delete both backend-local copies after owner direction; this repair preserves them.
- `crates/harmos-runtime/benches/runtime/main.rs` runs the writer workloads once
  in `in_place_writer_against_deleted_clone_and_publish_path` for a printed
  comparison and again in the Criterion measurements. `benchmarks` calls both.
  Prefer deleting the manual comparison/report pass and retaining Criterion;
  the snapshot-benchmark deletion does not authorize that separate removal.

The remaining `#[path]` declarations in
`crates/harmos-runtime/benches/runtime/main.rs` and
`crates/harmos-runtime/tests/support/storage_fixture.rs` cross Rust harness/crate
roots to reuse existing fixture source. Normal module resolution cannot reach
those files; replacing them requires the fixture-ownership decision above, not
another copy or an include-path workaround. No new override is introduced.

`tools/python/tests/fixtures/shared_sdk_inventory.json` is a hand-frozen generated
output regression snapshot, not an independently authored SDK call inventory.
`test_shared_generator_preserves_contracts.py` compares fresh output against its
fixed hashes alongside semantic checks for nested values, presence, audiences and
call options. No generation task produces the snapshot. The owner authorized
refreshing only changes caused by the State and collection additions. Rendering
the snapshot's original descriptor revision reproduces every recorded hash;
rendering the merged descriptors changes ten existing entries solely for State
Get/Put bindings and response exports. Collection messages add no entries to this
product fixture. The exact comparison and semantic checks remain in place.

## Product Integration Repair Contract

The scoped integration repair adds
`Runtime::bind_grpc(&Activation, scope, Clock) -> ScopedBinding` for an already
admitted activation. The lease owns scoped calls and clock propagation on the same
connection. Binding opens no Samples demand: `start_samples(streams)` opens the scope's
Samples selections for exactly the requested streams and `stop_samples()` settles them while calls stay bound; close
settles calls, captured ingestion and producer cleanup. Shared admitted writers retain separate wire-sequence
validation and scope-stamped rows. The runtime clock still owns policy deadlines.
See `docs/project/architecture.md#scoped-plugin-bindings` for the complete usage contract.

Source audit confirms the runtime clock replacement, attachment-time selection,
Rust oneof/layout overestimate, transfer-ceiling admission and encoded-status
rewriting claims. Python, C# and C++ do not use the Rust memory estimator. No
library generator emits a path-based Rust input dispatch match; the empty-match
lint finding belongs to a product's generator and needs no library source change.
The settlement Send repair named in the attached notes is already in the base.
Task group 12 records this completed repair inventory; final proof is retained in 8.1–8.2.

## Current Delivery Contract

The retained source is implemented; final native Linux/Windows proof stays in tasks
8.1–8.2. Those tasks cover remote disconnect, managed process settlement, integration
repairs, refusal propagation, SDK packaging and generation drift in one combined
candidate. Release rehearsal and published assets remain `release-assets-layout`
2.1; generated product-client proof remains its 2.3.

The historical migration inventories below describe completed slices. Their old
branch names, host-client roots, composition narrowing, two-level visibility,
plugin authoring/registry ownership and lifecycle names are superseded by the
current proposal and `operation-boundary`, `plugin-boundary`, `plugin-lifecycle`.
The only public host surface is build/start, operation calls, backend authoring and
plugin authoring through the product-owned contract; managers remain internal.
Generated output stays tracked. `proto/harmos/` is the sole maintained protocol
source; the obsolete copied draft under this change is removed.

## Crate Structure

The owner-locked target is six library crates plus one derive crate. This is the target,
not a claim that the current manifests already implement it:

| Package / Path | Target Ownership |
| --- | --- |
| `harmos` at `sdk/rust` | Public plugin and generated-contract client entry point; no host code |
| `harmos-protocol` at `crates/harmos-protocol` | Authoritative generated protobuf messages/services, descriptors and light helpers |
| `harmos-domain` at `crates/harmos-domain` | Reduced native schema, identity, state/recovery and storage foundation |
| `harmos-storage` at `crates/harmos-storage` | Generic persistence, query kernels, capture and backends |
| `harmos-operations` at `crates/harmos-operations` | Built-in declarations and handlers, dispatch, call context and policy validation |
| `harmos-runtime` at `crates/harmos-runtime` | Public host entry point, host composition/engine, clock, listener activation and guest sandbox |
| `harmos-sdk-derive` at `sdk/rust/derive` | The single procedural-macro crate, retaining only field-reading derives |

Arrows mean normal product-layer dependencies; direct downward edges are allowed, reverse
edges are forbidden. Transport libraries and derive/build support are outside this product graph:

```text
harmos-runtime [public host]             harmos [public plugin/client]
  |                                       |
  v                                       |
harmos-operations                         |
  |                                       |
  v                                       |
harmos-storage                           |
  |                                       |
  v                                       |
harmos-domain                            |
  |                                       |
  v                                       v
harmos-protocol <-------------------------+
```

The SDK's only product dependency is protocol; it never depends on domain, storage, operations
or runtime. Native plugin authors compile SDK, protocol and transport/derive support. Clients
will compile generated application contracts, protocol and the SDK client half. Host embedders
use runtime explicitly and compile its downward closure. Component authors will compile the
SDK's optional Wasm adapter and generated WIT bindings without host engine dependencies.
Examples, guest fixtures and `sdk/{python,csharp,cpp}` remain outside the Rust core graph.

### Reduced Domain Foundation

Unlike the consuming application's layout, a generic library benefits from a small native
foundation that lightweight application model crates can use without the storage stack
(Arrow, filesystem and async support) or the host engine. Removing domain is possible only by
moving its contracts into heavier owners; it is not the locked design. Storage already depends
on domain schemas, catalogs and journal `Position` through
`crates/harmos-storage/src/{objects.rs,schema_encoding.rs,state/reader.rs}`. Operations depends
on storage, and runtime on operations/storage. Moving shared contracts unchanged into operations
or runtime would introduce reverse edges and cycles.

The inventory below is relative to `crates/harmos-domain/src/`. Split mixed modules by semantics;
retention of a native contract does not retain its legacy transport representation.

| Current Modules | Required Destination and Treatment |
| --- | --- |
| `schema.rs`, `schema/`, `message/` | Keep stored-schema encoding, canonical revisions, validation and allocation accounting in domain; protobuf descriptors do not replace stored encoding |
| `identity.rs`, `metadata.rs`, `metadata/`, `state/`, `journal.rs` | Keep native identities, typed records/transactions, preparation, restoration, inverse pairing and journal positions; remove separable legacy plugin-name helpers |
| `model/storage/`, including `query/` | Keep schema/object catalogs, revisions, history identity, mutation receipts, files, query selections and transforms; replace transport mirrors with protocol types |
| `operations/`, `model/operations/`, `model/services/` | Move operation metadata, visibility and transport descriptions to protocol; typed dispatch, call context, native streaming and policy validation to operations; runtime supplies narrow resources |
| `inventory.rs`, `inventory/`, `plugin/`, `artifacts.rs`, `registration.rs` | Replace portable declarations with protocol; delete legacy plugin inventories, grants and markers; retain guest-required mechanisms only until their replacement in 7.1 |
| `listeners/`, `observers.rs`, `streams/` | Move portable declarations to protocol and listener activation/host behaviour to runtime; keep persisted row identity in the native foundation |
| `aggregate.rs`, `value*`, `presentation.rs`, `catalog.rs`, `config.rs`, `failure.rs`, `refusal.rs`, `error.rs`, `answer.rs`, `lib.rs`, `model/mod.rs` | Split remaining native values/codecs, catalog machinery, validation and failure mirrors by the owners above; retain only foundation semantics, not whole mixed modules |

Domain reduction follows declaration replacement, so obsolete catalog machinery is judged after
its users migrate. Preserve stored schema/revision compatibility and recovery/inverse behaviour;
delete superseded wire models and conversions rather than add another adapter layer. Further
storage contract reshuffling waits for `storage-interface` to settle snapshots, durability,
replay and capability semantics; this reduction preserves required storage behaviour.

### Derives and Guest Consolidation

After authoritative host protobuf adoption, implement `authoring-without-macros` in its agreed
order: inventory and alias handling, listeners, stream lists, lanes, records, stream rows,
operations, catalogs, record registries, messages, then transactions/state registries. The
`domain_catalog`, `runtime_catalog`, `domain_operation` and `runtime_operation` aliases are live;
migrate them with their catalog/operation families before deleting their exports. All catalog,
operation, lanes, listener, stream, record and transaction declarations become values and plain
functions. Request/message, stored-schema and row derives may read fields; they do not register
operations or catalogs.

Merge `harmos-macros` into `sdk/rust/derive` after those replacements, keeping only field-reading
derives. Preserve the distinct semantics of stored-data `Described` and the protobuf Message
derive; replace the old `message` attribute without conflating native stored encoding with wire
encoding. Migrate re-exports, dependencies, consumers and derive coverage, then delete the old
macro package. Keep the SDK free of host dependencies throughout.

After domain reduction, task 7.1 folds `harmos-guest` into optional WebAssembly support
in `harmos` and deletes both guest packages under the lifecycle design below.
The guest `plugin` macro, re-exported as `harmos_guest::guest`, becomes builder values and one
author-supplied registration function behind an SDK adapter. Generated component bindings stay
inside that adapter: one registration function does not mean one canonical ABI export.
Preserve WIT resources, async lifting, registration/operation glue and lifecycle/child controls
(submit, status, join and release) until their replacements are designed and migrated. Move the
sandbox fixture and guest tests with the adapter. Runtime retains guest loading, fuel/memory
limits and sandbox authority; neither these nor host dispatch enter the SDK.

### Guest Lifecycle and Child Control

Task 7.1 replaces the guest transport and authoring surface as one delivery. The SDK's
only product dependency remains `harmos-protocol`; runtime owns loading, Wasmtime stores,
fuel, memory, admission, generation capabilities and physical settlement.

```rust
use harmos::{Context, Operation, Plugin, Result};
use harmos::wasm::Registration;

#[unsafe(no_mangle)]
pub fn harmos_register() -> Registration {
    Registration::new(Plugin::new("example", "1.0.0")
        .operation(Operation::new("example.echo", echo)))
        .capacity(2)
        .on_initialize(initialize)
        .on_shutdown(shutdown)
}

async fn echo(context: &Context, request: Echo) -> Result<Echo> { Ok(request) }
async fn initialize(context: &Context, configuration: Vec<u8>) -> Result<()> { Ok(()) }
async fn shutdown(context: &Context) -> Result<()> { Ok(()) }

// Requirements, protobuf messages, handlers and invocation resources use the native SDK.
// A submitted child remains owned by the original host attempt.
async fn children(context: &Context, request: EchoChild) -> Result<()> {
    let child = context.submit(request).await?;
    let state = child.status().await?;
    child.request_cancel().await?;
    let result = child.join().await;
    Ok(())
}
```

The component SDK supplies canonical ABI exports, generated WIT bindings, async lifting,
resource destructors and `register`/`operate` glue. The author supplies only the ordinary
`harmos_register` function; no declaration or export macro is required. Registration
constructs builder values and describes protobuf contracts, never enters an initialization,
operation or cleanup callback. Runtime freezes and validates that catalog before activation;
there are no compiled catalog markers or parallel authored inventories. Componentization executes
`register` without authority and stamps its protobuf registration in one component custom section.
Bundle reading and `Host::load` read that bounded metadata without running a guest; activation
executes `register` on the retained store and rejects any difference from the stamped metadata.
This keeps loading synchronous and bundle reading independent of the optional sandbox engine.

| Mechanism | Replacement and Reason |
| --- | --- |
| Guest plugin attribute, declaration traits and static custom-section markers | Remove; the author-supplied function composes the native SDK's `Plugin` and `Operation` values, avoiding duplicate authoring contracts. |
| WIT register and operate exports, child import and async resources | Keep inside the optional SDK Wasm adapter; these are component ABI requirements rather than author declarations. |
| Guest registration records and JSON inventory | Replace with protobuf guest registration carrying the authoritative `harmos.v1.Catalog`, identity, version and lifecycle policy; one source describes every handler. |
| Prepare | Keep as a host-issued phase: validate unary payload, operation fingerprint and original generation, then reserve one slot before running author code. |
| Initialize invocation | Keep as the binding phase of the original prepared invocation: attach its unforgeable finite attempt token and context values without running the handler. |
| Attempt | Keep as the execution phase: run the selected unary handler once per host attempt, with host cancellation observable through `Context::cancelled`. |
| Finalize and Cancel | Keep: Finalize releases the original slot after its work settles; Cancel signals that slot without discarding the handler's cooperative final response. |
| Plugin initialization and shutdown operations | Replace payloads with protobuf lifecycle requests; run explicit initialize/shutdown callbacks using runtime-issued lifecycle authority, retaining cleanup after partial failure. |
| Lifecycle grants and retained initialization context | Keep solely as runtime authority: only the initialization callback receives a retainable generation context, which can make declared calls/publications while its activation is live and cannot submit attempt-owned children. Ordinary invocation contexts never carry that fallback authority. |
| Resource-scoped child grants | Replace authored guest grants with `PluginSettings::grants: Vec<harmos_operations::plugin::Grant>` supplied by the runtime consumer; only declared requirements can receive a scope, keeping authorization out of SDK metadata. |
| Child call and submit | Replace with protobuf child requests containing the attempt/generation capability, requirement identity, fingerprint, context values and optional policy override; runtime checks declaration, original activation, visibility and admission. |
| Child status, cancel, join, release and detach | Replace with protobuf child controls; all require the exact submitting attempt capability and an instance owned by that attempt. Status/cancel return protobuf instance state; Join consumes the handle and awaits settlement; Release cancels and relinquishes the handle while the parent retains settlement; Detach leaves work owned by the parent until its settlement. |
| Child handle drop | Keep as asynchronous Release, matching native ownership: a dropped handle cannot orphan accepted work. |
| Guest-private Postcard envelope, lifecycle and child status codecs | Delete; command, answer, assignment, lifecycle, child and failure messages use protobuf `harmos.v1` shapes, with no compatibility decoding. |
| Guest-only host built-in payload mirrors | Delete; use the authoritative host/storage protobuf requests and runtime-native handlers. |
| Streaming guest operations and their transfer endpoints | Remove from guest registration and transport; plugins expose unary operations and publish declared streams independently. Native host streaming remains. |
| Guest observers and listener macro projections | Remove unsupported observer glue and superseded declaration projections; retain publication/recovery behavior through the authoritative stream declarations. |
| Guest dispatch table and host execution dependencies in the guest kit | Replace with the SDK's unary handlers and a small adapter-owned prepared-slot table; sandbox and host dispatch never enter the SDK. |
| Sandbox fixture and guest stories | Move with the adapter, consolidate into one journey per file and retain authority, lifecycle, cancellation, recovery and sandbox assertions; do not increase the source test count. |

An attempt token grants only the current finite invocation's child and control operations.
A generation token grants only declared calls/publications during that activation. Neither a
numeric child id, an operation name nor another plugin's token grants authority. Retirement
revokes generation calls and new publication before shutdown; accepted children still settle.
Fuel, deadlines and memory limits are unchanged. Registration caches its descriptor work and
initialization avoids reconstructing the catalog. Development optimization remains a consumer
profile choice only where a retained host operation dependency still needs it.

Source inventory: `sdk/rust/{src,tests,wit,Cargo.toml}`, both old guest packages (deleted),
runtime guest loading/envelopes and their operation consumers, guest-only operation declarations,
protocol guest messages and generation inputs, Schematic guest and shared consumers, guest
fixture/build/package/conformance selectors, workspace manifests/locks, README and guest guides.
Storage-interface files, backends, query modules and storage benches remain separately owned.

### Tool Packages

Move `harmos-conformance` from `crates/harmos-conformance` to `tools/conformance` as a tool
package, preserving its `harmos-conformance` and `harmos-conformance-plugin` binaries, command interfaces
and release interface. It is executable cross-language proof, not a core library; an internal
library target used by Rust SDK tests may remain an implementation detail of the tool package.
Its protocol driver and reference plugin retain their protocol-only product dependency. Preserve all 19 protocol cases, managed and
remote driving, reporting and the Rust dev-dependency integration. Update non-Rust launchers,
fixtures, tasks, docs and planned archive paths with `release-assets-layout`; the current
example-host bundle is not evidence that a reference-host archive has landed.

Split `harmos-packaging`: Cargo build orchestration, cancellation/watch support, componentization,
inspection, archive construction and proto/Python generation move to a tool package under
`tools/packaging`. Runtime keeps only bundle reading needed to load artifacts. Remove normal
runtime dependencies on build/bundle/generation tooling; keep generation in build/tool callers.
Migrate manifests, build scripts, tasks, launchers, fixtures and documentation in the same move.
Conformance relocation and packaging separation are low risk and may happen at any time;
neither changes the required host-protobuf → authoring/derives → domain → guests order.

### Host Operations and Client Contracts

`proto/` owns all language-neutral contracts. `harmos-protocol` holds generated messages,
services and descriptors with light constructors, operators, `Display` and conversions for
Harmos's own code. Behaviour needed by another language is specified in words beside the proto
contract and implemented by each SDK; Rust helpers are not the cross-language specification.

Every host operation lives in `harmos-operations`, declaration and handler in one operation
file, taking and returning protocol messages. Existing built-ins under
`src/{host,lifecycle,storage,streams}/` already colocate implementations: retain that ownership.
Handlers access narrow state, lifecycle, publication and storage resources supplied by runtime;
operations never depends on runtime. Keep deadlines and retries with the Rust callee and allow
call-site overrides for one call. Remove the operations `runtime` feature after all
contract-only consumers use protocol/SDK; do not move handlers into runtime to keep that surface.

Extend the client-facing gRPC declaration/codec integration before the consuming product's
Harmos-adoption wave. Generated protobuf request/result types plus authoritative descriptors
must register directly, retaining decoding-memory admission, fingerprints, method paths,
cancellation, backpressure and unary/client-stream/server-stream/bidirectional envelopes.
The affected seam is `crates/harmos-runtime/src/grpc/{codec,routing,native,input,envelope}.rs`,
operation registration and `crates/harmos-operations/src/execution/operations/serving/protobuf.rs`.
A declaration owns its gRPC mapping; the host exposes that declaration without a per-service
adapter or a duplicate native request/response model.

### Product Consumer Layout and Sequence

```text
sdk/rust/                         product: plugin authoring + client half
crates/product-protocol/            generated product messages + light helpers
crates/product-storage/             application persistence (after gRPC waves)
crates/product-operations/          declarations + handlers + gRPC mappings
crates/product-runtime/             execution manager, state, transactions, cycles,
                                  effects, plugin entries, listener and main.rs
crates/product-cli/                 commands; hosts through runtime's library API
crates/product-gui/src-tauri/        desktop host composition
crates/product-tracing/             small development subscriber/export support

CLI / desktop -> runtime -> operations -> storage -> protocol
product [sdk/rust] -> protocol; client crate becomes its client half
```

`use product::prelude::*;` supplies authoring and client APIs with no host compilation. There is
no product domain crate: definitions are protocol messages; admission validation, parameter
expansion and expression evaluation live with the operations applying them; persisted shapes
live in storage. Keep transactions, state and execution/cycle transitions together in runtime.
No separate presentation extraction is required by this target.

In a product's host wave (wave 3), move handlers area by area from
`crates/product-runtime/src/runtime/operations/`, `domains/` and
`crates/product-server/src/grpc/` into operations and register directly. Delete each area's
`hosted!` and `hosting::{Host,Serve}` trampoline as it moves. Logs and trace replay, support
bundles, transfers/upload commit and abandonment, paging/frontier closure, watches/heartbeats,
plot deltas/reduction and redaction remain real operation behaviour. Retain their bounds,
cleanup, snapshot and resume semantics; transport registration does not implement them.
Remote access becomes listener configuration plus the one client authorization hook, preserving
pairing/key persistence, certificate rotation and shutdown. Runtime owns reusable startup and
listener composition, with `main.rs` as its thin process entry, so CLI hosting calls the runtime
library rather than depending on a binary. No server crate or per-service adapters remain.

In the last product waves (CLI wave 4, removal wave 5), redirect CLI and desktop hosting to
runtime, remove the server crate, dead conversions and generation dependencies, dissolve
domain into the owners above, and remove the operations `runtime` feature when no contract-only
consumer remains. Retain `product-tracing` as small development support. Update manifests,
build scripts, examples and user guidance with these boundaries.

Only after the last wave, Harmos `storage-interface` supplies the five generic extensions;
A product's separate change of the same name then merges
`crates/product-runtime/src/storage/` and `src/runtime/storage/` into `product-storage` through
the Harmos storage interface. Break reverse dependencies on runtime/query/plot first; runtime
retains transaction and cycle orchestration, passing owned inputs and narrow resources.
Execution/step statistics, history filters/tokens, finalization/seals, report capture, artifact
inventory and fresh-root refusal remain product behaviour built on those generic facilities.
Logs, support bundles, exports/report spools, plugin installation, desktop root lease/backup,
client pairing/download files, CLI and build tooling stay with their owners. Extraction covers
application persistence, not all disk I/O. Both changes require before/after benchmarks,
including deterministic multi-day data, restart, late samples and finalization.

## Protocol

The complete definition is `proto/harmos/plugin/v1/instance.proto`, with its compatibility rules in
`proto/README.md`; the landed behavioural contract is `proto/harmos/v1/behaviour.md`.
The change-local `proto/` copy must stay aligned until it is retired; tooling checker repair belongs to `tooling-streamline` 4.3. In outline:

```proto
service Plugin {
  rpc Describe(DescribeRequest) returns (Description);                 // identity, start time, catalog
  rpc Invoke(stream HostFrame) returns (stream PluginFrame);           // one operation; child calls ride back
  rpc Publish(stream PublishFrame) returns (stream Samples);           // one stream; the host sends ticks
  rpc Stop(StopRequest) returns (StopResponse);                        // release before the host closes
}
```

`Failure` keeps today's content (classification, code, message, cause, operation context) and adds
the action a user can take. Built-in operations use the same package.
`OperationPolicy.backoff` is a protobuf Duration carrying the retry delay.
The checked-in protocol and generated bindings, not the change-local draft, define the implemented field numbers.

Fingerprints use the data-only descriptor projection in `proto/harmos/v1/behaviour.md`:
file options, source locations and field JSON names are excluded, including for well-known imports.
Reachable message/enum metadata is retained; the complete ordered catalog has its own digest.
All SDKs and host operations share this rule. Rust shares `harmos-protocol` clock descriptors;
conformance covers schema graphs, setting-based observers and both built-in clock fingerprints.

## Placement

The host obtains a channel in one of two ways. Everything above the channel is identical.

| | Managed | Remote |
| --- | --- | --- |
| Process | The host starts, supervises and stops it | Someone else runs it |
| Address | The loopback address the plugin announces at start | A URL the product supplies |
| Who dials | Host | Host |
| Executable hash, kill on shutdown | Yes | No |
| Startup failure | Spawn, exit, announce or connect failure | Connect failure |

Catalog admission does not depend on placement. The remote placement is a new physical backend
behind the existing attach interface.

A managed plugin is any executable that serves `harmos.v1`. It binds a loopback port of its own
choosing and announces the bound address by calling the host (Managed Launch Announcement
below). The host therefore never has to find a free port. Nothing else about the executable
matters, so a plugin in any language with a build output can be managed.

## Managed Launch Announcement

Decision (owner, locked): the managed-launch readiness line on standard output is replaced by
the plugin calling the host. This is a clean replacement: no output parser, no fallback and no
`HARMOS_ADDRESS` (neither its managed-mode detection nor its dial override). Explicit bind
address APIs stay; a managed run binds `127.0.0.1:0`.

```proto
// proto/harmos/plugin/v1/launches.proto
service Launches {
  rpc Announce(LaunchesAnnounceRequest) returns (LaunchesAnnounceResponse);
}
message LaunchesAnnounceRequest {
  string token = 1;   // HARMOS_LAUNCH_TOKEN of this launch attempt
  string address = 2; // the bound loopback listener, for example "127.0.0.1:53124"
}
message LaunchesAnnounceResponse {}
```

The host starts every managed process with two variables, after the placement's own
environment so a placement cannot replace them:

| Variable | Value |
| --- | --- |
| `HARMOS_HOST_ENDPOINT` | The bootstrap listener as a bare socket address, `127.0.0.1:PORT`; plain HTTP/2 gRPC |
| `HARMOS_LAUNCH_TOKEN` | 64 lowercase hexadecimal characters: 256 random bits for this launch attempt |

Sequence: bind the plugin listener, start serving `Instance`, then call `Launches/Announce`
once. The host may call `Instance.Get` before the Announce reply arrives, so a plugin must
never wait for that reply before serving. Announcement means the plugin's listener is ready;
admission to Ready (Get, catalog admission, initialize, publications) stays the separate
existing sequence. The reply is produced without waiting for the host to dial or admit, so
there is no circular wait. A plugin started with credentials announces in either lifecycle;
`--remote`/explicit-address lifecycles only change whether it keeps serving after Stop.
A run without credentials serves silently (an author running the plugin by hand); a run
with only one of the two variables fails before serving. A managed launch that never
announces fails on the host through the existing load deadline. The plugin's standard
output carries no protocol.

Token rules:

- Generated from the operating system's cryptographic source per launch attempt; the host
  keeps only its SHA-256 digest, bound to the entry and its reserved activation attempt.
- Consumed atomically by the first authenticated Announce; never logged or persisted.
- Revoked when its attempt ends for any reason: cancellation, child exit, replacement,
  runtime stop, failure, or the existing effective `plugins.load` deadline. No new deadline
  or constant exists; the load policy owns the startup bound and the token lives exactly as
  long as the load's work.

Bootstrap listener: owned by the runtime, bound lazily on loopback before the first child
is spawned, serving only `Launches/Announce`, independent of any client listener a product
configures, and released when the runtime stops. Tools that start plugins without a runtime
(the conformance host, packaging inspection, test harnesses) reuse the same public
`harmos_runtime::plugin::grpc::Launches` owner instead of a second mechanism.

This narrowly amends the contract that plugin facilities have no independent listener
routes: exactly one bootstrap route exists, loopback only, token-authenticated, not a host
operation, never declared in an operation catalog, and absent from client listeners,
discovery and reflection. Ordinary and paired clients cannot reach it and gain no authority
from it.

Process output: standard output and standard error are both piped and drained for the
child's whole life, so neither can block the child. Lines are forwarded to the host log at
DEBUG under `harmos::plugin::stdout` and `harmos::plugin::stderr`. Until activation settles
the host also keeps a bounded tail of standard error, limited by
`GrpcPluginSettings.startup_output` (lines and bytes); it is released when activation
settles.

| Failure | Code to the plugin | Host failure and action |
| --- | --- | --- |
| Wrong, reused or retired token | `UNAUTHENTICATED`: launch credential is invalid or spent; restart through the host | none for the attempt |
| Announce after the load deadline | `UNAUTHENTICATED` (credential revoked) | the load keeps its `LoadDeadline` failure |
| Malformed, non-loopback or zero-port address | `INVALID_ARGUMENT`: announce a bound loopback socket | the attempt fails at once with the same action; the token is spent |
| Child exits before announcing | none | executable, process identifier, exit status; standard-error tail in `Failure.diagnostics`; inspect the startup diagnostics, then restart |
| Announced address unreachable | none | endpoint and transport cause; start serving before announcing, then restart |
| Load deadline before announcing | none | `LoadDeadline`; process, kill status and standard-error tail in `Failure.diagnostics` |

A failed load revokes the token and terminates and reaps the child.

Failure reporting: a plugin entry entering a failed state writes exactly one host log record
(WARN for transient, refused, cancelled or timed-out classifications, ERROR otherwise) with
entry, activation, code, message and action; an unchanged failure is not logged again. An
operation request completing with a failure writes exactly one record at the operation
layer's single completion point with operation, code and message and never payload contents;
sample rows travel inside window answers and never produce per-sample records.

## Endpoint Security

A plugin endpoint is plain gRPC: no transport encryption and no credentials, for both
placements. A server written by hand from the `.proto` files in any language is a valid
plugin. The network is trusted, as it is for clients.

## Connection and Reconnect

- Every connect calls `Describe`, admits the catalog and creates a new activation. The
  connection is the generation: a closed connection cannot deliver calls, so generation tokens
  and stale-call fencing are removed. A monotonic activation counter remains for status.
- Each plugin connects independently. There is no dependency graph, start order or reload plan
  across plugins.
- A lost connection fails the calls in flight. Their operation policy decides retries.
- The host reconnects without user action. Harmos exposes `started_at` of the previous and the
  current activation; the product decides what a changed value means.
- Samples produced while disconnected are lost. There is no request deduplication or watch replay.
  Client survival across transient drops is deferred until those host contracts are separately specified;
  reconnecting a plugin does not establish resumable client requests or subscriptions.
- Before the host shuts down or removes a managed plugin it calls `Stop` on its owned
  child after settling work and finalizing the activation. The plugin runs its process
  cleanup hooks and replies; the host closes the connection, awaits exit and reaps the child.
  Only a child owned by `crates/harmos-runtime/src/plugin/grpc/managed.rs` receives Stop,
  including startup failure and replacement rollback. Launch credentials announce an
  address and never establish process ownership.
- For an externally started plugin, removal, disable, attachment replacement, startup
  failure and failed reconnect settle work and finalize attempted initialization, then
  close only the host connection. The plugin observes ended Samples/Invoke streams and
  cancellation or transport failure for outstanding work, not Instance.Stop. Its listener
  and run entry stay serving, including when a developer tool supplied launch credentials;
  a product can register the same endpoint again. No SDK run entry exits on connection loss.
  The existing registry Replace operation still accepts only managed bundle entries;
  replacing a remote attachment requires detaching it and registering the desired endpoint.

## Operations

- Operations served by a plugin over `harmos.v1` are unary (one request and one response per `Invoke`); request and result streaming remains supported for operations implemented natively in the host process.
- Visibility has two levels. `Private` operations may be called by the declaring plugin and the
  host; products use it for operations the host drives. `Public` operations may be called by
  everyone connected: other plugins and the host's clients. The `Host` level is removed.
- Retries are the existing operation policy. Nothing new is added for retry on connection loss.
  On the new path an absent policy means no overall or attempt deadline, zero cancellation grace
  and no retries. `attempts` is the total attempt count; absent or at most one means no retry.
  Retrying uses the wire `backoff` as a fixed delay without jitter.
- An authored handler returns its value: `Result<T, E>`, not a result of a response of a type.
  The shared-owner `Response<T>` stays below the authoring surface for host code that returns a
  view into a larger allocation.
- A handle obtained on one connection fails when that connection closes and never follows its
  replacement.
- Plugin operations are unary: one request and one result. Streaming requests and results remain
  available only to host-native operations. Streaming child calls stay unsupported. Examples and
  fixtures with streamed plugin operations must migrate to unary calls or declared sample streams.

### Generated Host Messages

Host declarations implement `ProtobufOperation`. Request, response and item types are
Prost-generated messages implementing `prost_reflect::ReflectMessage` against their authoritative
compiled descriptor set. The declaration owns its mapping, policy, visibility and resource list.
`ProtobufInput<T>`, `ProtobufOutput<T>` and their input/output pair retain the normal host-native
stream envelopes, credits, cancellation and retry fencing. There is no request/response mirror.

```rust
use harmos_runtime::operations::{
    ExecutionContext, Grpc, OperationPolicy, ProtobufOperation, ProtobufOutput,
    Registration, ServingError, Visibility,
};
use sensor_protocol::{ReadRequest, ReadResult};

struct ReadSensor;

impl ProtobufOperation for ReadSensor {
    type Request = ReadRequest;
    type Response = ReadResult;
    type Endpoints = ProtobufOutput<ReadResult>; // Use () for unary.

    const ID: &'static str = "sensor.read";
    const GRPC: Option<Grpc> = Some(Grpc::new("sensor.v1", "Sensors", "Read"));
    const POLICY: OperationPolicy = READ_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;

    async fn run(
        context: ExecutionContext, request: ReadRequest, output: &Self::Endpoints,
    ) -> Result<ReadResult, ServingError> {
        let result = read_sensor(context, request).await?;
        output.send(result.clone()).await.map_err(ServingError::Stream)?;
        Ok(result)
    }
}

runtime.operations
    .register_batch(Registration::new(lane).protobuf::<ReadSensor>()?)
    .await?;
```

The server scans protobuf before Prost decoding, admitting a conservative bound for generated
message layouts and vector growth through the existing decoding-memory owner. Authoritative
message descriptors supply reflection, dynamic validation and canonical protobuf fingerprints;
the host adds only its operation service and streaming/control envelopes. All built-in
and example native operations now use generated declarations. Their replaced payload DTOs
and superseded guest declarations are removed. Native stored values keep their
portable schema and encoding contracts.

Assignment catalogs use `harmos.host.v1.OperationAssignments` from
`proto/harmos/v1/host.proto`, including when passed inside guest initialization. Operation
contracts advertise only `harmos.protobuf.v1`; storage codecs and schema revisions are unchanged.

Guest command/answer and child envelopes use `harmos.v1`
messages. Lifecycle payloads and assignments use generated protobuf messages.
Guest catalog construction uses SDK builders; componentization stamps one generated
`harmos.v1.GuestRegistration` custom section after inert registration. Activation
compares that metadata with registration on the original retained store.
Host encoded calls select a unique guest operation by the same `OperationRequest::ID` as native SDK plugins; an owner-qualified ID still selects a specific provider. This keeps consumers transport-independent.

Guest descriptor graphs are projected at protocol generation time, eliminating
source-location decoding and unrelated descriptor work during fuel-bounded registration. Development guest workspaces optimize `harmos` and `harmos-protocol` at level 2; unoptimized descriptor validation still exceeds the original fuel budget. Release builds need no package profile override. The host example retains its `harmos-operations` development optimization; the standalone sandbox has no operations dependency and optimizes only its SDK/protocol dependencies.

`storage.hash_file` and `host.publish` use their authoritative protobuf contracts.
Runtime converts protobuf publication rows through the same row decoder as native
gRPC publication before native storage, preserving fingerprints and provider authority.
No private guest payload mirrors or compatibility envelope decoders remain.
`harmos-domain::Message` and stored file identity/range/hash codecs retain native
storage formats. Application messages are generated into `examples/schematic/protocol/`;
guest handlers use those messages directly under `examples/schematic/guest/`.

## Calls Between Plugins

```text
plugin A                        host                          plugin B
ctx.operation(SetTemperature) ─► ChildRequest{id, payload}
                                 Public? fingerprint matches? provider connected?
                                 ──────────────────────────► Invoke{id, payload}
                                 ◄────────────────────────── Response{payload}
               ◄──────────────── ChildResponse{payload}
```

- Payloads are protobuf, the only wire encoding. The host routes bytes and never decodes them.
- Calls are typed. The caller generates code from the provider's `.proto` files in its own
  language's toolchain; there is no dynamic call by identifier and JSON.
- An SDK derives the protobuf definition from the author's request and response types and puts
  it in `Catalog.types`. Message types are limited to what protobuf expresses.
- The caller lists the operation in `Catalog.requires` with the fingerprint it was built
  against. A mismatch with the provider is reported when both are connected, naming both plugins.
- Connected native gRPC plugins trust each other. Grants and provider authorization between
  those plugins are removed; visibility, fingerprint comparison and external-client authorization
  remain. Sandboxed guests retain runtime-owned authority as specified in Guest Lifecycle and
  Child Control; they cannot grant themselves resource access.
- An operation identifier is unique among connected plugins and host operations. A plugin that
  declares an identifier already in use is refused at connect, so provider selection is removed.
- A call to an operation whose provider is not connected is refused at call time, naming the
  missing plugin.

```cpp
// C++ caller, Python or Rust provider
auto reply = ctx.operation(thermal::v1::SetTemperature{.target = 85});
```

## Streams and Observers

- Opening a `Publish` call is the demand; closing it ends the demand. Publication leases,
  admission and byte credits are removed. gRPC flow control is the backpressure.
- Order is guaranteed within a stream. Streams do not block each other or operation calls.
- An observer is a function the author registers for a stream with an interval. The host sends
  one tick per interval on the stream's call and the SDK answers each tick by calling the
  function and sending its value as one sample, stamped with the tick's time. The interval is a
  declared default or a plugin setting, which the host resolves. `interval_setting` names a
  fully qualified protobuf section field in `Catalog.sections` whose type is
  `google.protobuf.Duration`; its positive value overrides the declared interval.
  Missing, unresolved, zero or malformed values refuse admission.
- A stream without an interval is driven by the plugin, which sends a sample when its device
  produces one.
- The connection's own (unscoped) demand, which the host opens on attach, reconnect and
  replacement, samples an interval stream only while something watches it: a
  `PublishedStream::watch()` handle or a plugin listener. It starts from a fresh origin when the first watcher appears and stops when the last
  leaves. Interval sampling is granted as windows (operation-grammar design, Windowed Interval
  Batches), and a demand that cannot keep up fails fast with an error naming its stream and
  action (Overload Fails Fast there).
- Guests cannot hold a call open, so they declare no streams and no observers.
- `PlacementSettings.stream_window` is required and is a positive retained-row capacity per
  stream supplied by the consumer. Harmos supplies no implicit storage budget.

## Clock

The host owns the only clock. An SDK offers no waits of its own: `ctx.sleep` is a call to the
host, and an observer runs on the host's ticks.

- There is no tick rate. The host schedules each wait and each observer interval as a timer and
  wakes the waiter when it is due.
- One clock object per host gives two readings from one source: a monotonic instant for waits
  and deadlines, and a date for evidence, derived as a fixed epoch plus the elapsed time. The
  runtime, storage and built-in operations use that captured clock for their time reads and waits.
  Transport scheduling and whole-test hang guards remain on real time.
- Samples and records carry host time, so evidence from plugins on different machines shares one
  time base. A device's own timestamp, when a plugin supplies one, is kept as the source's time
  and never replaces the host's.
- It costs one round trip per wait.

### Virtual Mode

In production the clock is real time. In a test the host runs a virtual clock so a story that
waits for an hour inside a plugin finishes at once, for a plugin in any language and process.

- The virtual clock has its own timer queue and moves only when its controller advances it.
  Transport runs on real time. Tokio's paused clock is not used for this: it jumps whenever the
  host is idle, and a host waiting for a plugin's answer looks idle, so an operation deadline
  would fire before the plugin replied.
- The controller advances to the next timer only when nothing is active: every call in flight
  is waiting on the clock, no tick is unanswered, and no disk, process or transport work the
  host started is outstanding. A deadline guarding active work is a timer but never a reason to
  advance.
- A call counts as waiting when its only outstanding activity is a sleep. A handler that starts
  background work of its own and then sleeps is outside what virtual time can see; the SDKs
  state this, and such a plugin simply runs in real time.
- A stream driven by a device has no interval and never counts as waiting on the clock.
- A test advances by running until a condition holds, so periodic observers cannot make the
  clock run forever. A real-time bound on the whole test stays as a hang guard, since a plugin
  that never waits would otherwise stall a frozen clock.
- It is built by lifting the simulation facility's clock into this one clock, so simulation and
  plugins share one virtual scheduler. Simulation's recovery, replay and inverse-law tests keep
  their guarantees.
- An observer function does not wait: the stream call carries no path for a child call. A
  sample that needs a wait belongs in an operation.

## Authoring

Derives describe data. Everything else is an ordinary value the author builds and passes on:
there are no attribute macros for plugins, operations, observers or catalogs.

```rust
use harmos::{Context, Observer, Operation, Plugin, Result, Stream, Visibility};

#[derive(harmos::Message)]
struct SetTemperature { target: f64 }

#[derive(harmos::Message)]
struct Temperature { celsius: f64 }

async fn set_temperature(ctx: &Context, request: SetTemperature) -> Result<()> {
    ctx.operation(ReadSensor::default()).await?;        // call to a required operation
    ctx.get::<Chamber>()?.set(request.target).await
}

async fn read_temperature(ctx: &Context) -> Result<Temperature> {
    Ok(Temperature { celsius: ctx.get::<Chamber>()?.read().await? })
}

fn main() -> Result<()> {
    let set = Operation::new("thermal.set", set_temperature).visibility(Visibility::Public);
    let temperature = Stream::<Temperature>::new("thermal.temperature");
    let observer = Observer::new(&temperature, read_temperature).every_ms(1000);

    let plugin = Plugin::new("thermal", env!("CARGO_PKG_VERSION"))
        .operation(set)
        .requires::<ReadSensor>()
        .stream(temperature)
        .observer(observer);

    plugin.serve()
}
```

- The same source builds as a sidecar or a guest when it reaches the outside world only through
  the context. `serve` runs a gRPC server on a native target and exports the same registration
  through the guest interface on Wasm.
- A mistake such as a duplicate identifier is reported when the plugin starts and builds its
  catalog, not at compile time.
- Derives remain for types whose fields must be read: messages, stream rows, settings.
- Plugin, observer and catalog attribute macros and their sidecar variants are removed.
  Macros still required by guests or operation/state authoring remain until the separate
  `authoring-without-macros` change replaces their consumers.
- Concepts shared with a product keep the Harmos name in the product's SDK.

## Language SDKs

The pilot covers Rust (`sdk/rust`), Python (`sdk/python`), C# (`sdk/csharp`) and C++
(`sdk/cpp`). Go is outside the pilot. Python publishes as `harmos-sdk` and imports as `harmos`.
The Rust SDK at `sdk/rust` is the public package and library `harmos`. Plugin and client
authors depend on it and use `harmos::prelude::*;` without compiling host code.
Its normal dependency closure contains protocol and SDK support, with no host crates.
Host APIs belong to `harmos-runtime`; a host embedder depends on that crate explicitly.
The old `crates/harmos-sdk` and `crates/harmos` facade are removed.

Every SDK, Rust included, is built over the `.proto` files: the authoring layer produces and
consumes `harmos.v1` messages, and a transport adapter carries them (a gRPC server for a
sidecar, the guest interface for a Wasm guest). The Rust SDK therefore does not reach into the
host's internal operation machinery, and all SDKs exercise the same contract.

`sdk/<language>` provides, over `harmos.v1`: serve operations, call required operations, declare
and publish streams, observers, lifecycle and settings. Each passes the conformance suite. Product
SDKs (a product's `sdk/<language>`) build on these and add their own concepts.

### Python Authoring

Python uses generated protobuf message classes and decorated async instance methods.
The [Python authoring example](../../../docs/project/tooling.md#python-plugin-sdk) shows
operation, observer and cleanup registration, typed child calls and remote serving.
The instance owns author state; `Context` owns invocation resources and host clock calls.

### C# Authoring

C# uses a typed builder registering async instance methods. The class owns its state;
requests, replies and rows are generated protobuf message classes. The generation task
checks in private wire bindings and typed client/host adapters. Transport requests are
internal; consumer messages live at the root and client/plugin boundary namespaces.

Import shared types from `Harmos` and authoring types from `Harmos.Plugins`.
The maintained [plugin example](../../../sdk/csharp/examples/Plugin/Program.cs)
shows the builder and unary handler using only those public paths.

`ServeAsync(address: "127.0.0.1:53124", remote: true)` serves an explicitly chosen remote
endpoint and remains alive after Stop. Managed mode binds a dynamic loopback port and
announces it through `Launches/Announce` with the launch credentials.
`Stream<TRow>` registers a device producer receiving a context and typed `Publisher<TRow>`;
closing demand cancels the context and ends publication. `Section` carries typed product
contributions, including product settings definitions. `Context.Scope` remains opaque.

Declare `Requires(HostOperation.TimeSleep)` or `Requires(HostOperation.TimeGet)` before
using `context.Host.Time.Sleep` or `context.Host.Time.Get`; the clock convenience methods
use the same requirements. These are child calls on the invocation, never local timers.
Observers receive the host tick in `Context.At` and the per-demand `Context.Sequence`;
they may call declared operations, watches, settings and the clock while the tick is
unanswered. Returning the row cancels outstanding children. Background work outside these host-visible waits
is outside virtual-time accounting and runs in real time. Instance state shared by
concurrent calls must use the application's normal synchronization.
### C++ Authoring

C++20 uses generated protobuf message classes and typed builder values. Handlers
return their protobuf response directly and throw `harmos::Failure` for a structured
application failure. `ctx.call` starts a child call and returns a typed future;
`ctx.operation` waits for that future. C++20 reserves `requires`, so the builder
spells the dependency method `requires_operation`.

Import shared types from `harmos` and authoring types from `harmos::plugin`.
The maintained [plugin example](../../../sdk/cpp/examples/plugin/main.cpp)
shows named imports, a typed operation and managed serving.

`serve()` selects managed lifecycle on an ephemeral loopback port and announces it
through `Launches/Announce`. `serve("127.0.0.1:53124")` selects remote lifecycle and
keeps serving after Stop; it announces only when launch credentials are present. Observer callbacks may make declared child calls and clock waits during their unanswered tick. Device producers use
`Stream<Row>::publish_on` with a typed `Publisher<Row>`, ending when demand closes.
`section(message)` carries typed settings/contributions; `on_stop(callback)` owns
cleanup. Invocation resources use `ctx.set(value)` and `ctx.get<Type>()`; shared
author state is captured explicitly by handlers. Authors synchronize shared state
across concurrent calls. Independently started background work is invisible to
virtual time.

### SDK Admission, Invocation Scope and Independent Payload Types

These additive follow-ups keep the SDK dependent only on protocol and transport support.
Generated-client admission scans bound message descriptors before Prost decoding, including repeated, map and nested allocations; managed envelopes merge borrowed wire slices without a payload accumulator.
Accounting ends at handoff: required caller-owned `ClientSettings.retained_bytes: NonZeroUsize` bounds decoding and SDK-owned queued values across client clones/calls, releasing each claim when its bare value reaches the caller; callers own memory thereafter.
Admission refuses before decoding when a claim cannot be granted and names `retained_bytes` with advice to raise it or request smaller pages; no consumer policy default is supplied.

```rust
pub struct ClientSettings {
    pub retained_bytes: NonZeroUsize,
    // Existing wire and outgoing queue bounds.
}
// Host: a scoped handle preserves the opaque identity through inherited child contexts.
let operations = runtime.operations.with_scope("opaque-scope")?;
let invocation = operations.submit_encoded("plugin.operation", payload)?;
// Plugin authors explicitly require a nonempty transport scope.
let scope = context.scope()?;
// Descriptor registration creates no operation, stream, requirement or section.
let plugin = Plugin::new("example", "1.0.0").message::<RecordPayload>();
```

`harmos_runtime::Scope::as_str()` exposes the inherited identity to native host handlers. The runtime
copies it unchanged into the existing `harmos.v1.Request.scope` for managed and remote plugins;
child payloads cannot replace this trusted local context value. Empty scope is refused by
`Context::scope()`, only when the author calls it; no operation-name list imposes
scope requirements. Python and C++ use `context.scope()`, and C# uses `context.Scope()`.
All accessors refuse an absent scope with code `scope_required` and message
"Invocation Scope Is Required: the caller must bind a scope".
Ordinary unscoped operations remain valid. Scope does not grant
operation visibility or replace generation, invocation, policy, cancellation or settlement.

```python
Plugin(author, id="example", version="1.0.0", messages=[RecordPayload])
```
```csharp
new Plugin("example", "1.0.0").Message<RecordPayload>();
```
```cpp
using harmos::plugin::Plugin;

Plugin("example", "1.0.0").message<RecordPayload>();
```

All four message-registration APIs use their existing descriptor-closure merge and validation;
repeated identical registration is idempotent. Shared conformance case 20 resolves and decodes
an independent payload type while proving no executable declaration or section references it.
The Rust SDK conformance journey lives with the executable runner in
`tools/conformance/tests/integration`, retaining its fixture and shared cases without compiling
the SDK's unrelated Wasm development dependencies for a native protocol journey.
The SDK's unused development dependency on the runner is removed with that move.
The public README's entry points and setup remain accurate; the additive shapes are specified
here and in their SDK API documentation rather than changing unaffected README claims.

## Validation

- Each SDK's conformance plugin builds from the `.proto` files alone, so the
  languages build in parallel and none waits for the host build.
- One story runs against every language: the same suite, written once per
  SDK, executed by the same host, with the same expected evidence. A
  difference between languages is a failure.
- Tests at integration level and above follow a real user journey, one story
  per file. A story reads like a test template: top to bottom, with few or no
  indentation levels, a short comment naming each stage, and every line
  either an action the user takes or a check of what they then see.
- Setup, fixtures and mechanics live in shared modules with one convention.
  A shared module never chooses a scenario or hides an assertion, and a test
  never rebuilds what a shared module provides.
- No test depends on real time. A test waits for a condition and advances the
  host's virtual clock through its controller. Transport keeps its ordinary
  scheduler, including when a plugin runs in another process. Activity accounting
  holds virtual time still while a call, tick answer or host work is in flight.

## What It Removes

| Area | Path |
| --- | --- |
| Framed codec, credits, drivers | `crates/harmos-sidecar/src/transport.rs`, `transport/` |
| Frame model | `crates/harmos-sidecar/src/wire.rs` (frames and sizing) |
| Binary carriers, control pipe | `crates/harmos-sidecar/src/kit/{connection,carrier}.rs`, `attach/{control,windows}.rs` |
| Handshake and frame exchange | `crates/harmos-runtime/src/services/sidecar/{handshake,carrier,frames,exchange}.rs` |
| Guest operation envelopes | `harmos.v1` messages with SDK dispatch and runtime authority; Postcard envelope codecs are deleted by 7.1 |
| Publication leases, admission, cadence reports | With the stream path they serve |
| Plugin graph, start and stop ordering, reload plans | `crates/harmos-runtime/src/plugin/{mod,reload}.rs` and their use in `manager.rs` |
| Grants between plugins, provider selection | Plugin paths of `crates/harmos-runtime/src/operations/manager/grants.rs`, `OperationProvider` |
| Generation tokens and stale-call fencing | `crates/harmos-runtime/src/plugin/scope.rs`, stale endpoints in `router.rs` |
| Native observer runner | `crates/harmos-sidecar/src/kit/{observer,task}.rs` |
| Native plugin, observer and catalog macros and their sidecar variants; guest/operation/state authoring macros remain for `authoring-without-macros` | `crates/harmos-macros/src/artifact/`, `catalog/typed.rs` (about 1,300 lines with their tests' share) |
| Transport tests and benches | `crates/harmos-sidecar/tests/transport/`, `benches/transport/` |
| C ABI and its C++ prototype | `crates/harmos-ffi`, the reflection bridge under `sdk/cpp`; `sdk/cpp` is rebuilt over `harmos.v1` |

Behaviour tests under `crates/harmos-runtime/tests/sidecar/` migrate to the new transport.

## Remaining Source Inventory

Task 1.2 records the native cutover inventory; its removed paths describe completed migration,
not current packages or features. The remaining inventory is `proto/`, `crates/harmos-protocol`,
`crates/harmos-domain`, `crates/harmos-storage`, `crates/harmos-operations`,
`crates/harmos-runtime`, `sdk/rust` and `sdk/rust/derive`; remaining packages to relocate or
remove are `crates/harmos-conformance`, `crates/harmos-packaging`, `crates/harmos-macros`,
`crates/harmos-guest` and `crates/harmos-guest-macros`. Target owners and removal order are in
Crate Structure and tasks 4.7, 6.5, 6.8–6.12 and 7.1. Include `sdk/{python,csharp,cpp}` launchers,
`examples/schematic`, guest sandbox, runtime/SDK fixtures, manifests/locks, build scripts,
`mise.toml`, `tools/project.toml`, `tools/python`, CI, README and product/topic guides for each
placement change. Declaration migration follows `authoring-without-macros`; story consolidation
follows `test-stories`. Update affected claims and links without rewriting unaffected guidance.

The Crate Structure section defines this batch's boundary replacements. Storage extensions and
Product storage extraction follows after the last wave as separate `storage-interface` changes.
Existing System test serial scheduling remains.
The host clock and facility trim may share storage paths only through sequential handoff and
review of the combined tree; the clock no longer needs an isolated implementation wave.

The external-client authorization hook remains. Without it Harmos falls back to anonymous
access; products may use it to require an authenticated client.

## Open Points

Guest migration (7.1) follows Guest Lifecycle and Child Control above. Windows guest, conformance and example proof is recorded in task 7.1; native Linux validation remains open under 8.1.

### Replacement Ownership

The old open-point questionnaire is superseded by [operation-boundary](../operation-boundary/tasks.md) for native-only host
operations, typed facade access, dedicated plugin facilities and deleted host-client
surfaces, and by [plugin-lifecycle](../plugin-lifecycle/tasks.md) for register/initialize/finalize, host start/stop/run,
catalog() and SDK name parity. Those changes carry the current locked direction;
older authoring examples in this design describe the original migration only.

Source in this checkout already has unary plugin operation builders, explicit policy,
interval settings, consumer placement settings and four SDK packages. This observation
is not final native Linux/Windows proof, which remains in tasks 8.1–8.2. Reconnection
must retain the existing in-flight failure/policy semantics; transparent client survival
is not established by this reconciliation. Statistics ownership is now operation-boundary
group 6, not the old downsampling-retention question.

## SDK Call-Style Source Inventory

The call-style replacement owns all four SDKs, their generators and conformance/client
fixtures, example client/plugin consumers and SDK guides. Its protocol edits mark optional
read cuts/pages and the Plugins.Watch selector. Regeneration also updates protocol output.
Minimal non-storage runtime consumers adapt that selector and the generated SDK calls;
storage/query implementations remain with their separate owner. Manifests, locks, formatter
configuration and generated-file inventories are included. Commits remain local.

Required fields flatten positionally; optional fields and retry identity use optional
arguments/options. Exactly one response field projects its payload; empty/multiple-field
responses retain their envelope and metadata. Status.Get contains both status and resolved_at.
It therefore stays wrapped under that rule pending the owner's decision about a direct
HostStatus return. Public type ownership is root/shared, client and plugin; C# names its
boundary namespaces Clients and Plugins to avoid the root Client class collision.

The optional-marker edits are:

| Protocol Input | Fields |
| --- | --- |
| `proto/harmos/client/v1/access/pairings.proto` | PairingsListRequest.at |
| `proto/harmos/client/v1/plugins.proto` | Get/List/Watch read cuts; Watch.name |
| `proto/harmos/client/v1/plugins/settings.proto` | SettingsListRequest.at |
| `proto/harmos/client/v1/status.proto` | Get/Watch read cuts |
| `proto/harmos/client/v1/status/logs.proto` | List.at/page; Watch.at |
| `proto/harmos/client/v1/status/trace_events.proto` | List.at/page; Watch.at |
| `proto/harmos/plugin/host/v1/settings.proto` | SettingsListRequest.at |

Product binding acceptance retains static consumer typing and aggregate round-trip proof;
the superseded snapshot of a public module's private wire imports is removed. Wire modules
and their request aliases now stay behind the generated private binding module.

## Managed Process Settlement

Managed startup and installation mutation share native process ownership. Readiness
is borrowed work on an already-owned child. Cancellation signals retained startup
work, which terminates and reaps before the manager can remove or reuse files.
Ready Stop keeps its graceful placement deadline; startup failure and disconnect
request termination and await exit within `stop_grace`. A failed exit bound retains
the installation and is reported with the action, executable and process identifier,
including through runtime shutdown. Candidate rollback explicitly settles before
removing extracted content. Archive extraction never launches children; upload
staging and guest component bytes are separate from native executable installations.
Guests have no on-disk module cache in this implementation.

Remote disconnection has no child to reap. Scoped publication cleanup still drains
accepted ingestion and reports connection loss to the binding. The attachment
retains that failure in disconnected status and reconnects with a fresh activation;
the old binding stays closed and a new binding may use the same scope. Cleanup
timeout remains terminal under the unchanged placement stop policy. README claims,
public APIs and managed process settlement remain unchanged.

The sandbox limit story's spinning operation consumes finite store fuel; failed
memory growth traps through the store limiter. Fuel is refilled only between idle
batches, never inside a spinning call. Suspended work settles through operation
policy and the virtual clock; store destruction acknowledges guest exit. Inspection
found no unbounded guest loop or machine-speed-dependent wait in these paths.
The existing 120-second story hang guard remains unchanged; native Linux Nightly
must establish whether its isolated timeout recurs under hosted load.
