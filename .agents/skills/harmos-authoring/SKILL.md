---
name: harmos-authoring
description: Author Harmos operations, plugins, streaming endpoints, and generated transport contracts.
---

# Harmos Authoring

Use the original runtime and resource owners. Consult the relevant section only; source is authoritative for signatures.

## Resources And Operation Ownership

- Plain application state and replayable transactions describe durable facts.
  `runtime.state.state()` returns a read-only `ManagedState` handle to the same
  allocation; it is not a snapshot or a place to store device handles.
- Bind nonserializable shared application dependencies through
  `Application::Resources`; plugin `resources()` returns its original initialized
  `Arc<Resources>`. Keep devices, clients, file handles and task ownership out of
  portable requests and persisted configuration.
- The original OperationManager owns admission, generation, policy and physical
  settlement. `#[harmos::operation(declaration = contract::Read)]` forwards the
  resource-bound implementation to the shared declaration. Put declaration
  metadata on a unit struct in its vertical operation file, including unshared
  declarations. Never put operation metadata on catalog enum entries. Catalogs
  assemble already-declared leaf types.
- `new(Arc<Resources>)` constructs an inert owner. `initialize(context)` prepares
  common resources. `run(context, &mut request, &mut input, &mut output)` performs
  work, omitting each endpoint argument when that direction is not declared;
  retain request-dependent handles on `self` for `stop()` and `finalize()`, including
  partial failures. The same request, endpoints and owner survive allowed retries.
- The authored `Operation::run` trait returns `operations::Response<T>` for the
  declared wire payload `T`. `#[harmos::operation]` converts an ordinary payload
  through `Into`; an implementation may return the owned response directly.
  Use an explicit shared declaration when the response handle is imported under
  an alias. The wrapper adds no protobuf field or wire definition.
- Storage producers return their admitted payload through
  `preparation.finish(wire)?.try_into_value()?.try_into_response()?`.
  This fallible conversion admits a conservative delivery allowance covering
  the larger response or stream-item handle against the original read ledger
  before creating its sole shared owner. Clones and cached responses
  retain that same allocation and loan. Encoding releases its delivery owner;
  the loan remains until the last cache, stream, or shared owner is dropped.
  Never replace this path with a copied payload, a second reservation, or an
  ambient context retention slot. `try_into_shared()` retains its existing
  accounting for the separate shared query handle.
- Send an admitted owned streamed payload with `output.send_owned(response).await?`.
  This retains its original payload and loan alongside the existing channel
  permits, with no additional payload owner allocation or wire field. Ordinary
  `send_with` items still require only `Send`; owned items require `Send + Sync`.
  `FileChunk::into_query_value()` retains its existing file-read reservation;
  project the canonical frame through the admitted query-value API before
  converting it into an owned response. `StreamItem::into_request` is fallible:
  a borrowed owned payload cannot be moved out into a mutable request. Likewise,
  consuming `project` refuses an owned item instead of silently copying it.
- Declare portable child metadata through `operations::ContextValue`. Native
  `Context::with_value` consumes and derives the scoped caller; sidecar
  `Supervisor::with_value` and guest `Context::with_value` borrow and clone it.
  They retain the original identity, grants, deadline, cancellation and parent
  admission. Use `CallContext::value::<T>()` for portable metadata, including
  stage scope used by product idempotency. Product key construction remains
  product-owned. Replaced raw portable values invalidate cached local typed
  projections; trusted local-only values retain their original allocation.
  Overlays enforce the combined 64-entry, 64 KiB budget before child admission.
  Child values travel in operation envelope version 5; rebuild both SDK and host
  together. This transient protocol revision does not change persisted records
  or public WIT namespaces.
- Declare `input = Item` and/or `output = Item` explicitly. Context has no optional
  stream handles. Caller APIs are `upload`, `download`, and `exchange`, with
  `send`, `next`, synchronous `finish_input`, `split`, `join`, and
  `cancel_and_settle`. Join requires explicit input EOF and drained output;
  abandonment requests cancellation while the manager retains cleanup ownership.
