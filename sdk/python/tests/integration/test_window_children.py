# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Observer Children Stay Within One Window and Share Its Scope."""

# === Imports === #

# Standard Library Imports
import asyncio
from asyncio import Event, Task
from unittest import IsolatedAsyncioTestCase

# Third Party Imports
import grpc
from google.protobuf.duration_pb2 import Duration
from google.protobuf.timestamp_pb2 import Timestamp
from google.protobuf.wrappers_pb2 import BytesValue
from grpc import StatusCode
from grpc.aio import EOF, AioRpcError

# Local Imports
from harmos import (
    CLASSIFICATION_REFUSED,
    VISIBILITY_PUBLIC,
    FailureError,
    Position,
    ReadCut,
    SettingDeclaration,
    SettingsDeclarations,
    SettingValue,
    SettingValues,
    model,
)
from harmos._wire.host.v1.time_pb2 import TimeGetResponse, TimeSleepResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceSamplesRequest
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos._wire.plugin.v1.invocation_pb2 import ChildResponse
from harmos._wire.plugin.v1.stream_pb2 import SampleBatch, SamplesSelection, Window
from harmos.plugin import Catalog, Context, HostOperation, Operation, Plugin, Publisher, SettingsListResponse, observer, publisher
from tests.fixtures.conformance import Values

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ECHO = Operation("example.echo", BytesValue, BytesValue, visibility=VISIBILITY_PUBLIC)
WATCH = Operation("example.watch", BytesValue, BytesValue, visibility=VISIBILITY_PUBLIC, watch=True)

# === Types === #

# === Classes === #


class Reader:
    def __init__(self) -> None:
        self.answer = Event()
        self.pending: Task[BytesValue] | None = None
        self.escaped: Context | None = None
        self.settings: SettingsListResponse | None = None

    @observer("example.readings", row=BytesValue, interval=Duration(seconds=1))
    async def read(self, context: Context) -> BytesValue:
        self.escaped = context
        if context.sequence == 1:
            first, second = await asyncio.gather(
                context.call(ECHO, BytesValue(value=b"first")), context.call(ECHO, BytesValue(value=b"second"))
            )
            return BytesValue(value=first.value + second.value + context.scope().encode())
        if context.sequence == 2:
            settings = await context.host.plugins.settings.list()
            self.settings = settings
            now = await context.host.time.get()
            await context.host.time.sleep(settings.value.values["interval"].duration)
            result = b""
            items = context.watch(WATCH, BytesValue(), max_item_bytes=4096)
            try:
                async for item in items:
                    result += item.value
                    break
            finally:
                await items.aclose()
            return BytesValue(value=result + now.seconds.to_bytes(8, "little"))
        if context.sequence == 3:
            self.pending = asyncio.create_task(context.call(ECHO, BytesValue(value=b"pending")))
            await self.answer.wait()
            return BytesValue(value=b"answered")
        try:
            return await context.call(ECHO, BytesValue(value=b"failure"))
        except FailureError as failure:
            return BytesValue(value=failure.value.SerializeToString())

    @publisher("example.device", row=BytesValue)
    async def device(self, context: Context, publisher: Publisher) -> None:
        await publisher.publish(BytesValue(value=b"device"))
        await context.cancelled()


