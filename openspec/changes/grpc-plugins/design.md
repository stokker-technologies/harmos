# gRPC Plugins Design

Status: locked by the owner on 2026-11-10; not implemented, and implementation waits for the
owner's go. Code below is the intended shape, not landed source. Field numbers and exact names are settled during implementation.

## Protocol

The complete definition is `proto/harmos/v1/plugin.proto`, with its compatibility rules in
`proto/README.md`. In outline:

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
- Samples produced while disconnected are lost.
- Before the host shuts down or removes a plugin it calls `Stop` on it. The plugin runs its
  stop and cleanup hooks and replies; the host closes once every plugin has replied or the
  grace period ends. A managed plugin then exits; a remote one keeps running.

## Operations

- Visibility has two levels. `Private` operations may be called by the declaring plugin and the
  host; products use it for operations the host drives. `Public` operations may be called by
  everyone connected: other plugins and the host's clients. The `Host` level is removed.
- Retries are the existing operation policy. Nothing new is added for retry on connection loss.
- An authored handler returns its value: `Result<T, E>`, not a result of a response of a type.
  The shared-owner `Response<T>` stays below the authoring surface for host code that returns a
  view into a larger allocation.
- A handle obtained on one connection fails when that connection closes and never follows its
  replacement.
- Streaming child calls stay unsupported.

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
  declared default or a plugin setting, which the host resolves.
- A stream without an interval is driven by the plugin, which sends a sample when its device
  produces one.
- Guests cannot hold a call open, so they declare no streams and no observers.

## Clock

The host owns the only clock. An SDK offers no waits of its own: `ctx.sleep` is a call to the
host, and an observer runs on the host's ticks.

- There is no tick rate. The host schedules each wait and each observer interval as a timer and
  wakes the waiter when it is due.
- One clock object per host gives two readings from one source: a monotonic instant for waits
  and deadlines, and a date for evidence, derived as a fixed epoch plus the elapsed time. The
  runtime reads time nowhere else; today about sixty places read Tokio or the system clock
  directly.
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
- The `sidecar` and `guest` variants of the catalog, operation and plugin macros are removed,
  with the plugin, observer and catalog generators. Macros for host-side state (transactions,
  records) are unaffected.
- Concepts shared with a product keep the Harmos name in the product's SDK.

## Language SDKs

The pilot covers Rust (`sdk/rust`), Python (`sdk/python`), C# (`sdk/csharp`) and C++
(`sdk/cpp`). Go is outside the pilot.

Every SDK, Rust included, is built over the `.proto` files: the authoring layer produces and
consumes `harmos.v1` messages, and a transport adapter carries them (a gRPC server for a
sidecar, the guest interface for a Wasm guest). The Rust SDK therefore does not reach into the
host's internal operation machinery, and all SDKs exercise the same contract.

`sdk/<language>` provides, over `harmos.v1`: serve operations, call required operations, declare
and publish streams, observers, lifecycle and settings. Each passes the conformance suite. Product
SDKs (Dokime's `sdk/<language>`) build on these and add their own concepts.

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
- No test depends on real time. A test waits for a condition or runs on a
  paused clock. A plugin under test can be served in the test's own process
  over an in-memory channel, so a paused clock accelerates the host and the
  plugin together.

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
| Plugin, observer and catalog macros and their sidecar and guest variants | `crates/harmos-macros/src/artifact/`, `catalog/typed.rs` (about 1,300 lines with their tests' share) |
| Transport tests and benches | `crates/harmos-sidecar/tests/transport/`, `benches/transport/` |
| C ABI and its C++ prototype | `crates/harmos-ffi`, the reflection bridge under `sdk/cpp`; `sdk/cpp` is rebuilt over `harmos.v1` |

Behaviour tests under `crates/harmos-runtime/tests/sidecar/` migrate to the new transport.

## Implementation Waves

Every change on this branch is delivered together, and Dokime adopts the result. Work proceeds in
waves; within a wave each worker owns a disjoint set of paths, and a wave starts when the one
before it builds and its focused tests pass.

| Wave | Work | Owned Paths |
| --- | --- | --- |
| 1 | The protocol crate and definitions (tasks 2.x); `facility-trim`; `tooling-streamline` | `crates/harmos-protocol`, `proto/`; the JSON backend, signal namespace, authoring tool, response cache, series adapter and stream batches; `mise.toml`, `tools/python`, `.github` |
| 2 | The host clock (task 3.5), alone, because it moves every time read in the runtime, storage and operations | `crates/harmos-runtime`, `crates/harmos-storage`, `crates/harmos-operations` |
| 3 | Placement, invoke, publish, clock operations, visibility and stop on the host (3.1 to 3.4, 3.6, 3.8); the Rust SDK over the protocol (4.x) | Runtime services, plugin, observers and streams; `sdk/rust`, `crates/harmos-sidecar`, `crates/harmos-guest` |
| 4 | Removal of the framed wire, plugin graph, grants and tokens (6.x); the virtual clock mode (3.7); the Python, C# and C++ SDKs with the conformance suite (5.x, 2.3) | The paths being deleted; the clock and simulation; `sdk/python`, `sdk/csharp`, `sdk/cpp` |
| 5 | `authoring-without-macros`, one worker at a time, since each step touches every crate | The whole workspace |
| 6 | `test-stories`; guests on the `harmos.v1` messages (7.1); guides | Test trees; the guest crate and host; documentation |

## Open Points

Not yet decided with the owner; each is settled before its task starts.

1. The exact wire format. A draft is in `proto/` with its compatibility rules and review
   questions. It is agreed before any implementation.

The hook through which an application authorizes external clients stays: without it Harmos falls
back to anonymous access, and Dokime uses it to require an authenticated client.