- A native or guest SDK requirement may select an embedded wire provider through
  `PluginSettings::requires`; its exact contract and grants use original admission.
  Provider reload restarts active dependents and preserves unrelated generations.
- Reload and replacement initialization are interrupted by caller abandonment,
  explicit stop and runtime shutdown. Fence new generations and retain cleanup
  until settlement; a committed replacement supplies later explicit activation.
- Bound signal-handler waits with `Runtime::stop_with_timeout` when the host
  needs an overall shutdown budget. Timeout reports the unfinished phase and
  retains the same cleanup driver; it is not successful settlement or durability.
  Plain `stop` remains an unbounded orderly drain.
- Never retry/cache a streaming operation after item transfer. WireTable streaming
  providers bind standard portable endpoints with explicit `WireStreamLimits`,
  individually or through `bind_streaming_operations`; typed native/guest catalog
  projections forward the same original invocation. Handwritten declarations
  need shape-aware `visit_wire`/`visit_grpc` projections or explicit bindings.


```rust
// operations/validate.rs
#[harmos_contract::operation(
    id = "schematic.validate",
    request = Request,
    response = Response,
    policy = POLICY,
    version = 1,
)]
pub struct Validate;

// operations/mod.rs
mod validate;
pub use validate::Validate;

#[harmos_contract::catalog]
pub enum Operations {
    Validate(Validate),
}
```

The declaration attribute belongs on the struct; the implementation-binding
attribute belongs on its implementation. Catalog-level composition remains on
the catalog. Rust helper attributes for lane/default or nested selection do not
declare operation metadata.

## gRPC Composition And Discovery

- Explicitly register `operations::discovery::Discovery` with the original
  manager to expose `host.operations`; it is not an automatic second catalog.
  With `grpc`, the declared unary route is `/harmos.host.v1.Host/Operations`.
  Requests accept optional `after`, `rows` and `bytes` (defaults 64 and 64 KiB;
  maxima 256 and 4 MiB). Responses contain a UTF-8 JSON `catalog` array of
  canonical `host::DiscoveredOperation` values and an optional `next` cursor.
  Preserve contracts, selected grants, owner/generation and gRPC availability.
  Metadata is conditional: invocation keeps original request grants/admission
  and middleware. Re-discover after reload/restart; paging is live.

- Register one `operations::authorization::Authorizer` through
  `OperationManager::register_authorizer` before external exposure. Its bounded,
  nonblocking `required(&CallContext, Provider)` callback selects a compatible
  canonical `Required` or denies with `None`; the same selection filters discovery
  and actual request grants. Principal roles and explicit entitlement remain
  application-owned. Registration rejects a second owner and freezes at first
  external admission/catalog access; earlier internal boot calls remain valid.
  Callbacks run outside registry locks, and provider readiness is rechecked.
  Configured policies must reject missing principals and undeclared entitlements.
  Without a configured policy, existing explicitly exposed anonymous routes keep
  their behavior. All gRPC adapters use a private external origin, inherited by
  children/retries with a distinct per-ingress cache identity; they are not
  trusted internal host calls. Portable metadata cannot mint that authority.

- Normal operation registration automatically publishes declared gRPC methods
  when their provider is ready. Removing the `grpc(...)` declaration removes that
  exposure; do not add a second exposure catalog or flag.
- Configure and start `runtime.grpc` at application composition. Empty catalogs
  are valid. Use `runtime.grpc.methods()` for local ready-method inspection and
  standard reflection when enabled. Startup/registration tracing reports counts.
- Register ordered asynchronous transport middleware before startup. Trusted
  identity flows into original operation admission; reflection uses the same
  interception boundary. TLS, gRPC-Web (`Settings::web`), and anonymous-access choices belong to the host.
  With `Settings::web` enabled, `allowed_origins` accepts exact `grpc::Origin`
  values. The outer layer handles browser preflight; actual calls still pass
  through ingress authentication. Empty origins emit no CORS headers.
- Macro expansion produces metadata only. Each producer's `build.rs` imports
  its shared contract crate as a build dependency with the intended explicit
  features and calls
  `Artifacts::new(&Contracts::of::<Operations>()?)?.export_build("operations")`
  from `harmos-contract` with `reflection`. Build scripts cannot import their
  own not-yet-built crate.
