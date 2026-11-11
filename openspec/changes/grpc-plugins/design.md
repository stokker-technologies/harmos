# gRPC Plugins Design

## Resuming

Integration branch: `feat/grpc-plugins`; every change under `openspec/changes/` is delivered together on it.
All changes use the one `grpc-plugins` implementation marker. Source and proof are tracked separately in tasks.
Next work is the first unticked tasks in wave order; resolve confirmations before changing those contracts.
Final validation waits for source closure; completed source does not close native proof.

| Wave | Implementation Waves | State and Next Work | Ownership |
| --- | --- | --- | --- |
| 1 | Protocol 2.x; `facility-trim`; `tooling-streamline`; `align-ci-release` | Source landed; tooling follow-ups and native proof remain | `proto/`, protocol/conformance crates; removed facilities; tooling and CI |
| 2 | Host clock 3.5 | Landed | Runtime, storage and built-in operation time reads |
| 3 | Host 3.1–3.4, 3.6, 3.8, 3.10–3.12; Rust SDK 4.x | Finish generated-protobuf declarations before consumer adoption; migrate host handlers and SDK boundaries | Protocol, domain, operations, runtime, SDK roots, derives and adapters |
| 4 | Removal 6.x; virtual mode 3.7; pilot SDKs 5.x | Finish host engine relocation, dead conversions and contract-only surface removal | Operations/domain/runtime; removed transport paths; simulation; pilot SDKs |
| 5 | `authoring-without-macros` | Inventory, then declaration steps in task order | One writer across the workspace |
| 6 | `test-stories`; guests 7.1; guides | Inventory, stories and guest envelopes remain; consumer adoption is separate proof | Tests; guest/host adapters; docs |

After the last gRPC wave in both products, `storage-interface` designs and implements the Harmos
extensions first. Dokime consumes them and extracts storage in its own `storage-interface` change.
Neither storage implementation nor extraction belongs to these gRPC waves.

Give a worker one whole task, its specification section and an explicit disjoint path ownership list.
Name its focused `mise run build -- --package ... --check` and `mise run test -- --package ... --target ...` selections;
pass `--features` where required. Run `mise run format -- --language ...` and the applicable scoped check before handoff.
A handoff is at most ten lines: task, owned paths, changes, focused proof, format/check result and remaining follow-ups.
SDK system suites run in Nightly only. No full lane has been established for this candidate; native Linux proof remains open.

Status: implementation is partial. The new `plugin-grpc` feature coexists with the framed path;
`sdk/rust` supplies plugin authoring under its temporary package name; the former
`crates/harmos-sdk` engine now lives in `crates/harmos-operations/src/execution/`.
Guest-private dispatch extraction, the public SDK rename and facade removal remain open.
The intended final contracts below do not imply that the remaining removal and migration tasks are complete.

## Crate Structure

Target layout (arrows below mean "depends on"; this is the target, not the current manifests):

```text
sdk/rust/                         harmos: public plugin authoring + client
crates/harmos-protocol/            generated protobuf definitions + light helpers
crates/harmos-domain/              native state/recovery and storage contracts only
crates/harmos-storage/             generic persistence, queries, capture and backends
crates/harmos-operations/          built-in declarations + handlers, one file per operation
crates/harmos-runtime/             host engine, composition, plugins, clock and listeners
crates/harmos-conformance/         cross-language protocol proof and reference host
sdk/{python,csharp,cpp}/           language authoring + client implementations
sdk/rust/ (Wasm module)            guest transport; no host engine
crates/harmos-runtime/ (guest)     guest loading and sandbox ownership
examples/*/ and guest fixtures/   consumers, outside the core crate graph

runtime -> operations -> storage -> protocol
runtime/operations/storage -> reduced domain -> protocol
SDK harmos -> protocol
conformance -> protocol; reference host -> runtime
CLI / desktop host consumers -> runtime
```

