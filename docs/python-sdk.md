# Python SDK Guide

Install `harmos-sdk` on Python 3.13 or later. The SDK and repository tooling ship
`py.typed`; ty checks maintained code, examples and tests without annotation ignores.
Compiler-owned protobuf internals are the private untyped boundary; generated stubs
preserve the public message types.

## Typed Client Calls

```python
from harmos import Client

client = Client.connect("127.0.0.1:53124")
try:
    response = await client.status.get()
    print(response.status.identity.application_name)
    operations = await client.operations.list()
    selected = await client.plugins.get("example")
    updates = client.status.watch(metadata=(("authorization", token),))
    try:
        async for update in updates:
            print(update.status)
            break
    finally:
        await updates.aclose()
finally:
    await client.close()
```

Typed calls take required request content positionally and optional fields as
keyword-only arguments. For example, `client.status.logs.list(at=cut, page=page)` and
`client.plugins.settings.set("example", values, revision)` construct their own
requests. A missing `request_id` is generated; pass an explicit identity when
replaying a mutation. Callers never construct transport requests.

Exactly one response field returns its payload, including a repeated collection;
empty or multiple-field responses retain the response message. Status responses
retain status and resolved read-cut metadata. See the complete retained-response
list in [SDK Call Contracts](architecture.md#sdk-call-contracts).

Unary calls are typed awaitables. Streaming calls are async generators; use
`await updates.aclose()` when ending demand early. Exhaustion, errors and closure
cancel the owned native call. `metadata`, `credentials` and `timeout` are typed
keyword overrides; timeout is in seconds. An omitted deadline remains governed by
operation policy, with no literal SDK default.

`Client(channel)` borrows a caller-owned channel. `Client.connect(address)` owns
its channel and closes it through `await client.close()`. Connection transfer
budgets use `transfer` and `message_ceiling`; invalid limits refuse before dialing.

## Public Types

Import Client, connection options, errors and shared vocabulary such as
`SettingValue`, `Declarations`, `File`, `PageRequest` and `Position` from `harmos`.
Client response types live in `harmos.client`; Context, Plugin, Operation, decorators,
HostOperation and plugin-host response types live in `harmos.plugin`. Each type has
one public path. Generated wire requests and transport carriers stay private.

Storage remains in-process. Applications supply their own operations for data,
uploads and queries. Plugin observers answer ticks; publishers send device rows
while demand is open.

## Plugin Settings and Host Calls

```python
from harmos import SettingDeclaration, SettingValue
from harmos.plugin import HostOperation, Plugin

plugin = Plugin(
    handlers,
    id="example",
    version="1.0.0",
    requires=[HostOperation.SETTINGS_LIST],
    settings=[SettingDeclaration(name="prefix", title="Echo Prefix", default=SettingValue(text=""))],
)

# Inside an operation handler or observer tick:
settings = await context.host.settings.list()
prefix = settings.value.values["prefix"].text
```

Host Settings.List selects the admitted calling entry without a plugin selector.
Its response keeps complete values, revision and issued read cut; pass `at=cut` to
reuse a cut. Client Settings.Set replaces the complete map and requires its expected
revision. Preserve each declared key and value variant. Plugin replacement preserves
the entry name and settings; failed replacement preserves the serving bundle.

`context.host.time.get()`, `.time.sleep(duration)`, `.operations.list/get(...)` and
`.invocations.watch(instance, max_item_bytes=...)` hide child-call messages and
project single payloads. Declare the corresponding `HostOperation` requirements.
Host clock convenience methods `context.now()` and `context.sleep(duration)` use the
same contracts. Host policies own retries and deadlines.

## Tick-Owned Child Calls

Observer Context can call declared operations, read settings, use the host clock
and consume watches while answering a tick. Children inherit scope, admitted caller
and grants. IDs remain unique across ticks; responses may arrive out of order.
Structured failures retain all fields. Returning a row cancels outstanding children;
ending demand cancels the active tick. Retained contexts cannot start children after
answering. Device publishers have no tick and cannot make child calls.

## Product Roots

Generate product protobuf modules and typed stubs into the product's private `_wire`
package, with imports rewritten to that prefix. Then generate its composed root:

```sh
mise run generate -- --sdk python --descriptors path/to/descriptors.bin --output path/to/application/_generated/client.py --namespace application
```

The generator merges product and library descriptors, refuses conflicts and resource
collisions, and writes the root and public response barrel. The product root inherits
SDK connection ownership. Import `Client` from `application`; call `client.jobs.start(value)`
and `client.plugins.annotations.get(name)` alongside ordinary library resources.
The generated read-only properties retain complete nested parents and static types.
No handwritten attribute grafting is needed. Commit and regenerate these outputs
when adopting a new contract pin. The shared fixture lives under
`sdk/python/tests/fixtures/client/`.

## Runnable Examples

```sh
mise -C sdk/python run prepare
uv run --locked --no-sync --project sdk/python python sdk/python/examples/client.py --address 127.0.0.1:53124
uv run --locked --no-sync --project sdk/python python sdk/python/examples/plugin.py --address 127.0.0.1:53125
mise -C sdk/python run test:examples
```

The [client](../sdk/python/examples/client.py) discovers operations, reads status,
consumes one status update and closes its stream and connection. The
[plugin](../sdk/python/examples/plugin.py) serves a public echo operation with a typed
prefix setting; its first stdout line announces its bound endpoint. Two integration
stories drive these actual entry points through gRPC. The separate conformance fixture
covers all 36 reference-host rules, including tick children, typed settings and discovery.

See [Python Plugin SDK](tooling.md#python-plugin-sdk) for declarations, child calls
and packaging, and [Repository Tooling](tooling.md) for generation drift checks.
