# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Stateful Authoring Uses Host Time and Releases Demand-Owned Work."""

# === Imports === #

# Standard Library Imports
import unittest

# Third Party Imports
import grpc
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from harmos import GET_TIME, SLEEP, Plugin
from tests.fixtures.conformance.v1.fixture_pb2 import Value
from harmos.v1 import types_pb2 as wire
from harmos.plugin.v1.instance_pb2_grpc import InstanceStub
from tests.support import calls
from tests.support.author import Author

from harmos.plugin.v1 import instance_pb2 as carrier

from harmos.context.v1 import time_pb2 as clock

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class HostClockAndState(unittest.IsolatedAsyncioTestCase):
    async def test_host_clock_and_state(self):
        # Serve an Instance with Clock Requirements and a Typed Settings Section.
        author = Author()
        plugin = Plugin(author, id="author", version="1.0.0", requires=[SLEEP, GET_TIME], sections=[Value(value=b"settings")])
        endpoint = await plugin.start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.close)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        client = InstanceStub(channel)
        description = await client.Get(carrier.InstanceGetRequest(revision=1))
        self.assertEqual(description.catalog.streams[0].interval_setting, "author.interval")
        self.assertEqual(Value.FromString(description.catalog.sections[0].payload).value, b"settings")

        # An Hour-Long Wait Is a Child Call, Completed Only by the Host's Reply.
        call = await calls.invoke(client, "author.wait", Value(value=b"resource"))
        sleep = (await call.read()).child
        self.assertEqual(sleep.operation, "harmos.context.v1.Time/Sleep")
        self.assertEqual(clock.TimeSleepRequest.FromString(sleep.payload).duration.seconds, 3600)
        self.assertEqual(sleep.fingerprint, description.catalog.requires[0].fingerprint)
        await calls.reply(call, sleep, clock.TimeSleepResponse())

        # Supply Host Evidence Time Through the Next Child Call.
        now = (await call.read()).child
        self.assertEqual(now.operation, "harmos.context.v1.Time/Get")
        at = Timestamp(seconds=12345, nanos=67890)
        await calls.reply(call, now, clock.TimeGetResponse(now=at))
        result = (await call.read()).response
        self.assertEqual(clock.TimeGetResponse.FromString(result.payload).now, at)
        self.assertIs(await call.read(), grpc.aio.EOF)

        # Instance State Survives Calls While Invocation Resources Stay Isolated.
        state = await calls.invoke(client, "author.state", Value())
        self.assertEqual(Value.FromString((await state.read()).response.payload).value, (1).to_bytes(8, "little"))
        self.assertIs(await state.read(), grpc.aio.EOF)

        # Closing Demand Cancels an Observer's In-Progress Read.
        demand = client.Samples()
        await demand.write(carrier.InstanceSamplesRequest(open=wire.SamplesSelection(stream="author.reading")))
        await demand.write(carrier.InstanceSamplesRequest(tick=wire.Tick(at=at)))
        await author.reading.wait()
        await demand.done_writing()
        self.assertIs(await demand.read(), grpc.aio.EOF)
        await author.released.wait()

        # Reject Invalid Timestamp Ranges Without Losing the Connection.
        invalid = client.Samples()
        await invalid.write(carrier.InstanceSamplesRequest(open=wire.SamplesSelection(stream="author.reading")))
        await invalid.write(carrier.InstanceSamplesRequest(tick=wire.Tick(at=Timestamp(seconds=0, nanos=-1))))
        with self.assertRaises(grpc.aio.AioRpcError) as error:
            await invalid.read()
        self.assertEqual(error.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)

        # Stop Finishes Cleanup and Preserves the Remote Endpoint's Identity.
        await client.Stop(carrier.InstanceStopRequest(reason="Story Complete"))
        self.assertEqual(author.cleaned, 1)
        self.assertEqual(await client.Get(carrier.InstanceGetRequest(revision=1)), description)

        # Duplicate Declarations Fail When the Author Builds the Catalog.
        with self.assertRaises(ValueError):
            Plugin(author, id="author", version="1.0.0", requires=[SLEEP, SLEEP]).catalog()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
