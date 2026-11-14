# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Observer Children Stay Within One Tick and Share Its Scope."""

# === Imports === #

# Standard Library Imports
import asyncio
import unittest

# Third Party Imports
import grpc
from google.protobuf.duration_pb2 import Duration
from google.protobuf.timestamp_pb2 import Timestamp
from google.protobuf.wrappers_pb2 import BytesValue

# Local Imports
from harmos import GET_TIME, SETTINGS, SLEEP, Context, Failure, Operation, Plugin, Publisher, observer, publisher
from harmos.plugin.host.v1.settings_pb2 import SettingsListResponse
from harmos.plugin.host.v1.time_pb2 import TimeGetResponse, TimeSleepResponse
from harmos.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceSamplesRequest
from harmos.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos.v1.failure_pb2 import CLASSIFICATION_REFUSED
from harmos.v1.failure_pb2 import Failure as WireFailure
from harmos.v1.invocation_pb2 import ChildResponse
from harmos.v1.operation_pb2 import VISIBILITY_PUBLIC
from harmos.v1.setting_pb2 import SettingDeclaration, SettingsDeclarations, SettingValue, SettingValues
from harmos.v1.stream_pb2 import SamplesSelection, Tick

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ECHO = Operation("example.echo", BytesValue, BytesValue, visibility=VISIBILITY_PUBLIC)
WATCH = Operation("example.watch", BytesValue, BytesValue, visibility=VISIBILITY_PUBLIC, watch=True)

# === Types === #

# === Classes === #


class Reader:
    def __init__(self) -> None:
        self.answer = asyncio.Event()
        self.pending: asyncio.Task[BytesValue] | None = None
        self.escaped: Context | None = None

    @observer("example.readings", row=BytesValue, interval=Duration(seconds=1))
    async def read(self, context: Context) -> BytesValue:
        self.escaped = context
        if context.sequence == 1:
            first, second = await asyncio.gather(
                context.call(ECHO, BytesValue(value=b"first")), context.call(ECHO, BytesValue(value=b"second"))
            )
            return BytesValue(value=first.value + second.value + context.scope().encode())
        if context.sequence == 2:
            settings = await context.settings()
            now = await context.now()
            await context.sleep(settings.value.values["interval"].duration)
            result = b""
            async with context.watch(WATCH, BytesValue(), max_item_bytes=4096) as items:
                async for item in items:
                    result += item.value
                    break
            return BytesValue(value=result + now.seconds.to_bytes(8, "little"))
        if context.sequence == 3:
            self.pending = asyncio.create_task(context.call(ECHO, BytesValue(value=b"pending")))
            await self.answer.wait()
            return BytesValue(value=b"answered")
        try:
            return await context.call(ECHO, BytesValue(value=b"failure"))
        except Failure as failure:
            return BytesValue(value=failure.value.SerializeToString())

    @publisher("example.device", row=BytesValue)
    async def device(self, context: Context, publisher: Publisher) -> None:
        await publisher.publish(BytesValue(value=b"device"))
        await context.cancelled()


