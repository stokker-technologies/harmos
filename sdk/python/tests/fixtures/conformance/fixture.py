# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve the Fixed Conformance Declarations at an Explicit Remote Address."""

# === Imports === #

# Standard Library Imports
import asyncio
from asyncio import Event, Task

# Third Party Imports
from google.protobuf.duration_pb2 import Duration

# Local Imports
from harmos import (
    CLASSIFICATION_PERMANENT,
    CLASSIFICATION_TRANSIENT,
    VISIBILITY_PRIVATE,
    VISIBILITY_PROTECTED,
    VISIBILITY_PUBLIC,
    Failure,
    OperationPolicy,
    RetryPolicy,
    SettingDeclaration,
    SettingValue,
)
from harmos import FailureValue as WireFailure
from harmos._wire.host.v1.invocations_pb2 import InvocationsCreateRequest, InvocationsCreateResponse
from harmos.plugin import Context, HostOperation, Operation, Plugin, Publisher, TimeGetResponse, observer, operation, publisher

from . import Graph, PayloadOnly, Value
from ._caller import REQUEST

# Type Checking Imports

# === Public Exports === #

# === Constants === #

LOCAL = Operation("conformance.local", Value, Value, visibility=VISIBILITY_PRIVATE)
HOST_WATCH = Operation("conformance.host.watch", Value, Value, visibility=VISIBILITY_PUBLIC, watch=True)
HOST_ECHO = Operation("conformance.host.echo", Value, Value, visibility=VISIBILITY_PUBLIC)
HOST_PENDING = Operation("conformance.host.pending", Value, Value, visibility=VISIBILITY_PUBLIC)
HOST_FAIL = Operation("conformance.host.fail", Value, Value, visibility=VISIBILITY_PUBLIC)
PEER_PUBLIC = Operation("conformance.peer.public", Value, Value, visibility=VISIBILITY_PUBLIC)
PEER_PROTECTED = Operation("conformance.peer.protected", Value, Value, visibility=VISIBILITY_PROTECTED)
PEER_LOCAL = Operation("conformance.peer.local", Value, Value, visibility=VISIBILITY_PROTECTED)
OWN_PROTECTED = Operation("example.plugin.v1.Lifecycle/Initialize", Value, Value, visibility=VISIBILITY_PROTECTED)
HOST_SCHEMA = Operation("conformance.host.schema", Graph, Value, visibility=VISIBILITY_PUBLIC)
CREATE_INVOCATION = Operation(
    "harmos.host.v1.Invocations/Create", InvocationsCreateRequest, InvocationsCreateResponse, visibility=VISIBILITY_PUBLIC
)

# === Types === #

# === Classes === #


