---
sidebar: { order: 2 }
---

# Python SDK Guide

Install `harmos-sdk` on Python 3.13 or later. The SDK and repository tooling ship
`py.typed`; ty checks maintained code, examples and tests without annotation ignores.
Compiler-owned protobuf internals are the private untyped boundary; generated stubs
preserve the public message types.

## Public Types

Import connection plumbing, `FailureError` and shared vocabulary such as
`SettingValue`, `Declarations`, `File`, `PageRequest` and `Position` from `harmos`.
Context, Plugin, Operation, decorators, HostOperation and plugin response types
live in `harmos.plugin`. Generated wire requests, version packages
and transport carriers stay private. The protocol failure keeps its name at
`harmos.model.Failure`; raise `FailureError(model.Failure(...))` after importing
`FailureError` and `model` from `harmos`.

Harmos host operations have no wire services or client root. Products declare their
own wrapper operations and CLIENTS, PLUGINS or ALL audiences. Generators project
those product descriptors into one product-owned client over `Connection`; resource
attributes are composed directly, without inheriting a Harmos client tree.

Plugin contexts retain time, own settings, own invocation controls and admitted
discovery as PLUGINS-only facilities on the invocation and Samples carriers.
Products own remote storage, administration, uploads and queries.

## Plugin Settings and Host Calls

```python
from harmos import SettingDeclaration, SettingValue
from harmos.plugin import Catalog, HostOperation, Plugin, Registrar

def catalog() -> Catalog:
    return Catalog(
        handlers,
        requires=[HostOperation.PLUGINS_SETTINGS_LIST],
        settings=[SettingDeclaration(name="prefix", title="Echo Prefix", default=SettingValue(text=""))],
    )


def register(ctx: Registrar) -> None:
    ctx.register(catalog())


plugin = Plugin(id="example", version="1.0.0", register=register)

# Inside an operation handler or an observer answering a window:
settings = await context.host.plugins.settings.list()
prefix = settings.value.values["prefix"].text
```

Plugins.Settings.List selects the admitted calling entry when a plugin omits `name`.
A plugin may name itself; the host refuses another entry. The facility returns declarations, complete values, revision and resolved
read cut; pass `at=cut` to reuse a cut. Client Plugins.Settings.Set replaces the complete map and requires its expected
revision. Preserve each declared key and value variant. Plugin replacement preserves
the entry name and settings; failed replacement preserves the serving bundle.

`context.host.time.get()`, `.time.sleep(duration)`, `.operations.list/get(...)` and
`.invocations.watch(instance, max_item_bytes=...)` hide child-call messages and
project single payloads. Declare the corresponding `HostOperation` requirements.
Host clock convenience methods `context.now()` and `context.sleep(duration)` use the
same contracts. Host policies own retries and deadlines.

## Interval Observers and Windows

The host owns an interval stream's schedule and clock. It grants a `Window` of
`count` consecutive samples, the first at `start` and each following one `period`
later; the plugin answers the whole window before the host grants the next one.

```python
from google.protobuf.duration_pb2 import Duration
from harmos.plugin import Context, Window, observer, window_observer

@observer("bench.reading", row=Reading, interval=Duration(nanos=1_000), window_setting="window")
async def reading(context: Context) -> Reading:
    return Reading(value=read())

@window_observer("bench.readings", row=Reading, columns=ReadingColumns, interval=Duration(nanos=1_000))
async def readings(context: Context, window: Window) -> ReadingColumns:
    return ReadingColumns(value=read_many(window.count))
```

The one-sample observer is called once per sample of each window, in order;
`context.sequence` is the sample's sequence, `window.sequence + i`. The SDK transposes
the rows into columns. The window observer receives the grant and returns the
columns itself, one value per sample in every column. Its columns message must
repeat every field of the row with the same number and type; the SDK checks this
when the observer is declared. An interval row holds one value per field: rows with
repeated, map or oneof fields, including proto3 `optional`, are refused at declaration.

`interval_setting` names a plugin Duration setting that replaces `interval`.
`window_setting` names a plugin Duration setting that overrides the host's window
length for the stream. The SDK splits a window's columns at sample boundaries so
each part fits the plugin's advertised `message_bytes`; a single sample that cannot
fit fails the window, naming the row and the limit.

Observer Context can call declared operations, read settings, use the host clock
and consume watches while answering a window. Children inherit scope, admitted caller
and grants. IDs remain unique across windows of a Samples call; responses may arrive
out of order. Structured failures retain all fields. Answering the window cancels
outstanding children; ending demand cancels the unanswered window. Retained contexts
cannot start children after answering. Device publishers have no windows and cannot
make child calls.

## Product Roots

Generate product messages and typed stubs into a private protocol package using the
[import mapping](../project/architecture.md#product-wrappers-and-the-native-facade). Then generate its composed roots:

```sh
mise run generate -- --sdk python --descriptors path/to/descriptors.bin --output path/to/application/_generated/client.py --namespace application
```

The generator emits a client and a sibling plugin context from descriptor-declared
audiences. Bind `PluginContext(context)` from the generated plugin module in product
handlers; its `host` root includes plugin-callable declarations. The generator merges
library and product descriptors, refuses descriptor conflicts and resource collisions,
and writes the root and public response barrel. Runtime assembly has no audience
overrides. Change the product declaration to change its generated audience.
The product root retains SDK connection ownership. Import `Client` from `application`; call `client.jobs.start(value)`
and `client.plugins.annotations.get(name)` alongside its other product resources.
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

The [client](../../sdk/python/examples/client.py) discovers operations, reads status,
consumes one status update and closes its stream and connection. The
[plugin](../../sdk/python/examples/plugin.py) serves a public echo operation with a typed
prefix setting; its first stdout line announces its bound endpoint. Two integration
stories drive these actual entry points through gRPC. The client uses the reference product wrappers; the plugin uses carrier facilities.
The separate [dynamic invocation example](../../sdk/python/examples/invocation.py)
discovers an operation's fingerprint and calls `client.invocations.create(...)` with
encoded bytes; it returns the unwrapped response bytes and owns its connection:

```sh
uv run --locked --no-sync --project sdk/python python sdk/python/examples/invocation.py --address 127.0.0.1:53124 --operation example.echo --payload 0a0568656c6c6f
```

Application-specific typed calls belong in the application's own generated client.
The separate conformance fixture
covers the reference-host rules, including caller refusal from operations and windows,
window children, window observers, typed settings and discovery.

See [Python Plugin SDK](../project/tooling.md#python-plugin-sdk) for declarations, child calls
and packaging, and [Repository Tooling](../project/tooling.md) for generation drift checks.