class TickChildren(unittest.IsolatedAsyncioTestCase):
    async def test_observer_children_keep_tick_scope_and_reject_invalid_traffic(self) -> None:
        # Declare Typed Settings and Serve the Observer.
        reader = Reader()
        declaration = SettingDeclaration(name="interval", default=SettingValue(duration=Duration(seconds=2)))
        plugin = Plugin(reader, id="example", version="1.0.0", requires=[ECHO, WATCH, GET_TIME, SLEEP, SETTINGS], settings=[declaration])
        endpoint = await plugin.start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.close)
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
        await stream.write(InstanceSamplesRequest(tick=Tick(at=at)))
        first = (await stream.read()).child
        second = (await stream.read()).child
        self.assertNotEqual(first.call, second.call)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=second.call, payload=second.payload)))
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=first.call, payload=first.payload)))
        sample = (await stream.read()).samples[0]
        self.assertEqual(BytesValue.FromString(sample.row).value, b"firstsecondselected")
        self.assertEqual(sample.at, at)
        self.assertEqual(sample.sequence, 1)
        assert reader.escaped is not None
        with self.assertRaises(ValueError):
            await reader.escaped.call(ECHO, BytesValue())

        # Settings and the Controlled Host Clock Drive the Next Tick's Watch.
        await stream.write(InstanceSamplesRequest(tick=Tick(at=Timestamp(seconds=101))))
        child = (await stream.read()).child
        self.assertGreater(child.call, max(first.call, second.call))
        self.assertEqual(child.operation, SETTINGS.id)
        await stream.write(
            InstanceSamplesRequest(
                child=ChildResponse(
                    call=child.call,
                    payload=SettingsListResponse(value=SettingValues(values={"interval": declaration.default})).SerializeToString(),
                )
            )
        )
        child = (await stream.read()).child
        self.assertEqual(child.operation, GET_TIME.id)
        await stream.write(
            InstanceSamplesRequest(child=ChildResponse(call=child.call, payload=TimeGetResponse(now=at).SerializeToString()))
        )
        child = (await stream.read()).child
        self.assertEqual(child.operation, SLEEP.id)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=child.call, payload=TimeSleepResponse().SerializeToString())))
        child = (await stream.read()).child
        self.assertTrue(child.watch)
        self.assertEqual(child.operation, WATCH.id)
        self.assertEqual((await stream.read()).child_demand.call, child.call)
        await stream.write(
            InstanceSamplesRequest(child=ChildResponse(call=child.call, item=BytesValue(value=b"observed").SerializeToString()))
        )
        self.assertEqual((await stream.read()).child_cancel.call, child.call)
        sample = (await stream.read()).samples[0]
        self.assertEqual(BytesValue.FromString(sample.row).value, b"observed" + at.seconds.to_bytes(8, "little"))
        self.assertEqual(sample.sequence, 2)

        # Answering a Tick Retires Outstanding Calls Before the Sample.
        await stream.write(InstanceSamplesRequest(tick=Tick(at=Timestamp(seconds=102))))
        pending = (await stream.read()).child
        reader.answer.set()
        self.assertEqual((await stream.read()).child_cancel.call, pending.call)
        self.assertEqual(BytesValue.FromString((await stream.read()).samples[0].row).value, b"answered")
        assert reader.pending is not None
        with self.assertRaises(Failure):
            await reader.pending

        # A Late Retired Reply Cannot Corrupt the Next Tick or Its Failure.
        await stream.write(InstanceSamplesRequest(tick=Tick(at=Timestamp(seconds=103))))
        child = (await stream.read()).child
        self.assertGreater(child.call, pending.call)
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=pending.call, payload=b"")))
        failure = WireFailure(
            classification=CLASSIFICATION_REFUSED,
            code="denied",
            operation=ECHO.id,
            plugin="example",
            details={"key": "value"},
            cause=WireFailure(code="source"),
            diagnostics="details",
        )
        await stream.write(InstanceSamplesRequest(child=ChildResponse(call=child.call, failure=failure)))
        self.assertEqual(WireFailure.FromString(BytesValue.FromString((await stream.read()).samples[0].row).value), failure)
        await stream.done_writing()
        self.assertIs(await stream.read(), grpc.aio.EOF)

        # Refuse Child Replies Without an Unanswered Tick, Including Device Demand.
        for name in ("example.readings", "example.device"):
            invalid = host.Samples()
            await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream=name)))
            if name == "example.device":
                await invalid.read()
            await invalid.write(InstanceSamplesRequest(child=ChildResponse(call=1, payload=b"")))
            with self.assertRaises(grpc.aio.AioRpcError) as error:
                await invalid.read()
            self.assertEqual(error.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)

        # Unknown or Duplicate Terminal Replies Cannot Settle Another Child.
        for duplicate in (False, True):
            invalid = host.Samples()
            await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
            await invalid.write(InstanceSamplesRequest(tick=Tick(at=at)))
            first = (await invalid.read()).child
            second = (await invalid.read()).child
            reply = InstanceSamplesRequest(child=ChildResponse(call=first.call, payload=first.payload))
            if duplicate:
                await invalid.write(reply)
            else:
                reply.child.call = max(first.call, second.call) + 1
            await invalid.write(reply)
            with self.assertRaises(grpc.aio.AioRpcError) as error:
                await invalid.read()
            self.assertEqual(error.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)

        # A Second Tick Cannot Overwrite the Unanswered Tick's Context.
        invalid = host.Samples()
        await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
        await invalid.write(InstanceSamplesRequest(tick=Tick(at=at)))
        await invalid.read()
        await invalid.read()
        await invalid.write(InstanceSamplesRequest(tick=Tick(at=at)))
        with self.assertRaises(grpc.aio.AioRpcError) as error:
            await invalid.read()
        self.assertEqual(error.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)

        # Closing Demand Settles the Active Context and Its Outstanding Children.
        closing = host.Samples()
        await closing.write(InstanceSamplesRequest(open=SamplesSelection(stream="example.readings", scope="selected")))
        await closing.write(InstanceSamplesRequest(tick=Tick(at=at)))
        await closing.read()
        await closing.read()
        active = reader.escaped
        assert active is not None
        await closing.done_writing()
        self.assertIs(await closing.read(), grpc.aio.EOF)
        await active.cancelled()
        with self.assertRaises(ValueError):
            await active.call(ECHO, BytesValue())

        # Reject Incomplete or Duplicate Settings at Declarations Construction.
        for declarations in ([SettingDeclaration(name="missing")], [declaration, declaration]):
            with self.assertRaises(ValueError):
                Plugin(reader, id="example", version="1.0.0", settings=declarations).declarations()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