- Generated bundles live under that producer's `OUT_DIR/grpc/<catalog>/`,
  containing package-relative `.proto` files, `descriptors.bin`, canonical
  `contracts.json` and `manifest.txt`.
  Packaging selects the current invocation's `build-script-executed` Cargo JSON
  messages and preserves producer/catalog ownership. Never glob stale Cargo
  output directories or generate schemas during componentization. Static bundles
  describe known catalogs; runtime reflection describes currently ready methods.
- Generate Python bindings through `harmos_contract::bindings::python::export`
  with the `bindings-python` feature, or the thin `cargo harmos bindings python`
  adapter. Supply explicit distribution/module/version/description/license
  metadata and an exact `Artifacts` export. CLI selection uses an exact export
  directory or the selected build receipt's producer/catalog; never scan stale
  `OUT_DIR`s. Ordinary Rust builds export schemas without Python packaging or
  `protoc`; explicit binding generation invokes the selected `protoc` tool.
- Python output is a normal `src/` package with standard protobuf message
  classes and stubs, `py.typed`, canonical contracts and descriptors. Wheels and
  sdists install without Rust or the producer source. Preserve optional fields,
  integer widths, variants, shared definitions and recursive references.
  Refuse selected non-protobuf contracts, unknown identities, naming collisions
  and unsupported compiler output. Never replace an existing package directory.
- Each generated producer package owns its descriptor pool. Standard protobuf
  `message_factory` creates its classes without process-global symbol
  registration; separate producer classes remain distinct even when protobuf
  names match. `Operation` and structural `Invoker` descriptors are transport
  neutral. Streaming `ResponseStream` exposes typed output items and the declared
  final response. The consumer owns connections, limits, cancellation and
  registration; do not introduce a second generic runtime or codec model.
- `Described` declares a stable type name, direct fields referencing shared type
  definitions, and an inert marker. `schema::graph::Schema` owns a finite root and
  definition table; use borrowed graph views for field traversal. Never expand
  references into nested owned field trees. `config::SchemaSettings` bounds
  preparation memory/work independently of payload byte/nesting/work limits.
- Artifact metadata revision 6 requires rebuilt hosts and SDK consumers. Storage
  metadata uses the generic `graph_v1` representation; Inline is unsupported.
  Only `storage.write_schema` and `storage.query_schemas` advance to semantic
  version 2 for finite schema-description RPC bytes. Product row codecs and
  unrelated operation versions are unchanged. Preserve original data and use
  a fresh storage root for unsupported metadata; never reset or silently migrate.
- Default `Message` bytes are Postcard; protobuf is an explicit separate codec.
  Persist encoding and schema version. Legacy protobuf data requires an explicit,
  bounded migration with an application-owned commit, never decode fallback.



## Saved-View Frame Filters

- `storage::frame::Predicate` has `Compare { expr, compare, value }`,
  `In { expr, values }` and `Exists { expr, present }` variants. Filter vectors
  combine with AND; membership alternatives combine with OR and are capped at
  64. Empty membership matches nothing; duplicate alternatives do not duplicate
  rows. `Exists` excludes both missing and null values.
- Use `ScalarPath::Payload` for validated nested scalar fields and compose
  separate predicates for compound ownership. Keep product scope identifiers in
  the consumer. The original saved view, schema validation and query budgets
  remain authoritative; frame filters do not create a new storage owner.
- Raw saved paging uses `RecordQuery::filter: Option<RecordFilter>` and Core
  `DataSelection::record_filter`. `RecordFilter` declares 1..64 scalar columns
  and at most 64 predicates over `Expr::Col`; membership remains capped at 64.
  Backends implement `StorageBackend::validate_record_filter`, validate selected
  schema paths, and apply predicates under the existing read cut, continuation,
  memory and time bounds. Typed output retains its original decoded allowance.
  The default backend capability
  refuses filtered queries. Keep envelope process scope separate from nested
  product ownership. Direct follow/replay rejects filters; poll saved history
  with an explicit cut when the consumer needs filtered watching.

