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
from asyncio import Queue, create_task
from unittest import IsolatedAsyncioTestCase

# Third Party Imports
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from harmos._generated.host import _SUBSCRIPTIONS_SUBSCRIBE
from harmos._invocation import Context, FailureError
from harmos._wire.host.v1.time_pb2 import TimeGetResponse
from harmos._wire.plugin.v1.invocation_pb2 import ChildResponse, Request
from harmos._wire.plugin.v1.subscriptions_pb2 import SubscriptionRow, SubscriptionsSubscribeResponse
from harmos.plugin import Gap, StreamRow, Subscription

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
        item = await reading
        self.assertIsInstance(item, StreamRow)
        self.assertEqual((item.value.now, item.source, item.received), (source, source, received))
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
        self.assertEqual((await reading).value.now, source)
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


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
