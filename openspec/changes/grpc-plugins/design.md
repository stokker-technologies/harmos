# gRPC Plugins Design

Status: shape agreed on 2026-11-10; not implemented. Code below is the intended shape, not landed
source. Field numbers and exact names are settled during implementation.

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
| Address | A local endpoint the host assigns at start | A URL the product supplies |
| Who dials | Host | Host |
| Executable hash, kill on shutdown | Yes | No |
| Startup failure | Spawn or connect failure | Connect failure |

Catalog admission does not depend on placement. The remote placement is a new physical backend
behind the existing attach interface.

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

## Operations

- A plugin's operations keep the three visibilities. `Host` means only the host application may
  call it; products use it for operations the host drives.
- Retries are the existing operation policy. Nothing new is added for retry on connection loss.
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
- Connected plugins trust each other. Grants and provider authorization are removed; visibility
  and the fingerprint comparison remain.
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
  while the stream's call is open and sends each returned value as one sample.
- Guests cannot hold a call open, so they declare no streams and no observers.

## Authoring

One source builds as a sidecar or a guest when it reaches the outside world only through the
context.

```rust
pub struct Thermal;

impl harmos::Lifecycle for Thermal {
    type Config = Config;
    type Error = Error;
    type Context = harmos::PluginContext;   // Supervisor on native targets, guest Context on Wasm
}

#[harmos::plugin(id = "thermal", requires = (ReadSensor,), streams(Temperature))]
impl harmos::Plugin for Thermal {
    const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
}

#[harmos::operation(id = "thermal.set", visibility = Host)]
impl harmos::Operation for SetTemperature {
    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<(), Error> {
        ctx.operation(ReadSensor::default()).await?;   // child call, same on both targets
        Ok(())
    }
}

#[harmos::observer(stream = Temperature, interval_ms = 1000)]
async fn temperature(ctx: &harmos::PluginContext) -> f64 {
    ctx.resource::<Chamber>().read().await
}

harmos::export_plugin!(Thermal);   // native: gRPC server; wasm32: guest exports
```

What already exists and stays: operations, catalogs, lifecycle, settings, resources and child
calls in `harmos-sdk`; one plugin macro behind both attributes; guest state retained across calls
for the plugin's session.

What changes: one context alias and one export macro chosen by target; `publish_on` and its error
type shared in the SDK; target-selected dependencies instead of the `sidecar` and `guest`
features.

## Language SDKs

`sdk/<language>` provides, over `harmos.v1`: serve operations, call required operations, declare
and publish streams, observers, lifecycle and settings. Each passes the conformance suite. Product
SDKs (Dokime's `sdk/<language>`) build on these and add their own concepts.

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
| Grants, provider selection and authorization | `crates/harmos-runtime/src/operations/manager/grants.rs`, `OperationProvider` |
| Generation tokens and stale-call fencing | `crates/harmos-runtime/src/plugin/scope.rs`, stale endpoints in `router.rs` |
| Native observer runner | `crates/harmos-sidecar/src/kit/{observer,task}.rs` |
| Transport tests and benches | `crates/harmos-sidecar/tests/transport/`, `benches/transport/` |

Behaviour tests under `crates/harmos-runtime/tests/sidecar/` migrate to the new transport.

## Open Points

Not yet decided with the owner; each is settled before its task starts.

1. Authentication of the managed local endpoint (for example a token passed at start) and
   transport security for a remote URL. Agreed so far: a remote plugin is trusted by whoever adds
   its URL.
2. Reconciling `runtime-observer-cadence`, which changes the observer runner this change removes.
3. Whether `operation-visibility` finishes first or folds into this change.
4. Whether two connected plugins may declare the same operation identifier. Proposed: no; the
   second is refused at connect, which removes provider selection.
5. A shorter operation handler signature than a result of a response of a type, and a context
   without template-specific type parameters. Under research.
