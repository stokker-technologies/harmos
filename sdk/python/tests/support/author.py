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

# Third Party Imports
from asyncio import Event

from google.protobuf.duration_pb2 import Duration

from harmos import VISIBILITY_PROTECTED
from harmos._wire.host.v1.time_pb2 import TimeGetResponse

# Local Imports
from harmos.plugin import Context, observer, on_stop, operation
from tests.fixtures.conformance import Value

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Author:
    def __init__(self) -> None:
        self.calls = 0
        self.cleaned = 0
        self.reading = Event()
        self.released = Event()

    @operation("author.wait", request=Value, response=TimeGetResponse, visibility=VISIBILITY_PROTECTED)
    async def wait(self, ctx: Context, request: Value) -> TimeGetResponse:
        self.calls += 1
        ctx.set(request)
        await ctx.host.time.sleep(Duration(seconds=3600))
        at = await ctx.host.time.get()
        if ctx.get(Value) != request:
            raise ValueError("Execution Lost Its Resource")
        return TimeGetResponse(now=at)

    @operation("author.state", request=Value, response=Value, visibility=VISIBILITY_PROTECTED)
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
            await Event().wait()
        finally:
            self.released.set()
        return Value()

    @on_stop
    async def cleanup(self) -> None:
        self.cleaned += 1


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
