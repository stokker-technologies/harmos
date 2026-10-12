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
- Declare `input = Item` and/or `output = Item` explicitly. Context has no optional
  stream handles. Caller APIs are `upload`, `download`, and `exchange`, with
  `send`, `next`, synchronous `finish_input`, `split`, `join`, and
  `cancel_and_settle`. Join requires explicit input EOF and drained output;
  abandonment requests cancellation while the manager retains cleanup ownership.
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

- Normal operation registration automatically publishes declared gRPC methods
  when their provider is ready. Removing the `grpc(...)` declaration removes that
  exposure; do not add a second exposure catalog or flag.
- Configure and start `runtime.grpc` at application composition. Empty catalogs
  are valid. Use `runtime.grpc.methods()` for local ready-method inspection and
  standard reflection when enabled. Startup/registration tracing reports counts.
- Register ordered asynchronous transport middleware before startup. Trusted
  identity flows into original operation admission; reflection uses the same
  interception boundary. TLS, gRPC-Web (`Settings::web`), and anonymous-access choices belong to the host.
- Macro expansion produces metadata only. Application build scripts use
  `Artifacts::new(&Contracts::of::<Operations>()?)?.export_cargo(manifest, name)`
  from `harmos-contract` with `reflection`; packaging inspects plugin metadata
  without executing it. `harmos-component` exports while packaging guests;
  `harmos-sidecar-export` exports inspected native binaries.
- Generated files live under Cargo's resolved `target/grpc/<name>/`, including
  descriptors, manifest and imported `.proto` files. Honor Cargo target overrides;
  ship generated artifacts for external clients, never commit a second schema.
- Default `Message` bytes are Postcard; protobuf is an explicit separate codec.
  Persist encoding and schema version. Legacy protobuf data requires an explicit,
  bounded migration with an application-owned commit, never decode fallback.



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
  `harmos-guest/componentize` and `harmos-bundle` own host-independent packaging.
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