The host chain is linear: protocol, storage, operations, runtime, then shells. Direct downward
edges are allowed; reverse edges are not. The reduced Harmos domain is a host-side foundation,
not another wire model or an SDK dependency. Derive/build support is tooling, outside this
runtime dependency chain. Public SDK runtime dependencies are protocol and transport/authoring
support, never domain, storage, operations or runtime; the only product layer below it is protocol.

`sdk/rust` is the `harmos` product crate itself. `use harmos::prelude::*;` exposes plugin
authoring and the client without compiling host code. Host embedding uses `harmos-runtime`
explicitly. Move the surviving engine from `crates/harmos-sdk` to its host owners; do not merge
it into the public SDK. Fold client functionality into the SDK's client half and remove the
old facade root at `crates/harmos`. Native serving and Wasm guest transport share this SDK;
guest loading remains in runtime. Conformance is development-only, not an SDK dependency.

`proto/` owns all language-neutral contracts. `harmos-protocol` holds generated messages,
services and descriptors with light constructors, operators, `Display` and conversions for
Harmos's own code. Behaviour needed by another language is specified in words beside the proto
contract and implemented by each SDK; Rust helpers are not the cross-language specification.

Keep in `harmos-domain` only non-wire native contracts: typed state/transaction catalogs,
inverse pairing and undo/replay envelopes (`src/state/`, `src/journal.rs`), native schema and
stored-value encoding/allocation contracts (`src/schema/`, `src/message/`), and storage catalog,
object revision, snapshot, query-plan and durability contracts (`src/model/storage/`) needed by
generic storage. These are Rust state/recovery/storage semantics, not parallel protobuf DTOs.
Audit each retained type against its wire counterpart: replace wire mirrors with protocol types;
move protocol helpers to protocol, dispatch/resources to their engine owners and operation
policy to its callee. Delete obsolete plugin inventory, grant and envelope models after migration.

Every host operation lives in `harmos-operations`, declaration and handler in one operation
file, taking and returning protocol messages. Existing built-ins under
`src/{host,lifecycle,storage,streams}/` already colocate implementations: retain that ownership.
Handlers access narrow state, lifecycle, publication and storage resources supplied by runtime;
operations never depends on runtime. Keep deadlines and retries with the Rust callee and allow
call-site overrides for one call. Remove the operations `runtime` feature after all
contract-only consumers use protocol/SDK; do not move handlers into runtime to keep that surface.

Extend the client-facing gRPC declaration/codec integration before Dokime's Harmos-adoption
wave (Dokime wave 2). Generated protobuf request/result types plus authoritative descriptors
must register directly, retaining decoding-memory admission, fingerprints, method paths,
cancellation, backpressure and unary/client-stream/server-stream/bidirectional envelopes.
The affected seam is `crates/harmos-runtime/src/grpc/{codec,routing,native,input,envelope}.rs`,
operation registration and `crates/harmos-operations/src/execution/operations/serving/protobuf.rs`. A declaration owns its gRPC mapping; the host exposes that declaration without a
per-service adapter or a duplicate native request/response model.

### Dokime Consumer Layout and Sequence

```text
sdk/rust/                         dokime: plugin authoring + client half
crates/dokime-protocol/            generated product messages + light helpers
crates/dokime-storage/             application persistence (after gRPC waves)
crates/dokime-operations/          declarations + handlers + gRPC mappings
crates/dokime-runtime/             execution manager, state, transactions, cycles,
                                  effects, plugin entries, listener and main.rs
crates/dokime-cli/                 commands; hosts through runtime's library API
crates/dokime-gui/src-tauri/        desktop host composition
crates/dokime-tracing/             small development subscriber/export support

CLI / desktop -> runtime -> operations -> storage -> protocol
dokime [sdk/rust] -> protocol; client crate becomes its client half
```

