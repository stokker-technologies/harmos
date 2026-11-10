# gRPC Plugins Design

Status: locked by the owner on 2026-11-10; not implemented, and implementation waits for the
owner's go. Code below is the intended shape, not landed source. Field numbers and exact names are settled during implementation.

## Protocol

```proto
syntax = "proto3";
package harmos.v1;

service Plugin {
  // Identity, start time and catalog. Called on every connect.
  rpc Describe(DescribeRequest) returns (Description);

  // One operation invocation. The host sends the request and may send a cancel; the plugin
  // sends child calls to host operations and finally the response.
  rpc Invoke(stream HostFrame) returns (stream PluginFrame);

  // One declared stream. The host opens the call when it wants the stream and closes it when
  // it no longer does. The plugin sends one sample per message.
  rpc Publish(PublishRequest) returns (stream Sample);
}

message Description {
  string id = 1;
  string version = 2;
  google.protobuf.Timestamp started_at = 3;  // captured once when the process starts
  Catalog catalog = 4;
}

message Catalog {
  repeated OperationContract operations = 1;  // provided, with visibility and policy
  repeated OperationContract requires = 2;    // host or other-plugin operations it calls
  repeated StreamDeclaration streams = 3;
  repeated Section sections = 4;              // opaque to Harmos: namespace, schema, payload
  bytes digest = 5;                           // SHA-256 over the other fields
  google.protobuf.FileDescriptorSet types = 6;  // definitions of every message it serves
}

message OperationContract {
  string id = 1;            // "thermal.set"; unique among connected plugins
  string request = 2;       // "thermal.v1.SetTemperature"
  string response = 3;
  Visibility visibility = 4;
  OperationPolicy policy = 5;
  bytes fingerprint = 6;    // hash of the request and response definitions
}

message HostFrame {
  oneof frame {
    Request request = 1;       // first frame: operation id, payload, attempt, scope
    ChildResponse child = 2;   // answer to a PluginFrame.child
    Cancel cancel = 3;
  }
}

message PluginFrame {
  oneof frame {
    ChildRequest child = 1;    // ctx.operation(...): a call to a required operation
    Response response = 2;     // last frame: payload or Failure
  }
}

message PublishRequest {
  string stream = 1;
  string scope = 2;            // opaque to Harmos; the product names what the stream belongs to
}

message Sample {
  google.protobuf.Timestamp at = 1;
  bytes row = 2;               // one row of the declared stream schema
}
```

`Failure` and `Refusal` keep today's content (classification, code, details, cause, operation
context) as `harmos.v1` messages. Built-in operations use the same package.

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

- A plugin's operations keep the three visibilities. `Host` means only the host application may
  call it; products use it for operations the host drives.
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
- An observer is authoring sugar in the SDK: a function plus an interval. The SDK runs the timer
  while the stream's call is open and sends each returned value as one sample. The interval is a
  declared default or comes from the plugin's settings, as today.
- Guests cannot hold a call open, so they declare no streams and no observers.

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
    let set = Operation::new("thermal.set", set_temperature).visibility(Visibility::Host);
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

## Open Points

Not yet decided with the owner; each is settled before its task starts.

1. The exact wire format. A draft is in `proto/` with its compatibility rules and review
   questions, including whether visibility needs three values or two. It is agreed before any
   implementation.
2. A clock shared by the host and its plugins (proposed). A plugin in another process has its
   own clock, so a test cannot accelerate a story that waits inside a plugin. If every wait
   an SDK offers (`ctx.sleep`, an observer's interval) is served by the host, the host owns
   the only clock. In a test the host advances that clock to the next timer as soon as every
   call in flight is parked on it, so a long story finishes at once in any language. It
   costs one round trip per wait and makes observer ticks come from the host.

The hook through which an application authorizes external clients stays: without it Harmos falls
back to anonymous access, and Dokime uses it to require an authenticated client.
