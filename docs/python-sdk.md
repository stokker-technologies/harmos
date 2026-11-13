# Python SDK Guide

Install `harmos-sdk` and import `harmos` on Python 3.13 or later. The package ships
protobuf messages, typed client resources and plugin authoring. The SDK and the
repository tooling ship `py.typed`; the pinned ty checker covers maintained SDK
code, examples, tests and tooling. Ruff enables E4, E7, E9, F, I, N, UP, B and ANN.
Compiler-owned protobuf internals are the untyped boundary; generated message
stubs preserve their public types. No blanket type or annotation ignores apply.

## Typed Host Calls

```python
from harmos import Client
from harmos.v1.status_pb2 import StatusGetRequest, StatusWatchRequest

client = Client.connect("127.0.0.1:53124")
try:
    response = await client.status.get(StatusGetRequest())
    updates = client.status.watch(StatusWatchRequest(), metadata=(("authorization", token),))
    try:
        async for update in updates:
            print(update.status)
            break
    finally:
        updates.cancel()
finally:
    await client.close()
```

`status.get` takes `StatusGetRequest` and returns a native call awaitable whose
result is `StatusGetResponse`. `status.watch` returns a native server-streaming
call with async iteration over `StatusWatchResponse`. Both accept typed
`metadata`, `credentials` and `timeout` keyword arguments. Timeout is a per-call
override in seconds; omitted options retain the transport's defaults. Keep the
call object to invoke its typed `cancel()` method. Stream calls also expose
`read()` for callers using explicit reads instead of iteration.

`Client(channel)` borrows a caller-owned `grpc.aio.Channel`; the caller closes
that channel. `Client.connect(address)` owns its channel and releases it through
`await client.close()`. Transfer budgets are configured with `transfer` and
`message_ceiling` on `connect`; invalid limits refuse configuration before dialing.

## Product Roots

Generate product protobuf modules with the pinned protobuf compiler, including
its `.pyi` output. Generate a composed root from the product descriptor set:

```sh
mise run generate -- --sdk python --descriptors path/to/application-descriptors.bin --output path/to/application/generated/client.py --namespace application
```

The generator merges library and product descriptor definitions, refusing
conflicting files, resource collisions, wrong faces and client-streaming methods.
It writes a marked generated module with a `ClientRoot`, one class per resource,
typed nested attributes and typed methods. Commit that output and regenerate it
when pinning new contracts. Message imports resolve to their defining protobuf
modules, including requests or responses defined in another file or nested message.

Compose product resources with the library using ordinary Python attributes:

```python
import grpc
from application.generated.client import ClientRoot
from harmos import Client

class ApplicationClient(Client):
    def __init__(self, channel: grpc.aio.Channel) -> None:
        super().__init__(channel)
        self.jobs = ClientRoot(channel).jobs
```

`ApplicationClient.connect(address)` preserves the subclass type and initializes
the product's typed `jobs` resource alongside the library's resources. If a
product extends a shared parent such as `status`, retain the complete composed
`ClientRoot(channel)` as a typed attribute instead; it contains both sets of
children. The repository fixture demonstrates composed `jobs.progress` and real
library `status.diagnostics` roots in `sdk/python/tests/fixtures/client/client.py`.

Client constructors accept only the channel. Downstream code that supplied
runtime resource tables must generate a product module at its next pin and
replace table arguments with explicit resource composition. No runtime binding
fallback or serialized inventory remains.

## Runnable Examples

Run from the repository root after preparing the SDK environment:

```sh
mise -C sdk/python run prepare
uv run --locked --no-sync --project sdk/python python sdk/python/examples/client.py --address 127.0.0.1:53124
uv run --locked --no-sync --project sdk/python python sdk/python/examples/plugin.py --address 127.0.0.1:53125
mise -C sdk/python run test:examples
```

[client.py](../sdk/python/examples/client.py) connects to a host, discovers its
operations, makes one typed Status.Get call and consumes one typed Status.Watch
response. It cancels stream demand and closes the channel on completion.
[plugin.py](../sdk/python/examples/plugin.py) serves one public echo operation
using protobuf `BytesValue` requests and responses. Its first stdout line is its
bound endpoint, including the chosen port when binding port zero.

`Examples.test_client_example_discovers_calls_and_streams_from_host` exercises
the client entry point against a real gRPC host fixture.
`Examples.test_plugin_example_serves_one_operation_to_host` starts the actual
plugin program and drives discovery, invocation and Stop over gRPC. Events and
protocol completion synchronize these stories without fixed waits or elapsed-time
assertions. The SDK integration task runs both stories; the native CI SDK jobs
also select the example task. The full conformance fixture separately retains all
31 reference-host rules.

Plugin declarations, child calls, host-clock waits, streams and packaging are
described in [Python Plugin SDK](tooling.md#python-plugin-sdk). Repository tasks
and generation drift checks are documented in [Repository Tooling](tooling.md).