`use dokime::prelude::*;` supplies authoring and client APIs with no host compilation. There is
no Dokime domain crate: definitions are protocol messages; admission validation, parameter
expansion and expression evaluation live with the operations applying them; persisted shapes
live in storage. Keep transactions, state and execution/cycle transitions together in runtime.
No separate presentation extraction is required by this target.

In Dokime's host wave (wave 3), move handlers area by area from
`crates/dokime-runtime/src/runtime/operations/`, `domains/` and
`crates/dokime-server/src/grpc/` into operations and register directly. Delete each area's
`hosted!` and `hosting::{Host,Serve}` trampoline as it moves. Logs and trace replay, support
bundles, transfers/upload commit and abandonment, paging/frontier closure, watches/heartbeats,
plot deltas/reduction and redaction remain real operation behaviour. Retain their bounds,
cleanup, snapshot and resume semantics; transport registration does not implement them.
Remote access becomes listener configuration plus the one client authorization hook, preserving
pairing/key persistence, certificate rotation and shutdown. Runtime owns reusable startup and
listener composition, with `main.rs` as its thin process entry, so CLI hosting calls the runtime
library rather than depending on a binary. No server crate or per-service adapters remain.

In the last Dokime waves (CLI wave 4, removal wave 5), redirect CLI and desktop hosting to
runtime, remove the server crate, dead conversions and generation dependencies, dissolve
domain into the owners above, and remove the operations `runtime` feature when no contract-only
consumer remains. Retain `dokime-tracing` as small development support. Update manifests,
build scripts, examples and user guidance with these boundaries.

Only after the last wave, Harmos `storage-interface` supplies the five generic extensions;
Dokime's separate change of the same name then merges
`crates/dokime-runtime/src/storage/` and `src/runtime/storage/` into `dokime-storage` through
the Harmos storage interface. Break reverse dependencies on runtime/query/plot first; runtime
retains transaction and cycle orchestration, passing owned inputs and narrow resources.
Execution/step statistics, history filters/tokens, finalization/seals, report capture, artifact
inventory and fresh-root refusal remain Dokime behaviour built on those generic facilities.
Logs, support bundles, exports/report spools, plugin installation, desktop root lease/backup,
client pairing/download files, CLI and build tooling stay with their owners. Extraction covers
application persistence, not all disk I/O. Both changes require before/after benchmarks,
including deterministic multi-day data, restart, late samples and finalization.

## Protocol

The complete definition is `proto/harmos/v1/plugin.proto`, with its compatibility rules in
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
| Startup failure | Spawn or connect failure | Connect failure |

Catalog admission does not depend on placement. The remote placement is a new physical backend
behind the existing attach interface.

A managed plugin is any executable that serves `harmos.v1`. It binds a loopback port of its own
choosing and prints the address as one line on standard output; the host reads that line and
dials. The host therefore never has to find a free port, and the line doubles as the signal
that the plugin is ready. `HARMOS_ADDRESS` sets the address instead, which is how the same
executable runs as a remote plugin. Nothing else about the executable matters, so a plugin in
any language with a build output can be managed.

```text
harmos listening 127.0.0.1:53124
```

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
- Before the host shuts down or removes a plugin it calls `Stop` on it. The plugin runs its
  stop and cleanup hooks and replies; the host closes once every plugin has replied or the
  grace period ends. A managed plugin then exits; a remote one keeps running.

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
- Connected plugins trust each other. Grants and provider authorization between plugins are
  removed; visibility and the fingerprint comparison remain, and so does authorization of
  external clients.
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
(`sdk/cpp`). Go is outside the pilot. Python publishes as `harmos-plugin` and imports as `harmos`.
The Rust SDK at `sdk/rust` is the public package and library `harmos`. Plugin and client
authors depend on it and use `harmos::prelude::*;` without compiling host code. Task 4.1
renames the current temporary package `harmos-sdk-grpc`, deletes `crates/harmos-sdk` and
the `crates/harmos` facade, and migrates their consumers. Host APIs belong to
`harmos-runtime`; a host embedder depends on that crate explicitly. This is the owner
decision for the final entry point; the temporary package and facade still exist until
task 4.1 is implemented.

