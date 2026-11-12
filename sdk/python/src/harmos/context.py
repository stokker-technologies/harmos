# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Invocation-Owned Child Calls and Host Clock Access."""

# === Imports === #

# Standard Library Imports
import asyncio

# Third Party Imports
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from .contracts import Operation
from .v1 import plugin_pb2 as wire

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SLEEP = Operation("harmos.clock.sleep", wire.Sleep, wire.SleepResponse)
NOW = Operation("harmos.clock.now", wire.Now, wire.Time)

# === Types === #

# === Classes === #


class Failure(Exception):
    """An Application Failure, Preserved Without Reclassification."""

    def __init__(self, value: wire.Failure):
        super().__init__(value.message)
        self.value = wire.Failure()
        self.value.CopyFrom(value)


class ProtocolError(Exception):
    pass


class Context:
    """Background Work Outside This Context Is Invisible to Virtual Time."""

    def __init__(self, request: wire.Request, plugin: str, requires: dict[str, bytes], output: asyncio.Queue | None = None):
        self.invocation = request.invocation
        self.attempt = request.attempt
        self._scope = request.scope
        self.sequence = 0
        self._operation = request.operation
        self._plugin = plugin
        self._requires = requires
        self._output = output
        self._pending: dict[int, asyncio.Future] = {}
        self._next = 0
        self._cancelled = asyncio.Event()
        self._reason = ""
        self._resources: dict[type, object] = {}

    def scope(self) -> str:
        """Require a Caller-Bound Scope for This Invocation."""
        if not self._scope:
            raise Failure(
                wire.Failure(
                    classification=wire.CLASSIFICATION_REFUSED,
                    code="scope_required",
                    message="Invocation Scope Is Required: the caller must bind a scope",
                )
            )
        return self._scope

    def set[T](self, value: T) -> None:
        self._resources[type(value)] = value

    def get[T](self, kind: type[T]) -> T:
        value = self._resources[kind]
        if not isinstance(value, kind):
            raise TypeError("Invalid Resource Type")
        return value

    async def operation[Q: Message, R: Message](
        self, operation: Operation[Q, R], request: Q, *, policy: wire.OperationPolicy | None = None
    ) -> R:
        if self._output is None:
            raise ValueError("Observers Cannot Call Operations or Wait")
        if not isinstance(request, operation.request):
            raise TypeError("Request Does Not Match the Operation")
        if self._requires.get(operation.id) != operation.fingerprint:
            raise ValueError(f"Undeclared or Incompatible Requirement: {operation.id}")
        if self._cancelled.is_set():
            raise self._cancel_failure()

        self._next += 1
        call = self._next
        complete = asyncio.get_running_loop().create_future()
        self._pending[call] = complete
        child = wire.ChildRequest(
            call=call, operation=operation.id, payload=request.SerializeToString(), fingerprint=operation.fingerprint, policy=policy
        )
        try:
            await self._output.put(wire.PluginFrame(child=child))
            reply = await complete
            if reply.WhichOneof("result") == "failure":
                raise Failure(reply.failure)
            return operation.response.FromString(reply.payload)
        except asyncio.CancelledError:
            if self._pending.pop(call, None) is not None:
                await self._output.put(wire.PluginFrame(child_cancel=wire.ChildCancel(call=call)))
            raise

    async def sleep(self, duration: Duration) -> None:
        await self.operation(SLEEP, wire.Sleep(duration=duration))

    async def now(self) -> Timestamp:
        reply = await self.operation(NOW, wire.Now())
        if not reply.HasField("now"):
            raise ProtocolError("Host Time Missing")
        validate_timestamp(reply.now)
        return reply.now

    async def cancelled(self) -> str:
        await self._cancelled.wait()
        return self._reason

    def _reply(self, reply: wire.ChildResponse) -> None:
        if reply.WhichOneof("result") is None:
            raise ProtocolError("Child Result Missing")
        complete = self._pending.pop(reply.call, None)
        if complete is None or complete.done():
            raise ProtocolError("Unknown or Duplicate Child")
        complete.set_result(reply)

    async def _cancel(self, reason: str) -> None:
        if self._cancelled.is_set():
            return
        self._reason = reason
        self._cancelled.set()
        pending, self._pending = self._pending, {}
        for call, complete in pending.items():
            if self._output is not None:
                await self._output.put(wire.PluginFrame(child_cancel=wire.ChildCancel(call=call)))
            if not complete.done():
                complete.set_exception(self._cancel_failure())

    def _cancel_failure(self) -> Failure:
        return Failure(
            wire.Failure(
                classification=wire.CLASSIFICATION_CANCELLED,
                code="cancelled",
                message=self._reason,
                action="Retry If Needed",
                operation=self._operation,
                plugin=self._plugin,
            )
        )


class Publisher:
    def __init__(self, row: type[Message], output: asyncio.Queue):
        self._row = row
        self._output = output

    async def publish(self, row: Message, *, at: Timestamp | None = None) -> None:
        if not isinstance(row, self._row):
            raise TypeError("Invalid Stream Row")
        if at is not None:
            validate_timestamp(at)
        await self._output.put(wire.Sample(row=row.SerializeToString(), at=at))


# === Public Functions === #


def validate_timestamp(at: Timestamp) -> None:
    if not -62_135_596_800 <= at.seconds <= 253_402_300_799 or not 0 <= at.nanos < 1_000_000_000:
        raise ProtocolError("Invalid Timestamp")


# === Private Functions === #

# === Entry Point === #