## Canonical Guest And Sidecar Layout

Use vertical feature folders, with one implementation per leaf and catalog
assembly in each domain's `mod.rs`. Omit folders for capabilities the plugin does
not contribute; do not create empty placeholders.

```text
src/
    lib.rs              Plugin type, lifecycle, and catalog composition
    main.rs             Thin native sidecar runner; absent for a guest library
    config.rs           Typed activation configuration
    resources.rs        Shared initialized resources, when needed
    operations/
        mod.rs          Operation catalog
        <operation>.rs  Contract/handler implementation
    observers/
        mod.rs          Observer catalog
        <observer>.rs   Periodic producer implementation
    services/
        mod.rs          Resident service catalog
        <service>.rs    Resident implementation
    listeners/
        mod.rs          Listener catalog
        <listener>.rs   Record or stream reaction
    streams/
        mod.rs          Published stream catalog
        <stream>.rs     Row schema
    records/
        mod.rs          Published record catalog
        <record>.rs     Record schema
    transactions/       Host transaction invocation wrappers, when used
```

Use `operations/` and `observers/` for new contributions; obsolete `jobs/`,
`routes/` and `samplers/` examples do not establish a competing convention.
Folder names do not imply that a guest supports every sidecar capability. Keep
unsupported callbacks absent until the guest lifecycle implements them.

A shared portable declaration belongs in the product contract; its resource-bound
implementation belongs in the application or plugin. Keep lifecycle initialization
and cleanup on the plugin owner, not independently duplicated in contribution
files. Bundling and transport differences must not create different authoring
layouts for guests and sidecars.


## Unified Plugin Contracts

- Use `#[harmos::plugin]` on a concrete portable `Plugin` implementation with
  explicit `provides` and `requires` typed catalogs. Empty catalogs are explicit;
  a linked type grants no runtime dependency or resource authority.
- Static consumer registration is the optional `register(&mut Registrar)` hook
  on the same portable `Plugin` implementation; ordinary authors never implement
  a second registration-only plugin trait. It runs before preparation and
  resource acquisition, while inspecting embedded metadata executes no code.
- Keep inert `Declared` metadata independent of physical `Plugin` ownership.
  SDK-generated projections use the same macro with explicit `context` and
  `config` types; the original SDK instance forwards those declarations and
  owns actual preparation, initialization, resources and cleanup. Never add an
  unreachable physical implementation just to emit metadata.
- Return clones of the exact initialized resource Arc from `resources`.
  Normal `shutdown(&mut self, &Context)` uses the current operation context;
  context-independent `cleanup(&mut self)` releases physical resources after
  partial setup or EOF. Acknowledged domain shutdown is not replayed while
  physical cleanup resumes.
- Author portable guests with `harmos-guest` and native plugins with
  `harmos-sidecar`; neither authoring crate depends on the host runtime.
  The SDK context identifies its carrier. Host features `plugin-guest` and
  `plugin-sidecar` enable execution and imply the shared `plugin` manager.
  `harmos::packaging` exposes reusable build, inspection, bundle and componentization
  APIs under the `packaging` feature. `cargo-harmos` consumes that facade;
  portable SDK authors retain their dedicated SDK dependency.
  Use `cargo harmos sidecar build|dev` or `cargo harmos guest build|dev` with
  Cargo package/feature selections. Both `dev` commands watch source and local
  dependencies, coalesce and serialize builds, retain usable packages on failure,
  exclude generated outputs and settle the Cargo tree on cancellation. They do
  not activate or reload plugins; hosts own that application integration.
- Host launch settings, grants and `PluginRestartSettings` are supplied once to
  the runtime plugin owner. Automatic restarts default to zero; configured
  allowance is bounded per explicit activation cycle and successful startup does
  not replenish it. Status exposes attempts, last failure class, next retry and
  retained cleanup, including `Stopping` with the original generation and
  `cleanup_pending` while retirement settles. Invalid configuration/contracts
  are terminal; explicit stop
  cancels scheduled restarts and incomplete cleanup prevents overlap. Plugin
  restarts never replay uncertain operations or streaming transfers.