Every SDK, Rust included, is built over the `.proto` files: the authoring layer produces and
consumes `harmos.v1` messages, and a transport adapter carries them (a gRPC server for a
sidecar, the guest interface for a Wasm guest). The Rust SDK therefore does not reach into the
host's internal operation machinery, and all SDKs exercise the same contract.

`sdk/<language>` provides, over `harmos.v1`: serve operations, call required operations, declare
and publish streams, observers, lifecycle and settings. Each passes the conformance suite. Product
SDKs (Dokime's `sdk/<language>`) build on these and add their own concepts.

### Python Authoring

Python uses generated protobuf message classes and decorated async instance methods.
The [Python authoring example](../../../docs/tooling.md#python-plugin-sdk) shows
operation, observer and cleanup registration, typed child calls and remote serving.
The instance owns author state; `Context` owns invocation resources and host clock calls.

### C# Authoring

C# uses a typed builder registering async instance methods. The class owns its state;
requests, replies and rows are generated protobuf message classes. `Grpc.Tools` generates
code at build time; generated C# is not checked in. C# and C++ authoring use the builder
shapes below; Python uses the linked decorated-method example.

```csharp
using Harmos.Sdk;
using Harmos.V1;
using Google.Protobuf.WellKnownTypes;
// SetTemperature, Temperature and ReadSensor are generated from the product's .proto files.

await new ThermalPlugin().Build().ServeAsync();

sealed class ThermalPlugin
{
    private readonly Chamber chamber = new();
    private readonly Operation<ReadSensor, Temperature> read = new("thermal.read", Visibility.Public);

    public Harmos.Sdk.Plugin Build() => new Harmos.Sdk.Plugin("thermal", "1.0.0")
        .Operation(new Operation<SetTemperature, Temperature>("thermal.set", Visibility.Public), SetAsync)
        .Requires(read)
        .Observer("thermal.temperature", new Duration { Seconds = 1 }, ObserveAsync)
        .OnStop(chamber.CloseAsync);

    private async Task<Temperature> SetAsync(Context context, SetTemperature request)
    {
        await context.CallAsync(read, new ReadSensor());
        await chamber.SetAsync(request.Target, context.CancellationToken);
        return await ObserveAsync(context);
    }

    private Task<Temperature> ObserveAsync(Context context) => chamber.ReadAsync(context.CancellationToken);
}
```

`ServeAsync(address: "127.0.0.1:53124", remote: true)` serves an explicitly chosen remote
endpoint and remains alive after Stop. `HARMOS_ADDRESS` overrides the address in either
mode. Managed mode defaults to a dynamically bound loopback port and announces readiness.
`Stream<TRow>` registers a device producer receiving a context and typed `Publisher<TRow>`;
closing demand cancels the context and ends publication. `Section` carries typed product
contributions, including product settings definitions. `Context.Scope` remains opaque.

Declare `Requires(Clock.Sleep)` or `Requires(Clock.Now)` before using `context.SleepAsync`
or `context.NowAsync`. These are child calls on the invocation, never local timers.
Observers receive the host tick in `Context.At` and the per-demand `Context.Sequence`;
they cannot call operations or sleep. Background work outside these host-visible waits
is outside virtual-time accounting and runs in real time. Instance state shared by
concurrent calls must use the application's normal synchronization.
### C++ Authoring

C++20 uses generated protobuf message classes and typed builder values. Handlers
return their protobuf response directly and throw `harmos::Failure` for a structured
application failure. `ctx.call` starts a child call and returns a typed future;
`ctx.operation` waits for that future. C++20 reserves `requires`, so the builder
spells the dependency method `requires_operation`.

```cpp
const harmos::Operation<thermal::v1::ReadSensor, thermal::v1::Reading>
    read_sensor("sensor.read");
harmos::Operation<thermal::v1::SetTemperature, thermal::v1::Reading>
    set("thermal.set", set_temperature);
set.visibility(harmos::wire::VISIBILITY_PUBLIC);
harmos::Stream<thermal::v1::Reading> temperature("thermal.temperature");
harmos::Observer<thermal::v1::Reading> observer(temperature, read_temperature);
google::protobuf::Duration interval;
interval.set_seconds(1);
observer.every(interval);
harmos::Plugin("thermal", "1.0.0")
    .operation(set).requires_operation(read_sensor)
    .stream(temperature).observer(observer)
    .serve();
```

`serve()` selects managed lifecycle and reads `HARMOS_ADDRESS`, defaulting to an
ephemeral loopback port. `serve("127.0.0.1:53124")` selects remote lifecycle and
keeps serving after Stop. Both announce the bound address. Observer callbacks
cannot make child calls or clock waits. Device producers use
`Stream<Row>::publish_on` with a typed `Publisher<Row>`, ending when demand closes.
`section(message)` carries typed settings/contributions; `on_stop(callback)` owns
cleanup. Invocation resources use `ctx.set(value)` and `ctx.get<Type>()`; shared
author state is captured explicitly by handlers. Authors synchronize shared state
across concurrent calls. Independently started background work is invisible to
virtual time.

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
| Postcard operation envelopes | `crates/harmos-sdk/src/operations/serving/codec.rs`, after guests move |
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

Task 1.2 covers `proto/`, `crates/harmos`, `crates/harmos-domain`,
`crates/harmos-protocol`, `crates/harmos-conformance`,
`crates/harmos-sdk`, `crates/harmos-sdk-derive`, `sdk/{rust,python,csharp,cpp}`,
`crates/harmos-runtime/src/{clock,plugin,operations,streams,observers,services,simulation}`,
`crates/harmos-storage` timing consumers, `crates/harmos-operations`, `crates/harmos-macros`,
`crates/harmos-sidecar`, `crates/harmos-guest` and `crates/harmos-ffi`.
Migrate `examples/schematic`, guest sandbox and runtime/SDK fixtures, sidecar behaviour tests,
transport benches, manifests/locks, `mise.toml`, `tools/python`, CI, README, product/topic guides
and canonical/generated authoring guidance. Removal paths are enumerated above.
Cross-cutting declarations and test consolidation follow their own change inventories.

The Crate Structure section defines this batch's boundary replacements. Storage extensions and
Dokime storage extraction follow after the last wave as separate `storage-interface` changes.
Existing System test serial scheduling remains.
The host clock and facility trim may share storage paths only through sequential handoff and
review of the combined tree; the clock no longer needs an isolated implementation wave.

The external-client authorization hook remains. Without it Harmos falls back to anonymous
access; products may use it to require an authenticated client.

## Open Points

Guest migration (7.1) is deferred: lifecycle phases and child submit, status, join and release controls have no equivalent in `harmos.v1`; guest-private dispatch, envelopes and authoring support preserve those behaviours independently of native plugin removal.

### Awaiting Owner Confirmation

- Unary-only plugin operations, with streamed requests/results confined to host-native operations.
- Default policy on the new path: no deadline, zero cancellation grace and no retries.
- `interval_setting` as a qualified protobuf section field of type `google.protobuf.Duration`.
- Required consumer-owned `PlacementSettings.stream_window`.
- Python package `harmos-plugin` / import `harmos`. The Rust public entry point is settled in Language SDKs.
- The Python, C# and C++ authoring shapes in Language SDKs.
- Deferral of client survival across transient drops until Harmos request deduplication and watch replay are specified.

The built-in visibility mapping awaits confirmation in `operation-visibility`; retaining the
hand-maintained downsampling kernels awaits confirmation in `facility-trim`.
