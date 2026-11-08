# Remote Plugins (Proposal)

Status: proposal for discussion. Nothing here is implemented yet. The protocol is
[`proto/harmos/plugin/v1/plugin.proto`](../proto/harmos/plugin/v1/plugin.proto).

A plugin becomes any process that serves `harmos.plugin.v1` at a URL, in any
language with gRPC support. The user starts the process and keeps it alive.
The host connects, reads the catalog and routes calls. It never spawns,
restarts or stops plugin processes, and there are no bundles to build or install.

This replaces the four ways a plugin runs today: native sidecar processes over
pipes, in-process embedded providers, WebAssembly guests and the C++ prototype
over a C ABI.

## Lifecycle

1. The application stores a plugin URL.
2. The host calls `Describe` and checks the catalog: protocol version, descriptors,
   operation and stream ids, and requirements against what the host and other
   plugins provide.
3. The host opens `Session`. The plugin answers `Ready` with its catalog digest.
4. The host routes calls, cancellations and stream demand on the session. The
   plugin answers calls, makes child calls and sends stream rows.
5. If the session ends, in-flight calls fail and the host reconnects with
   backoff. It does not start anything.

Generations come from the catalog digest. A reconnect with the same digest keeps
the generation; a new digest starts a new one, exactly as a reload does today.

## A Minimal Plugin

A Python plugin with one operation, using stock `grpcio` and stubs generated from
`plugin.proto` and its own `meter.proto` (`message Read { uint32 channel = 1; }`,
`message Reading { double volts = 1; }`). C++, C# or Go follow the same shape.

```python
class Meter(rpc.PluginServicer):
    async def Describe(self, request, context):
        files = descriptor_pb2.FileDescriptorSet()
        meter_pb2.DESCRIPTOR.CopyToProto(files.file.add())
        return pb.Catalog(
            protocol=1, id="meter", version="0.1.0", descriptors=files,
            operations=[pb.Operation(id="read", request_type="meter.v1.Read",
                                     response_type="meter.v1.Reading",
                                     visibility=pb.VISIBILITY_PUBLIC)],
        )

    async def Session(self, inbound, context):
        yield pb.PluginMessage(ready=pb.Ready())
        async for message in inbound:
            if message.WhichOneof("kind") == "call":
                read = meter_pb2.Read.FromString(message.call.request)
                reading = meter_pb2.Reading(volts=20 + read.channel * 0.5)
                yield pb.PluginMessage(result=pb.Result(
                    id=message.call.id, response=reading.SerializeToString()))
```

Run it with `grpc.aio.server()` on any address and give the host its URL.

## Calls and Cancellation

Each call is one `Call` and one `Result`, with `Cancel` in between if needed.
The host keeps the policy work it already owns: lanes and admission, deadlines,
retries and generation fencing. The plugin only runs attempts.

| Today's phase | What it does | With call and cancel |
| --- | --- | --- |
| `Prepare` | Plugin decodes and validates the request and returns a request-dependent grant | The host validates the request against the descriptor; grants come from `Catalog.requires`, optionally with a static scope |
| `Initialize` | Per-invocation setup inside the plugin | Done inside the plugin's handler |
| `Attempt` | One try, with portable context values | `Call` with `attempt` and `context` |
| `Finalize` | Releases plugin-side state for the invocation | The `Result` ends the call; the plugin cleans up; closing the session ends every call |
| `Cancel` | Cooperative stop | `Cancel` |
| `Stream*` | Streaming request and response bodies | Not in v1; telemetry uses `Rows` |
| SDK `initialize` and `shutdown` actions | Plugin start and stop | Session start and session end |

Uncertain outcomes are never replayed: when a session drops mid-call, the host
reports the call as failed and retries only when the operation's policy allows.

## Schemas

Requests, responses and stream rows are protobuf messages named in
`Catalog.descriptors`. The host derives its internal schema from those
descriptors, so stream revisions and stored row schemas keep working as they do
now. The supported subset is scalars, strings, bytes, nested messages,
`optional`, `repeated`, enums and maps.

Rust plugins keep their `#[harmos::operation]` and `#[harmos::plugin]`
declarations. A Harmos serve helper publishes them as a `Catalog` and converts
payloads at the edge, using the gRPC projection the runtime already has.

## What It Removes

When remote plugins can do what sidecars do, these go:

| Area | Path |
| --- | --- |
| Process attach, pipe framing, credits, plugin-side kit | `crates/harmos-sidecar/` |
| Sidecar hosting, restart policy and supervision | `crates/harmos-runtime/src/services/sidecar/`, `services/restart.rs`, `supervision/` |
| WebAssembly guests | `crates/harmos-guest/`, `crates/harmos-runtime/src/guest/` |
| C++ prototype and C ABI | `crates/harmos-ffi/`, `sdk/cpp/`, replaced by a stock grpc++ example |
| Bundles, dev loop and packaging commands | `crates/harmos-packaging/src/{bundle,build,dev,process}*`, `cargo harmos sidecar` and `package` |
| Embedded providers | `crates/harmos-runtime/src/plugin/host.rs` (`register_embedded*`) |

## Open Questions

- **Request-dependent grants.** Today `Prepare` can scope a grant to the channel,
  stream or source named in the request. Static scopes in `Requirement` cover
  known resources; per-request scopes would need the host to read a marked field.
- **Streaming operations.** v1 has unary calls and stream rows only. Uploads and
  downloads would add stream messages to the session if a plugin needs them.
- **Transport security.** TLS with a pinned CA, plus an optional bearer token in
  metadata, reusing the application's existing remote-access material.
- **Wasm sandboxing.** Remote plugins are isolated by process and network, not by
  Wasmtime limits. Applications that need a sandbox would lose it.