- Guest `PluginSettings::shutdown_timeout` bounds the whole stop wait, including
  SDK settlement, authored shutdown and physical finalization. Timeout retains
  the original invocation and cleanup state. Retry stop to settle that attempt;
  do not treat a bounded wait as physical cancellation or successful retirement.
- Embedded handlers use `register_embedded_wire` / `reload_embedded_wire` when
  portable invocation or declared gRPC exposure is needed. The same authored
  catalog projects `AuthoredDeclarations`, `AuthoredContracts` and
  `WireImplementations<R>`; original dependency/contract/grant admission applies.
  `register_embedded` / `reload_embedded` retain Rust-only bindings. Neither path
  creates another lifecycle owner or requires a portable SDK. Configure explicit
  operation preparation bytes for encoded invocation.
- Keep SDK initialization/shutdown leaves separate from ordinary provides and
  requires. Their explicit policies belong to the authored lifecycle owner;
  the host owns admission, generation and physical settlement.
- Select host Core requirements with `core = Catalog` and semantic grants with
  `capabilities = CONSTANT`. Core leaves use an explicitly authored shared
  `operations::core::Policy<Leaf<Self>>` with one `POLICY` constant per selected
  service. Unselected leaves need no policy; host and consumer contracts must
  match exactly.
  Stream and listener inventories retain their existing narrower grants.
  A requested Core ID alone grants no transaction, endpoint or stream access.
- `Supervisor::core::<AuthoredPlugin>()` and `Context::core::<AuthoredPlugin>()`
  project that selected policy around the original carrier. Typed publications
  await host admission without promising readiness or durability. Native rows
  preserve original producer sequences; guests leave sequence absent so the
  original guest publication lease derives it. Never replay an uncertain call.
- Preserve the distinct generation calling token supplied by SDK initialization.
  Current finite attempts take precedence; retained background handles may use
  only the same live generation. Retirement fences new calls while original
  receipt/resource owners settle admitted work. Do not reuse initialization
  deadlines or construct replacement scopes to authorize cleanup.
- Unified guests use `export_plugin!(Type, capacity = N)` for the minimal async
  operation world. Its portable owner exists before the authored instance and
  retains the same operation table throughout lifecycle and ordinary phases.
  Its read-only `register` export collects consumer metadata through the original
  Registrar before preparation or resource acquisition. Preserve the loader's
  existing collection validation, fuel, epoch and memory bounds. Registration
  is not a parallel lifecycle or application dispatcher; direct component
  execution alone does not prove Root activation or resource authority.
- Native unified plugins use `run_plugin::<Type>(capacity)` or
  `serve_plugin_with::<Type, _, _>(capacity, read, write)`. `PluginNative::<Type>`
  supplies an inspected in-process attachment over independent binary data/control
  channels. `serve_plugin_channels::<Type>(capacity, data, control)` embeds that
  carrier directly; Unix executable stdio uses binary data plus an inherited control socket.
  Windows executable attachment uses a separate local named control pipe.
  Native host/sidecar communication uses the binary carrier; optional `grpc`
  supplies explicit operation protobuf codecs, not a private gRPC transport. The original
  SDK owner/table exists before the authored instance; reserved legacy lifecycle
  frames cannot substitute for sealed SDK operations. Preserve `Attached::control`
  when wrapping attachments and retain the original driver and physical ending.
- Preserve authored stream, observer and listener contributions through the
  unified inventories and ordinary operation callbacks. Portable unit listener descriptors use the same
  `#[harmos::listener]` attribute and `listeners = Audience` catalog selection.
  Their source identity/schema and exact provided callback contract are inert
  metadata; manager feed/readiness belongs to the original host managers.
  Native observers select `observers = Catalog` and actual ordinary Host
  `observer_controls(Start, Stop)` leaves with explicit authored policies. Bind
  the same `ObserverAttachment<Resources, Host>` to initialized resources through the original
  operation table; stop/join it before resource cleanup. Guest observer callbacks
  remain unsupported and nonempty selections are rejected.
