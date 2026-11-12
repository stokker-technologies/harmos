# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve the Fixed Conformance Catalog at an Explicit Remote Address."""

# === Imports === #

# Standard Library Imports
import argparse
import asyncio
import sys

# Third Party Imports
from google.protobuf.duration_pb2 import Duration

# Local Imports
from harmos import GET_TIME, SLEEP, Context, Failure, Operation, Plugin, Publisher, observer, operation, publisher
from harmos.conformance.v1.fixture_pb2 import Graph, PayloadOnly, Value
from harmos.v1 import types_pb2 as wire

# Type Checking Imports

# === Public Exports === #

# === Constants === #

HOST_ECHO = Operation("conformance.host.echo", Value, Value)
HOST_PENDING = Operation("conformance.host.pending", Value, Value)
HOST_FAIL = Operation("conformance.host.fail", Value, Value)
HOST_SCHEMA = Operation("conformance.host.schema", Graph, Value)

# === Types === #

# === Classes === #


class Fixture:
    def __init__(self):
        self.echoes = 0

    @operation("conformance.echo", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def echo(self, ctx: Context, request: Value) -> Value:
        self.echoes += 1
        if request.value == b"scope":
            return Value(value=ctx.scope().encode())
        return request

    @operation("conformance.children", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def children(self, ctx: Context, request: Value) -> Value:
        first, second = await asyncio.gather(ctx.call(HOST_ECHO, Value(value=b"\x01")), ctx.call(HOST_ECHO, Value(value=b"\x02")))
        return Value(value=first.value + second.value)

    @operation("conformance.cancel", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def cancel(self, ctx: Context, request: Value) -> Value:
        return await ctx.call(HOST_PENDING, request)

    @operation("conformance.failure", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def failure(self, ctx: Context, request: Value) -> Value:
        raise Failure(
            wire.Failure(
                classification=wire.CLASSIFICATION_PERMANENT,
                code="fixture_failure",
                message="Fixture Failed",
                action="Use Another Input",
                operation="conformance.failure",
                plugin="conformance",
                cause=wire.Failure(
                    classification=wire.CLASSIFICATION_TRANSIENT,
                    code="fixture_cause",
                    message="Fixture Cause",
                    action="Retry",
                    operation="conformance.host.fail",
                    plugin="host",
                ),
                details={"fixture": "value"},
                diagnostics="Fixture Diagnostics",
            )
        )

    @operation("conformance.child_failure", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def child_failure(self, ctx: Context, request: Value) -> Value:
        return await ctx.call(HOST_FAIL, request)

    @operation("conformance.policy", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def policy(self, ctx: Context, request: Value) -> Value:
        first = await ctx.call(
            HOST_ECHO,
            request,
            policy=wire.OperationPolicy(
                retries=wire.RetryPolicy(
                    total_attempts=3, initial=Duration(seconds=1), maximum=Duration(seconds=4), deterministic_jitter=True
                )
            ),
        )
        return await ctx.call(HOST_ECHO, first)

    @operation("conformance.schema", request=Graph, response=Graph.Nested, visibility=wire.VISIBILITY_PUBLIC)
    async def schema(self, ctx: Context, request: Graph) -> Graph.Nested:
        return Graph.Nested(count=len(request.items), next=request)

    @operation("conformance.nested", request=Graph.Nested, response=Graph, visibility=wire.VISIBILITY_PUBLIC)
    async def nested(self, ctx: Context, request: Graph.Nested) -> Graph:
        return request.next

    @operation("conformance.projected", request=Graph, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def projected(self, ctx: Context, request: Graph) -> Value:
        return request.tail.value

    @operation("conformance.clock", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def clock(self, ctx: Context, request: Value) -> Value:
        before = await ctx.now()
        await ctx.sleep(Duration(seconds=3600))
        after = await ctx.now()
        return Value(value=(after.seconds - before.seconds).to_bytes(8, "little", signed=True))

    @operation("conformance.private", request=Value, response=Value)
    async def private(self, ctx: Context, request: Value) -> Value:
        return request

    @operation("conformance.resources", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC, replay_safe=True)
    async def resources(self, ctx: Context, request: Value) -> Value:
        ctx.set(request)
        await ctx.call(HOST_ECHO, request)
        return ctx.get(Value)

    @operation("conformance.cancel_reason", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def cancel_reason(self, ctx: Context, request: Value) -> Value:
        await ctx.call(HOST_ECHO, request)
        return Value(value=(await ctx.cancelled()).encode())

    @observer("conformance.ticks", row=Value, interval=Duration(seconds=1))
    async def ticks(self, ctx: Context) -> Value:
        return Value(value=ctx.sequence.to_bytes(8, "little"))

    @publisher("conformance.device", row=Value)
    async def device(self, ctx: Context, publisher: Publisher) -> None:
        await publisher.publish(Value(value=b"device"))
        await ctx.cancelled()

    @observer("conformance.settings", row=Value, interval=Duration(seconds=1), interval_setting="harmos.conformance.v1.Graph.interval")
    async def settings(self, ctx: Context) -> Value:
        return Value(value=ctx.sequence.to_bytes(8, "little"))


class ClockFixture:
    """Reuse the Clock Journey Without External Device Demand."""

    clock = Fixture.clock


# === Public Functions === #


def plugin() -> Plugin:
    return Plugin(
        Fixture(),
        id="conformance",
        version="1.0.0",
        requires=[HOST_ECHO, HOST_PENDING, HOST_FAIL, HOST_SCHEMA, SLEEP, GET_TIME],
        sections=[Graph(interval=Duration(seconds=2))],
        messages=[PayloadOnly, PayloadOnly],
    )


# === Private Functions === #


async def _main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    parser.add_argument("--clock-only", action="store_true", help="Serve Only the Host Clock Journey")
    parser.add_argument("--shutdown-on-stdin", action="store_true", help="Close When the Process Owner Closes Standard Input")
    options = parser.parse_args()
    selected = Plugin(ClockFixture(), id="conformance", version="1.0.0", requires=[SLEEP, GET_TIME]) if options.clock_only else plugin()
    endpoint = await selected.start(options.address)
    print(f"harmos listening {endpoint.address}", flush=True)
    try:
        if options.shutdown_on_stdin:
            await asyncio.to_thread(sys.stdin.buffer.read)
        else:
            await endpoint.server.wait_for_termination()
    finally:
        await endpoint.close()


# === Entry Point === #

if __name__ == "__main__":
    asyncio.run(_main())
