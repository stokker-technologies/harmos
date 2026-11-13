# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""An Author's Stateful Plugin and Typed Clock Consumers."""

# === Imports === #

# Standard Library Imports
import asyncio

# Third Party Imports
from google.protobuf.duration_pb2 import Duration

# Local Imports
from harmos import Context, observer, on_stop, operation
from tests.fixtures.conformance.v1.fixture_pb2 import Value
from harmos.v1 import types_pb2 as wire

from harmos.context.v1 import time_pb2 as clock

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Author:
    def __init__(self):
        self.calls = 0
        self.cleaned = 0
        self.reading = asyncio.Event()
        self.released = asyncio.Event()

    @operation("author.wait", request=Value, response=clock.TimeGetResponse, visibility=wire.VISIBILITY_PROTECTED)
    async def wait(self, ctx: Context, request: Value) -> clock.TimeGetResponse:
        self.calls += 1
        ctx.set(request)
        await ctx.sleep(Duration(seconds=3600))
        at = await ctx.now()
        if ctx.get(Value) != request:
            raise ValueError("Execution Lost Its Resource")
        return clock.TimeGetResponse(now=at)

    @operation("author.state", request=Value, response=Value, visibility=wire.VISIBILITY_PROTECTED)
    async def state(self, ctx: Context, request: Value) -> Value:
        try:
            ctx.get(Value)
        except KeyError:
            return Value(value=self.calls.to_bytes(8, "little"))
        raise ValueError("Another Invocation's Resource Leaked")

    @observer("author.reading", row=Value, interval_setting="author.interval")
    async def observe(self, ctx: Context) -> Value:
        self.reading.set()
        try:
            await asyncio.Event().wait()
        finally:
            self.released.set()
        return Value()

    @on_stop
    async def cleanup(self) -> None:
        self.cleaned += 1


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