class Fixture:
    def __init__(self) -> None:
        self.echoes = 0

    @operation("conformance.local", request=Value, response=Value, visibility=VISIBILITY_PRIVATE, replay_safe=True)
    async def local(self, context: Context, request: Value) -> Value:
        if request.value in (b"deadline", b"overall"):
            await context.host.time.sleep(Duration(seconds=3600))
            return request
        if request.value == b"cancel":
            await context.call(HOST_ECHO, request)
            return Value(value=(await context.cancelled()).encode())
        if request.value in (b"retry", b"retry-bound"):
            if context.attempt == 1:
                raise Failure(WireFailure(classification=CLASSIFICATION_TRANSIENT, code="retry", message="Retry"))
            return Value(value=str(context.attempt).encode())
        return request

    @operation("conformance.local_call", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def local_call(self, context: Context, request: Value) -> Value:
        policy = {
            b"retry": OperationPolicy(
                retries=RetryPolicy(total_attempts=2, initial=Duration(seconds=1), maximum=Duration(seconds=4), deterministic_jitter=True)
            ),
            b"deadline": OperationPolicy(timeout=Duration(seconds=3), attempt_timeout=Duration(seconds=2)),
            b"cancel": OperationPolicy(cancellation_timeout=Duration(seconds=1)),
            b"overall": OperationPolicy(timeout=Duration(seconds=2), cancellation_timeout=Duration(seconds=1)),
            b"retry-bound": OperationPolicy(
                timeout=Duration(seconds=1), retries=RetryPolicy(total_attempts=2, initial=Duration(seconds=3600))
            ),
        }.get(request.value)
        return await context.call(LOCAL, request, policy=policy)

    @operation("conformance.step.0", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def echo(self, ctx: Context, request: Value) -> Value:
        self.echoes += 1
        if request.value == b"scope":
            return Value(value=ctx.scope().encode())
        return request

    @operation("conformance.children", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def children(self, ctx: Context, request: Value) -> Value:
        first, second = await asyncio.gather(ctx.call(HOST_ECHO, Value(value=b"\x01")), ctx.call(HOST_ECHO, Value(value=b"\x02")))
        return Value(value=first.value + second.value)

    @operation("conformance.cancel", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def cancel(self, ctx: Context, request: Value) -> Value:
        return await ctx.call(HOST_PENDING, request)

    @operation("conformance.failure", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def failure(self, ctx: Context, request: Value) -> Value:
        raise Failure(
            WireFailure(
                classification=CLASSIFICATION_PERMANENT,
                code="fixture_failure",
                message="Fixture Failed",
                action="Use Another Input",
                operation="conformance.failure",
                plugin="conformance",
                cause=WireFailure(
                    classification=CLASSIFICATION_TRANSIENT,
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

    @operation("conformance.child_failure", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def child_failure(self, ctx: Context, request: Value) -> Value:
        return await ctx.call(HOST_FAIL, request)

    @operation("conformance.policy", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def policy(self, ctx: Context, request: Value) -> Value:
        first = await ctx.call(
            HOST_ECHO,
            request,
            policy=OperationPolicy(
                retries=RetryPolicy(total_attempts=3, initial=Duration(seconds=1), maximum=Duration(seconds=4), deterministic_jitter=True)
            ),
        )
        return await ctx.call(HOST_ECHO, first)

    @operation("conformance.schema", request=Graph, response=Graph.Nested, visibility=VISIBILITY_PUBLIC)
    async def schema(self, ctx: Context, request: Graph) -> Graph.Nested:
        return Graph.Nested(count=len(request.items), next=request)

    @operation("conformance.nested", request=Graph.Nested, response=Graph, visibility=VISIBILITY_PUBLIC)
    async def nested(self, ctx: Context, request: Graph.Nested) -> Graph:
        return request.next

    @operation("conformance.projected", request=Graph, response=Value, visibility=VISIBILITY_PUBLIC)
    async def projected(self, ctx: Context, request: Graph) -> Value:
        return request.tail.value

    @operation("conformance.clock", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def clock(self, ctx: Context, request: Value) -> Value:
        before = await ctx.host.time.get()
        await ctx.host.time.sleep(Duration(seconds=3600))
        after = await ctx.host.time.get()
        return Value(value=(after.seconds - before.seconds).to_bytes(8, "little", signed=True))

    @operation("example.plugin.v1.Lifecycle/Initialize", request=Value, response=Value, visibility=VISIBILITY_PROTECTED)
    async def protected(self, ctx: Context, request: Value) -> Value:
        return request

    @operation("example.plugin.v1.Steps/Execute", request=Value, response=Value, visibility=VISIBILITY_PUBLIC, replay_safe=True)
    async def resources(self, ctx: Context, request: Value) -> Value:
        ctx.set(request)
        await ctx.call(HOST_ECHO, request)
        return ctx.get(Value)

    @operation("conformance.cancel_reason", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def cancel_reason(self, ctx: Context, request: Value) -> Value:
        await ctx.call(HOST_ECHO, request)
        return Value(value=(await ctx.cancelled()).encode())

    @operation("conformance.watch", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def watch(self, ctx: Context, request: Value) -> Value:
        value = b""
        items = ctx.watch(HOST_WATCH, request, max_item_bytes=4096)
        try:
            async for item in items:
                value += item.value
                if request.value == b"cancel":
                    break
        finally:
            await items.aclose()
        return Value(value=value)

    @operation("conformance.peer_call", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def peer_call(self, context: Context, request: Value) -> Value:
        if request.value == b"self":
            return await context.call(OWN_PROTECTED, request)
        if request.value == b"public":
            return await context.call(PEER_PUBLIC, Value(value=context.scope().encode()))
        return await context.call(PEER_PROTECTED if request.value == b"protected" else PEER_LOCAL, request)

    @operation("conformance.settings_read", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def settings_read(self, context: Context, request: Value) -> Value:
        settings = await context.host.plugins.settings.list()
        return Value(value=settings.value.SerializeToString())

    @operation("conformance.discover", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def discover(self, context: Context, request: Value) -> Value:
        inventory = await context.host.operations.list()
        if not inventory:
            raise ValueError("Discover Operations: Empty Inventory")
        operation = inventory[0]
        selected = await context.host.operations.get(operation.contract.id)
        if selected != operation:
            raise ValueError("Get Operation: Contract Differs from Inventory")
        return Value(value=operation.contract.id.encode())

    @operation("conformance.caller_refusal", request=Value, response=Value, visibility=VISIBILITY_PUBLIC)
    async def caller_refusal(self, context: Context, request: Value) -> Value:
        create = InvocationsCreateRequest.FromString(REQUEST)
        try:
            await context.call(CREATE_INVOCATION, create)
        except Failure as failure:
            return Value(value=failure.value.SerializeToString())
        raise RuntimeError("Create Invocation: Client-Only Host Operation Was Admitted")

    @observer("conformance.ticks", row=Value, interval=Duration(seconds=1))
    async def ticks(self, ctx: Context) -> Value:
        return Value(value=ctx.sequence.to_bytes(8, "little"))

    @publisher("conformance.device", row=Value)
    async def device(self, ctx: Context, publisher: Publisher) -> None:
        await publisher.publish(Value(value=b"device"))
        await ctx.cancelled()

    @observer("conformance.settings", row=Value, interval=Duration(seconds=1), interval_setting="interval")
    async def settings(self, ctx: Context) -> Value:
        return Value(value=ctx.sequence.to_bytes(8, "little"))

    @observer("conformance.tick_children", row=Value, interval=Duration(seconds=1))
    async def tick_children(self, context: Context) -> Value:
        first, second = await asyncio.gather(context.call(HOST_ECHO, Value(value=b"\x01")), context.call(HOST_ECHO, Value(value=b"\x02")))
        return Value(value=first.value + second.value)

    @observer("conformance.tick_context", row=Value, interval=Duration(seconds=1))
    async def tick_context(self, context: Context) -> Value:
        echo = await context.call(HOST_ECHO, Value(value=context.scope().encode()))
        clock = TimeGetResponse(now=await context.host.time.get())
        settings = await context.host.plugins.settings.list()
        value = b""
        for reply in (echo, clock, settings):
            payload = reply.SerializeToString()
            value += len(payload).to_bytes(4, "little") + payload
        return Value(value=value)

    @observer("conformance.tick_failure", row=Value, interval=Duration(seconds=1))
    async def tick_failure(self, context: Context) -> Value:
        try:
            return await context.call(HOST_FAIL, Value())
        except Failure as failure:
            return Value(value=failure.value.SerializeToString())

    @observer("conformance.tick_watch", row=Value, interval=Duration(seconds=1))
    async def tick_watch(self, context: Context) -> Value:
        value = b""
        items = context.watch(HOST_WATCH, Value(value=context.scope().encode()), max_item_bytes=4096)
        try:
            async for item in items:
                value += item.value
        finally:
            await items.aclose()
        return Value(value=value)

    @observer("conformance.tick_cancel", row=Value, interval=Duration(seconds=1))
    async def tick_cancel(self, context: Context) -> Value:
        started = Event()

        async def pending() -> Value:
            started.set()
            try:
                return await context.call(HOST_PENDING, Value())
            except Failure:
                return Value()

        task = asyncio.create_task(pending())
        await started.wait()
        try:
            await context.call(HOST_ECHO, Value(value=b"release"))
        finally:
            task.cancel()
            await asyncio.gather(task, return_exceptions=True)
        started.clear()
        automatic = asyncio.create_task(pending())
        automatic.add_done_callback(_settled)
        await started.wait()
        return Value(value=b"cancelled")

    @observer("conformance.tick_caller_refusal", row=Value, interval=Duration(seconds=1))
    async def tick_caller_refusal(self, context: Context) -> Value:
        return await self.caller_refusal(context, Value())


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
            CREATE_INVOCATION,
            HostOperation.TIME_SLEEP,
            HostOperation.TIME_GET,
            HostOperation.PLUGINS_SETTINGS_LIST,
            HostOperation.OPERATIONS_LIST,
            HostOperation.OPERATIONS_GET,
        ],
        sections=[Graph(interval=Duration(seconds=2))],
        messages=[PayloadOnly, PayloadOnly],
        settings=[
            SettingDeclaration(name=name, title=name, description=f"Conformance {name}", default=value)
            for name, value in (
                ("count", SettingValue(integer=3)),
                ("enabled", SettingValue(flag=False)),
                ("gain", SettingValue(number=1.5)),
                ("interval", SettingValue(duration=Duration(seconds=2))),
                ("label", SettingValue(text="default")),
            )
        ],
    )


# === Private Functions === #


def _settled(task: Task[Value]) -> None:
    if not task.cancelled():
        task.exception()
