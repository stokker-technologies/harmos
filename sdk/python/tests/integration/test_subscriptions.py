# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Subscription Delivery and Demand-Only Closure."""

# === Imports === #

# Standard Library Imports
from asyncio import Event, Queue, Task, create_task, shield
from unittest import IsolatedAsyncioTestCase

# Third Party Imports
import grpc
from google.protobuf.timestamp_pb2 import Timestamp
from grpc.aio import EOF

# Local Imports
from harmos import CLASSIFICATION_CANCELLED, VISIBILITY_PUBLIC
from harmos._generated.host import _SUBSCRIPTIONS_SUBSCRIBE
from harmos._invocation import Context, FailureError
from harmos._wire.host.v1.time_pb2 import TimeGetResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceInvokeRequest, InstanceSamplesRequest
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos._wire.plugin.v1.invocation_pb2 import Cancel, ChildResponse, Request
from harmos._wire.plugin.v1.stream_pb2 import SamplesSelection
from harmos._wire.plugin.v1.subscriptions_pb2 import SubscriptionRow, SubscriptionsSubscribeResponse
from harmos.plugin import Catalog, Gap, Plugin, Publisher, StreamRow, Subscription, operation, publisher
from tests.support import calls

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Subscriptions(IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.output = Queue()
        contract = _SUBSCRIPTIONS_SUBSCRIBE
        self.context = Context(
            Request(operation="fixture.read", scope="unit/first"), "fixture", {contract.id: contract.fingerprint}, self.output
        )

    async def open(self) -> tuple[Subscription[TimeGetResponse], int]:
        subscribing = create_task(self.context.subscribe("fixture.samples", TimeGetResponse, max_item_bytes=4096))
        child = (await self.output.get()).child
        self.assertEqual(child.operation, _SUBSCRIPTIONS_SUBSCRIBE.id)
        self.assertTrue(child.watch)
        demand = (await self.output.get()).child_demand
        self.assertEqual(demand.call, child.call)
        await self.context._reply(ChildResponse(call=child.call, item=SubscriptionsSubscribeResponse(opened=True).SerializeToString()))
        return await subscribing, child.call

    async def test_timestamps_gap_and_close_leave_another_demand_running(self) -> None:
        first, first_call = await self.open()
        second, second_call = await self.open()
        received = Timestamp(seconds=11, nanos=987654321)
        source = Timestamp(seconds=7, nanos=123456789)
        reading = create_task(anext(first))
        self.assertEqual((await self.output.get()).child_demand.call, first_call)
        row = SubscriptionRow(payload=TimeGetResponse(now=source).SerializeToString(), source=source, received=received)
        await self.context._reply(ChildResponse(call=first_call, item=SubscriptionsSubscribeResponse(row=row).SerializeToString()))
        self.assertEqual(await reading, StreamRow(TimeGetResponse(now=source), source, received))
        reading = create_task(anext(first))
        await self.output.get()
        await self.context._reply(ChildResponse(call=first_call, item=SubscriptionsSubscribeResponse(missed=17).SerializeToString()))
        self.assertEqual(await reading, Gap(17))
        await first.close()
        self.assertEqual((await self.output.get()).child_cancel.call, first_call)
        self.assertNotIn(first_call, self.context._watches)
        self.assertIn(second_call, self.context._watches)
        parent = Context(Request(operation="fixture.read", scope="unit/first"), "fixture", {}, self.output)
        parent._children.append(self.context)
        await parent._reply(ChildResponse(call=first_call, item=SubscriptionsSubscribeResponse(row=row).SerializeToString()))
        reading = create_task(anext(second))
        self.assertEqual((await self.output.get()).child_demand.call, second_call)
        await self.context._reply(ChildResponse(call=second_call, item=SubscriptionsSubscribeResponse(row=row).SerializeToString()))
        self.assertEqual(await reading, StreamRow(TimeGetResponse(now=source), source, received))
        await second.close()

    async def test_scope_cancellation_ends_the_outstanding_read(self) -> None:
        subscription, call = await self.open()
        reading = create_task(anext(subscription))
        await self.output.get()
        await self.context._cancel("Fixture Scope Ended")
        self.assertEqual((await self.output.get()).child_cancel.call, call)
        with self.assertRaises(FailureError):
            await reading
        self.assertNotIn(call, self.context._watches)
        await subscription.close()

    async def test_close_unblocks_a_pending_read_and_is_idempotent(self) -> None:
        subscription, call = await self.open()
        reading = create_task(anext(subscription))
        await self.output.get()
        await subscription.close()
        self.assertEqual((await self.output.get()).child_cancel.call, call)
        with self.assertRaises(StopAsyncIteration):
            await reading
        await subscription.close()
        self.assertTrue(self.output.empty())

    async def test_invocation_scope_ends(self) -> None:
        class Reader:
            def __init__(self) -> None:
                self.reading: Task[StreamRow[TimeGetResponse] | Gap] | None = None
                self.context: Context | None = None
                self.settle = Event()
                self.stopped = Event()

            @operation("fixture.read", request=TimeGetResponse, response=TimeGetResponse, visibility=VISIBILITY_PUBLIC)
            async def read(self, ctx: Context, request: TimeGetResponse) -> TimeGetResponse:
                self.context = ctx
                subscription = await ctx.subscribe("fixture.samples", TimeGetResponse, max_item_bytes=4096)
                self.reading = create_task(anext(subscription))
                try:
                    if request.now.seconds:
                        await self.settle.wait()
                    else:
                        await shield(self.reading)
                    return TimeGetResponse()
                finally:
                    self.stopped.set()

        for ending in ("cancel", "disconnect", "settle"):
            with self.subTest(ending=ending):
                # Serve the Real SDK Invocation Runtime and Admit Its Subscription.
                reader = Reader()
                endpoint = await Plugin(
                    id="fixture",
                    version="1.0.0",
                    register=lambda ctx, reader=reader: ctx.register(Catalog(reader, requires=[_SUBSCRIPTIONS_SUBSCRIBE])),
                ).start("127.0.0.1:0")
                self.addAsyncCleanup(endpoint.stop)
                channel = grpc.aio.insecure_channel(endpoint.address)
                self.addAsyncCleanup(channel.close)
                call = await calls.invoke(
                    InstanceStub(channel), "fixture.read", TimeGetResponse(now=Timestamp(seconds=int(ending == "settle")))
                )
                self.addCleanup(call.cancel)
                child = (await call.read()).child
                self.assertEqual(child.operation, _SUBSCRIPTIONS_SUBSCRIBE.id)
                self.assertEqual((await call.read()).child_demand.call, child.call)
                await call.write(
                    InstanceInvokeRequest(
                        child=ChildResponse(call=child.call, item=SubscriptionsSubscribeResponse(opened=True).SerializeToString())
                    )
                )
                self.assertEqual((await call.read()).child_demand.call, child.call)

                # End the Scope While a Subscription Read Remains Outstanding.
                if ending == "settle":
                    reader.settle.set()
                    self.assertEqual((await call.read()).WhichOneof("frame"), "response")
                    self.assertIs(await call.read(), EOF)
                elif ending == "cancel":
                    await call.write(InstanceInvokeRequest(cancel=Cancel(reason="Scope Ended")))
                    self.assertEqual((await call.read()).child_cancel.call, child.call)
                    self.assertEqual((await call.read()).response.failure.classification, CLASSIFICATION_CANCELLED)
                    self.assertIs(await call.read(), EOF)
                else:
                    call.cancel()
                await reader.stopped.wait()
                assert reader.reading is not None
                with self.assertRaises(FailureError) as failure:
                    await reader.reading
                self.assertEqual(failure.exception.value.classification, CLASSIFICATION_CANCELLED)
                assert reader.context is not None
                self.assertEqual(reader.context._watches, {})
                self.assertTrue(reader.context._cancelled.is_set())

    async def test_producer_scope_ends(self) -> None:
        class Device:
            def __init__(self) -> None:
                self.started = Event()
                self.stopped = Event()
                self.context: Context | None = None

            @publisher("harmos.samples", row=TimeGetResponse)
            async def produce(self, ctx: Context, output: Publisher) -> None:
                self.context = ctx
                self.started.set()
                try:
                    await ctx.cancelled()
                finally:
                    self.stopped.set()

        device = Device()
        endpoint = await Plugin(id="harmos", version="1.0.0", register=lambda ctx: ctx.register(Catalog(device))).start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.stop)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        demand = InstanceStub(channel).Samples()
        self.addCleanup(demand.cancel)
        await demand.write(InstanceSamplesRequest(open=SamplesSelection(stream="harmos.samples")))
        await device.started.wait()
        demand.cancel()
        await device.stopped.wait()
        assert device.context is not None
        self.assertTrue(device.context._cancelled.is_set())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