class WindowChildren(IsolatedAsyncioTestCase):
    async def test_observer_children_keep_window_scope_and_reject_invalid_traffic(self) -> None:
        # Declare Typed Settings and Serve the Observer.
        reader = Reader()
        declaration = SettingDeclaration(name="interval", default=SettingValue(duration=Duration(seconds=2)))
        plugin = Plugin(
            id="example",
            version="1.0.0",
            register=lambda ctx: ctx.register(
                Catalog(
                    reader,
                    requires=[ECHO, WATCH, HostOperation.TIME_GET, HostOperation.TIME_SLEEP, HostOperation.PLUGINS_SETTINGS_LIST],
                    settings=[declaration],
                )
            ),
        )
        endpoint = await plugin.start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        host = InstanceStub(channel)
        description = await host.Get(InstanceGetRequest(revision=1))
        settings = SettingsDeclarations.FromString(description.declarations.sections[0].payload)
        self.assertEqual(settings.values, [declaration])

        # Complete Concurrent Children Out of Order, Preserving the Parent Scope.
        stream = host.Samples()
        await stream.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
        at = Timestamp(seconds=100)
        await stream.write(InstanceSamplesRequest(window=Window(start=at, period=Duration(seconds=1), count=1, sequence=1)))
        first = (await stream.read()).child
        second = (await stream.read()).child
        self.assertNotEqual(first.call, second.call)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=second.call, payload=second.payload)))
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=first.call, payload=first.payload)))
        self.assertEqual((await stream.read()).batch, SampleBatch(count=1, columns=_columns(b"firstsecondselected"), last=True))
        assert reader.escaped is not None
        with self.assertRaises(ValueError):
            await reader.escaped.call(ECHO, BytesValue())

        # Settings and the Controlled Host Clock Drive the Next Window's Watch.
        await stream.write(
            InstanceSamplesRequest(window=Window(start=Timestamp(seconds=101), period=Duration(seconds=1), count=1, sequence=2))
        )
        child = (await stream.read()).child
        self.assertGreater(child.call, max(first.call, second.call))
        self.assertEqual(child.operation, HostOperation.PLUGINS_SETTINGS_LIST.value)
        settings = SettingsListResponse(
            declarations=[declaration],
            value=SettingValues(values={"interval": declaration.default}),
            revision=b"settings-revision",
            resolved_at=ReadCut(positions=[Position(sequence=7)]),
        )
        await stream.write(
            InstanceSamplesRequest(
                child=ChildResponse(
                    call=child.call,
                    payload=settings.SerializeToString(),
                )
            )
        )
        child = (await stream.read()).child
        self.assertEqual(child.operation, HostOperation.TIME_GET.value)
        self.assertEqual(reader.settings, settings)
        await stream.write(
            InstanceSamplesRequest(child=ChildResponse(call=child.call, payload=TimeGetResponse(now=at).SerializeToString()))
        )
        child = (await stream.read()).child
        self.assertEqual(child.operation, HostOperation.TIME_SLEEP.value)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=child.call, payload=TimeSleepResponse().SerializeToString())))
        child = (await stream.read()).child
        self.assertTrue(child.watch)
        self.assertEqual(child.operation, WATCH.id)
        self.assertEqual((await stream.read()).child_demand.call, child.call)
        await stream.write(
            InstanceSamplesRequest(child=ChildResponse(call=child.call, item=BytesValue(value=b"observed").SerializeToString()))
        )
        self.assertEqual((await stream.read()).child_cancel.call, child.call)
        batch = (await stream.read()).batch
        self.assertEqual(batch, SampleBatch(count=1, columns=_columns(b"observed" + at.seconds.to_bytes(8, "little")), last=True))

        # Answering a Window Retires Outstanding Calls Before Its Last Part.
        await stream.write(
            InstanceSamplesRequest(window=Window(start=Timestamp(seconds=102), period=Duration(seconds=1), count=1, sequence=3))
        )
        pending = (await stream.read()).child
        reader.answer.set()
        self.assertEqual((await stream.read()).child_cancel.call, pending.call)
        self.assertEqual((await stream.read()).batch.columns, _columns(b"answered"))
        assert reader.pending is not None
        with self.assertRaises(FailureError):
            await reader.pending

        # A Late Retired Reply Cannot Corrupt the Next Window or Its FailureError.
        await stream.write(
            InstanceSamplesRequest(window=Window(start=Timestamp(seconds=103), period=Duration(seconds=1), count=1, sequence=4))
        )
        child = (await stream.read()).child
        self.assertGreater(child.call, pending.call)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=pending.call, payload=b"")))
        failure = model.Failure(
            classification=CLASSIFICATION_REFUSED,
            code="denied",
            operation=ECHO.id,
            plugin="example",
            details={"key": "value"},
            cause=model.Failure(code="source"),
            diagnostics="details",
        )
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=child.call, failure=failure)))
        self.assertEqual(model.Failure.FromString(Values.FromString((await stream.read()).batch.columns).value[0]), failure)
        await stream.done_writing()
        self.assertIs(await stream.read(), EOF)

        # Refuse Child Replies Without an Unanswered Window, Including Device Demand.
        for name in ("example.readings", "example.device"):
            invalid = host.Samples()
            await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream=name)))
            if name == "example.device":
                await invalid.read()
            await invalid.write(InstanceSamplesRequest(child=ChildResponse(call=1, payload=b"")))
            with self.assertRaises(AioRpcError) as error:
                await invalid.read()
            self.assertEqual(error.exception.code(), StatusCode.INVALID_ARGUMENT)

        # Unknown or Duplicate Terminal Replies Cannot Settle Another Child.
        for duplicate in (False, True):
            invalid = host.Samples()
            await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
            await invalid.write(InstanceSamplesRequest(window=Window(start=at, period=Duration(seconds=1), count=1, sequence=1)))
            first = (await invalid.read()).child
            second = (await invalid.read()).child
            reply = InstanceSamplesRequest(child=ChildResponse(call=first.call, payload=first.payload))
            if duplicate:
                await invalid.write(reply)
            else:
                reply.child.call = max(first.call, second.call) + 1
            await invalid.write(reply)
            with self.assertRaises(AioRpcError) as error:
                await invalid.read()
            self.assertEqual(error.exception.code(), StatusCode.INVALID_ARGUMENT)

        # A Second Window Cannot Overwrite the Unanswered Window's Context.
        invalid = host.Samples()
        await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
        await invalid.write(InstanceSamplesRequest(window=Window(start=at, period=Duration(seconds=1), count=1, sequence=1)))
        await invalid.read()
        await invalid.read()
        await invalid.write(InstanceSamplesRequest(window=Window(start=at, period=Duration(seconds=1), count=1, sequence=2)))
        with self.assertRaises(AioRpcError) as error:
            await invalid.read()
        self.assertEqual(error.exception.code(), StatusCode.INVALID_ARGUMENT)

        # Closing Demand Settles the Active Context and Its Outstanding Children.
        closing = host.Samples()
        await closing.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
        await closing.write(InstanceSamplesRequest(window=Window(start=at, period=Duration(seconds=1), count=1, sequence=1)))
        await closing.read()
        await closing.read()
        active = reader.escaped
        assert active is not None
        await closing.done_writing()
        self.assertIs(await closing.read(), EOF)
        await active.cancelled()
        with self.assertRaises(ValueError):
            await active.call(ECHO, BytesValue())

        # Reject Incomplete or Duplicate Settings at Declarations Construction.
        for declarations in ([SettingDeclaration(name="missing")], [declaration, declaration]):
            with self.assertRaises(ValueError):
                Plugin(
                    id="example",
                    version="1.0.0",
                    register=lambda ctx, declarations=declarations: ctx.register(Catalog(reader, settings=declarations)),
                ).declarations()


# === Public Functions === #

# === Private Functions === #


def _columns(value: bytes) -> bytes:
    return Values(value=[value]).SerializeToString()


# === Entry Point === #
