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

# Third Party Imports
from unittest import IsolatedAsyncioTestCase

import grpc
from google.protobuf.timestamp_pb2 import Timestamp
from grpc import StatusCode
from grpc.aio import EOF, AioRpcError

from harmos import SamplesSelection, Tick
from harmos._wire.plugin.host.v1.time_pb2 import TimeGetResponse, TimeSleepRequest, TimeSleepResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceGetRequest, InstanceSamplesRequest, InstanceStopRequest
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub

# Local Imports
from harmos.plugin import HostOperation, Plugin
from tests.fixtures.conformance import Value
from tests.support import calls
from tests.support.author import Author

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class HostClockAndState(IsolatedAsyncioTestCase):
    async def test_host_clock_and_state(self) -> None:
        # Serve an Instance with Clock Requirements and a Typed Settings Section.
        author = Author()
        plugin = Plugin(
            author,
            id="author",
            version="1.0.0",
            requires=[HostOperation.TIME_SLEEP, HostOperation.TIME_GET],
            sections=[Value(value=b"settings")],
        )
        endpoint = await plugin.start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.close)
        channel = grpc.aio.insecure_channel(endpoint.address)
        self.addAsyncCleanup(channel.close)
        client = InstanceStub(channel)
        description = await client.Get(InstanceGetRequest(revision=1))
        self.assertEqual(description.declarations.streams[0].interval_setting, "author.interval")
        self.assertEqual(Value.FromString(description.declarations.sections[0].payload).value, b"settings")

        # An Hour-Long Wait Is a Child Call, Completed Only by the Host's Reply.
        call = await calls.invoke(client, "author.wait", Value(value=b"resource"))
        sleep = (await call.read()).child
        self.assertEqual(sleep.operation, "harmos.plugin.host.v1.Time/Sleep")
        self.assertEqual(TimeSleepRequest.FromString(sleep.payload).duration.seconds, 3600)
        self.assertEqual(sleep.fingerprint, description.declarations.requires[0].fingerprint)
        await calls.reply(call, sleep, TimeSleepResponse())

        # Supply Host Evidence Time Through the Next Child Call.
        now = (await call.read()).child
        self.assertEqual(now.operation, "harmos.plugin.host.v1.Time/Get")
        at = Timestamp(seconds=12345, nanos=67890)
        await calls.reply(call, now, TimeGetResponse(now=at))
        result = (await call.read()).response
        self.assertEqual(TimeGetResponse.FromString(result.payload).now, at)
        self.assertIs(await call.read(), EOF)

        # Instance State Survives Calls While Invocation Resources Stay Isolated.
        state = await calls.invoke(client, "author.state", Value())
        self.assertEqual(Value.FromString((await state.read()).response.payload).value, (1).to_bytes(8, "little"))
        self.assertIs(await state.read(), EOF)

        # Closing Demand Cancels an Observer's In-Progress Read.
        demand = client.Samples()
        await demand.write(InstanceSamplesRequest(open=SamplesSelection(stream="author.reading")))
        await demand.write(InstanceSamplesRequest(tick=Tick(at=at)))
        await author.reading.wait()
        await demand.done_writing()
        self.assertIs(await demand.read(), EOF)
        await author.released.wait()

        # Reject Invalid Timestamp Ranges Without Losing the Connection.
        invalid = client.Samples()
        await invalid.write(InstanceSamplesRequest(open=SamplesSelection(stream="author.reading")))
        await invalid.write(InstanceSamplesRequest(tick=Tick(at=Timestamp(seconds=0, nanos=-1))))
        with self.assertRaises(AioRpcError) as error:
            await invalid.read()
        self.assertEqual(error.exception.code(), StatusCode.INVALID_ARGUMENT)

        # Stop Finishes Cleanup and Preserves the Remote Endpoint's Identity.
        await client.Stop(InstanceStopRequest(reason="Story Complete"))
        self.assertEqual(author.cleaned, 1)
        self.assertEqual(await client.Get(InstanceGetRequest(revision=1)), description)

        # Duplicate Declarations Fail When the Author Builds the Declarations.
        with self.assertRaises(ValueError):
            Plugin(author, id="author", version="1.0.0", requires=[HostOperation.TIME_SLEEP, HostOperation.TIME_SLEEP]).declarations()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
