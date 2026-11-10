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
from harmos import NOW, SLEEP, Plugin
from harmos.conformance.v1.fixture_pb2 import Value
from harmos.v1 import plugin_pb2 as wire
from harmos.v1.plugin_pb2_grpc import PluginStub
from tests.support import calls
from tests.support.author import Author

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class HostClockAndState(unittest.IsolatedAsyncioTestCase):
    async def test_host_clock_and_state(self):
        # Serve an Instance with Clock Requirements and a Typed Settings Section.
        author = Author()
        plugin = Plugin(author, id="author", version="1.0.0", requires=[SLEEP, NOW], sections=[Value(value=b"settings")])
        endpoint = await plugin.start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.close)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        client = PluginStub(channel)
        description = await client.Describe(wire.DescribeRequest(revision=1))
        self.assertEqual(description.catalog.streams[0].interval_setting, "author.interval")
        self.assertEqual(Value.FromString(description.catalog.sections[0].payload).value, b"settings")

        # An Hour-Long Wait Is a Child Call, Completed Only by the Host's Reply.
        call = await calls.invoke(client, "author.wait", Value(value=b"resource"))
        sleep = (await call.read()).child
        self.assertEqual(sleep.operation, "harmos.clock.sleep")
        self.assertEqual(wire.Sleep.FromString(sleep.payload).duration.seconds, 3600)
        self.assertEqual(sleep.fingerprint, description.catalog.requires[0].fingerprint)
        await calls.reply(call, sleep, wire.SleepResponse())

        # Supply Host Evidence Time Through the Next Child Call.
        now = (await call.read()).child
        self.assertEqual(now.operation, "harmos.clock.now")
        at = Timestamp(seconds=12345, nanos=67890)
        await calls.reply(call, now, wire.Time(now=at))
        result = (await call.read()).response
        self.assertEqual(wire.Time.FromString(result.payload).now, at)
        self.assertIs(await call.read(), grpc.aio.EOF)

        # Instance State Survives Calls While Invocation Resources Stay Isolated.
        state = await calls.invoke(client, "author.state", Value())
        self.assertEqual(Value.FromString((await state.read()).response.payload).value, (1).to_bytes(8, "little"))
        self.assertIs(await state.read(), grpc.aio.EOF)

        # Closing Demand Cancels an Observer's In-Progress Read.
        demand = client.Publish()
        await demand.write(wire.PublishFrame(open=wire.PublishRequest(stream="author.reading")))
        await demand.write(wire.PublishFrame(tick=wire.Tick(at=at)))
        await author.reading.wait()
        await demand.done_writing()
        self.assertIs(await demand.read(), grpc.aio.EOF)
        await author.released.wait()

        # Reject Invalid Timestamp Ranges Without Losing the Connection.
        invalid = client.Publish()
        await invalid.write(wire.PublishFrame(open=wire.PublishRequest(stream="author.reading")))
        await invalid.write(wire.PublishFrame(tick=wire.Tick(at=Timestamp(seconds=0, nanos=-1))))
        with self.assertRaises(grpc.aio.AioRpcError) as error:
            await invalid.read()
        self.assertEqual(error.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)

        # Stop Finishes Cleanup and Preserves the Remote Endpoint's Identity.
        await client.Stop(wire.StopRequest(reason="Story Complete"))
        self.assertEqual(author.cleaned, 1)
        self.assertEqual(await client.Describe(wire.DescribeRequest(revision=1)), description)

        # Duplicate Declarations Fail When the Author Builds the Catalog.
        with self.assertRaises(ValueError):
            Plugin(author, id="author", version="1.0.0", requires=[SLEEP, SLEEP]).catalog()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
