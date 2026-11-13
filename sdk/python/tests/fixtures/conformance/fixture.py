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
import asyncio

# Third Party Imports
from google.protobuf.duration_pb2 import Duration

# Local Imports
from harmos import GET_TIME, SLEEP, Context, Failure, Operation, Plugin, Publisher, observer, operation, publisher
from harmos.v1 import types_pb2 as wire

from .v1.fixture_pb2 import Graph, PayloadOnly, Value

# Type Checking Imports

# === Public Exports === #

# === Constants === #

LOCAL = Operation("conformance.local", Value, Value, visibility=wire.VISIBILITY_PRIVATE)
HOST_WATCH = Operation("conformance.host.watch", Value, Value, visibility=wire.VISIBILITY_PUBLIC, watch=True)
HOST_ECHO = Operation("conformance.host.echo", Value, Value, visibility=wire.VISIBILITY_PUBLIC)
HOST_PENDING = Operation("conformance.host.pending", Value, Value, visibility=wire.VISIBILITY_PUBLIC)
HOST_FAIL = Operation("conformance.host.fail", Value, Value, visibility=wire.VISIBILITY_PUBLIC)
PEER_PUBLIC = Operation("conformance.peer.public", Value, Value, visibility=wire.VISIBILITY_PUBLIC)
PEER_PROTECTED = Operation("conformance.peer.protected", Value, Value, visibility=wire.VISIBILITY_PROTECTED)
PEER_LOCAL = Operation("conformance.peer.local", Value, Value, visibility=wire.VISIBILITY_PROTECTED)
OWN_PROTECTED = Operation("example.plugin.v1.Lifecycle/Initialize", Value, Value, visibility=wire.VISIBILITY_PROTECTED)
HOST_SCHEMA = Operation("conformance.host.schema", Graph, Value, visibility=wire.VISIBILITY_PUBLIC)

# === Types === #

# === Classes === #


class Fixture:
    def __init__(self) -> None:
        self.echoes = 0

    @operation("conformance.local", request=Value, response=Value, visibility=wire.VISIBILITY_PRIVATE, replay_safe=True)
    async def local(self, context: Context, request: Value) -> Value:
        if request.value in (b"deadline", b"overall"):
            await context.sleep(Duration(seconds=3600))
            return request
        if request.value == b"cancel":
            await context.call(HOST_ECHO, request)
            return Value(value=(await context.cancelled()).encode())
        if request.value in (b"retry", b"retry-bound"):
            if context.attempt == 1:
                raise Failure(wire.Failure(classification=wire.CLASSIFICATION_TRANSIENT, code="retry", message="Retry"))
            return Value(value=str(context.attempt).encode())
        return request

    @operation("conformance.local_call", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def local_call(self, context: Context, request: Value) -> Value:
        policy = {
            b"retry": wire.OperationPolicy(
                retries=wire.RetryPolicy(
                    total_attempts=2, initial=Duration(seconds=1), maximum=Duration(seconds=4), deterministic_jitter=True
                )
            ),
            b"deadline": wire.OperationPolicy(timeout=Duration(seconds=3), attempt_timeout=Duration(seconds=2)),
            b"cancel": wire.OperationPolicy(cancellation_timeout=Duration(seconds=1)),
            b"overall": wire.OperationPolicy(timeout=Duration(seconds=2), cancellation_timeout=Duration(seconds=1)),
            b"retry-bound": wire.OperationPolicy(
                timeout=Duration(seconds=1), retries=wire.RetryPolicy(total_attempts=2, initial=Duration(seconds=3600))
            ),
        }.get(request.value)
        return await context.call(LOCAL, request, policy=policy)

    @operation("conformance.step.0", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
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

    @operation("example.plugin.v1.Lifecycle/Initialize", request=Value, response=Value, visibility=wire.VISIBILITY_PROTECTED)
    async def protected(self, ctx: Context, request: Value) -> Value:
        return request

    @operation("example.plugin.v1.Steps/Execute", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC, replay_safe=True)
    async def resources(self, ctx: Context, request: Value) -> Value:
        ctx.set(request)
        await ctx.call(HOST_ECHO, request)
        return ctx.get(Value)

    @operation("conformance.cancel_reason", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def cancel_reason(self, ctx: Context, request: Value) -> Value:
        await ctx.call(HOST_ECHO, request)
        return Value(value=(await ctx.cancelled()).encode())

    @operation("conformance.watch", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def watch(self, ctx: Context, request: Value) -> Value:
        value = b""
        async with ctx.watch(HOST_WATCH, request, max_item_bytes=4096) as items:
            async for item in items:
                value += item.value
                if request.value == b"cancel":
                    break
        return Value(value=value)

    @operation("conformance.peer_call", request=Value, response=Value, visibility=wire.VISIBILITY_PUBLIC)
    async def peer_call(self, context: Context, request: Value) -> Value:
        if request.value == b"self":
            return await context.call(OWN_PROTECTED, request)
        if request.value == b"public":
            return await context.call(PEER_PUBLIC, Value(value=context.scope().encode()))
        return await context.call(PEER_PROTECTED if request.value == b"protected" else PEER_LOCAL, request)

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
        requires=[
            PEER_PUBLIC,
            PEER_PROTECTED,
            PEER_LOCAL,
            OWN_PROTECTED,
            HOST_WATCH,
            HOST_ECHO,
            HOST_PENDING,
            HOST_FAIL,
            HOST_SCHEMA,
            SLEEP,
            GET_TIME,
        ],
        sections=[Graph(interval=Duration(seconds=2))],
        messages=[PayloadOnly, PayloadOnly],
    )


# === Private Functions === #
