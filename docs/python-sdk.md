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
from harmos._wire.client.v1.status_pb2 import StatusGetRequest, StatusWatchRequest

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

## Boundary Modules and Registries

Shared messages live in `harmos.v1` concept modules such as `setting_pb2`,
`policy_pb2` and `invocation_pb2`. Client wire messages live in
`harmos._wire.client.v1`; plugin-host messages live in `harmos.plugin.host.v1`.
The plugin implements `harmos.plugin.v1.Instance`; its handler object remains
`Context`. Align imports, generated roots and requirement fingerprints when
updating a consumer pin.

```python
from harmos._wire.client.v1.operations_pb2 import OperationsListRequest, OperationsGetRequest
from harmos._wire.client.v1.schemas_pb2 import SchemasListRequest
from harmos._wire.client.v1.streams_pb2 import StreamsListRequest

operations = await client.operations.list(OperationsListRequest())
selected = await client.operations.get(OperationsGetRequest(id=operation_id))
schemas = await client.schemas.list(SchemasListRequest())
streams = await client.streams.list(StreamsListRequest())
```

Each registry returns typed entries. Storage stays in process: applications
provide their own typed wire operations for data access, uploads and queries.
Plugins answer ticks or publish device rows through `Instance.Samples` while
stream demand is open. Application operations own other publication requests.

## Plugin Settings

```python
from harmos import SETTINGS, Context, Plugin
from harmos.v1.setting_pb2 import SettingDeclaration, SettingValue

plugin = Plugin(
    handlers,
    id="example",
    version="1.0.0",
    requires=[SETTINGS],
    settings=[SettingDeclaration(name="prefix", title="Echo Prefix", default=SettingValue(text=""))],
)

# In an Operation Handler or While Answering an Observer Tick
settings = await context.settings()
prefix = settings.value.values["prefix"].text
```

`settings` contributes one typed `SettingsDeclarations` section to the reported
`Declarations`. Each declaration requires a name and a selected typed default: flag,
integer, number, text or duration. False flags and empty text retain their
selected variant. The library stores overrides; `context.settings()` calls
plugin-host `Settings.List` for the admitted calling plugin, without a plugin
selector. Declare `SETTINGS` in `requires`. Its response includes complete values,
the opaque settings revision and the issued read cut; pass `at` to resend a cut.

Clients use `client.plugins.settings.list` and `.set` with messages from
`harmos._wire.client.v1.plugins.settings_pb2`. Set replaces the complete
`SettingValues` map; retain every declared key and variant and supply the selected
revision and request identity. `client.plugins.replace` replaces a managed bundle
in place, preserving its entry name and settings; failed replacement preserves
the serving bundle. Entry deletion removes settings atomically. Application data
cleanup follows the in-process plugin watch and startup reconciliation and is
eventual.

## Tick-Owned Child Calls

An observer's `Context` can call declared operations, read settings, use `now()`
and `sleep()` on the host clock, and consume declared watches while answering a
tick. Declare each dependency, including `GET_TIME`, `SLEEP` and `SETTINGS`, in
`requires`. Child calls inherit the stream scope and the host's admitted caller
and grants. IDs remain unique across ticks on one Samples call; replies may arrive
out of order. Structured child failures preserve all fields.

Returning a row answers the tick and cancels its outstanding children. Closing
stream demand cancels the active tick. Keep all work owned by that tick; a retained
context cannot start another child after its answer. Device publishers have no
tick and cannot make child calls. The SDK refuses unknown or duplicate terminal
replies, extra ticks before an answer and child traffic without an active tick.

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
product extends a shared parent such as `plugins`, retain the complete composed
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
using protobuf `BytesValue` requests and responses. It declares a typed prefix
setting and reads the calling plugin's values before answering. Its first stdout line is its
bound endpoint, including the chosen port when binding port zero.

`Examples.test_client_example_discovers_calls_and_streams_from_host` exercises
the client entry point against a real gRPC host fixture.
`Examples.test_plugin_example_serves_one_operation_to_host` starts the actual
plugin program and drives discovery, invocation and Stop over gRPC. Events and
protocol completion synchronize these stories without fixed waits or elapsed-time
assertions. The SDK integration task runs both stories; the native CI SDK jobs
also select the example task. The full conformance fixture separately covers
the reference-host rules, including tick children, typed settings and operation discovery.

Plugin declarations, child calls, host-clock waits, streams and packaging are
described in [Python Plugin SDK](tooling.md#python-plugin-sdk). Repository tasks
and generation drift checks are documented in [Repository Tooling](tooling.md).

The reported message is `harmos.v1.Declarations`; the SDK builder is
`declarations()`. Regenerate client/plugin bindings when adopting this revision.
Guest settings use the same library entry and settings services as other placements;
see [WebAssembly Guests](plugin-manager.md#webassembly-guests).
