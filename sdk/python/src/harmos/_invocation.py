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
import json
from asyncio import CancelledError, Event, Future, Queue
from collections.abc import AsyncGenerator, Callable, Coroutine
from typing import TYPE_CHECKING, Protocol

# Third Party Imports
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from harmos._wire.host.v1.plugins.settings_pb2 import SettingsListResponse
from harmos._wire.model.v1 import failure_pb2
from harmos._wire.model.v1.failure_pb2 import (
    CLASSIFICATION_CANCELLED,
    CLASSIFICATION_REFUSED,
)
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PUBLIC
from harmos._wire.model.v1.policy_pb2 import OperationPolicy
from harmos._wire.model.v1.position_pb2 import ReadCut
from harmos._wire.plugin.v1.invocation_pb2 import ChildCancel, ChildDemand, ChildRequest, ChildResponse, Request
from harmos._wire.plugin.v1.stream_pb2 import Sample

# Local Imports
from ._contracts import Operation, message_types
from ._generated.host import _SUBSCRIPTIONS_SUBSCRIBE, HostPluginsStateResource, HostRoot
from ._subscription import Subscription
from ._wire.plugin.v1.subscriptions_pb2 import SubscriptionsSubscribeRequest

# Type Checking Imports
if TYPE_CHECKING:
    from ._authoring import _FaultError

from harmos._wire.host.v1.time_pb2 import TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceInvokeResponse, InstanceSamplesResponse

# === Public Exports === #

# === Constants === #

SLEEP = Operation("harmos.plugin.v1.Time/Sleep", TimeSleepRequest, TimeSleepResponse, visibility=VISIBILITY_PUBLIC)
GET_TIME = Operation("harmos.plugin.v1.Time/Get", TimeGetRequest, TimeGetResponse, visibility=VISIBILITY_PUBLIC)


# === Types === #

type Handler = Callable[[Context, Message], Coroutine[object, object, Message]]

# === Classes === #


class OperationError(Exception):
    """An Operation Failure With the Host Scheduler's Retry Evidence."""

    def __init__(
        self,
        kind: str | failure_pb2.Failure = "permanent",
        *,
        code: str = "",
        message: str = "",
        action: str = "",
        operation: str = "",
        retry: failure_pb2.OperationRetry | None = None,
    ) -> None:
        self.value = failure_pb2.Failure()
        if isinstance(kind, failure_pb2.Failure):
            self.value.CopyFrom(kind)
        else:
            classification = {
                "transient": failure_pb2.CLASSIFICATION_TRANSIENT,
                "permanent": failure_pb2.CLASSIFICATION_PERMANENT,
                "refused": failure_pb2.CLASSIFICATION_REFUSED,
                "invalid": failure_pb2.CLASSIFICATION_INVALID,
                "cancelled": failure_pb2.CLASSIFICATION_CANCELLED,
            }
            self.value.CopyFrom(
                failure_pb2.Failure(
                    classification=classification[kind],
                    code=code,
                    message=message,
                    action=action,
                    operation=operation,
                    retry=retry,
                )
            )
        super().__init__(self.value.message)

    @property
    def kind(self) -> str:
        return {
            failure_pb2.CLASSIFICATION_TRANSIENT: "transient",
            failure_pb2.CLASSIFICATION_REFUSED: "refused",
            failure_pb2.CLASSIFICATION_INVALID: "invalid",
            failure_pb2.CLASSIFICATION_CANCELLED: "cancelled",
            failure_pb2.CLASSIFICATION_TIMED_OUT: "cancelled",
        }.get(self.value.classification, "permanent")

    @property
    def code(self) -> str:
        return self.value.code

    @property
    def message(self) -> str:
        return self.value.message

    @property
    def action(self) -> str:
        return self.value.action

    @property
    def operation(self) -> str:
        return self.value.operation

    @property
    def retry(self) -> failure_pb2.OperationRetry:
        return self.value.retry

    def to_dict(self) -> dict[str, object]:
        return {
            "kind": self.kind,
            "code": self.code,
            "message": self.message,
            "action": self.action,
            "operation": self.operation,
            "retry": {
                "retryable": self.retry.retryable,
                "attempts": self.retry.attempts,
                "max_attempts": self.retry.max_attempts,
                "next_after_s": self.retry.next_after_s,
            },
        }


class ProtocolError(Exception):
    pass


class _Output(Protocol):
    async def put(self, frame: "InstanceInvokeResponse | _FaultError", /) -> None: ...


class _Watching[R: Message]:
    def __init__(self, context: "Context", row: type[R], output: _Output, call: int, queue: Queue[ChildResponse]) -> None:
        self.context = context
        self.row = row
        self.output = output
        self.call = call
        self.queue = queue
        self.closed = False
        self.reading = False

    def __aiter__(self) -> "_Watching[R]":
        return self

    async def __anext__(self) -> R:
        if self.closed:
            raise StopAsyncIteration
        if self.reading:
            raise ValueError("Read Watch: Await the Outstanding Read Before Reading Again")
        self.reading = True
        try:
            if self.call in self.context._watches:
                await self.output.put(InstanceInvokeResponse(child_demand=ChildDemand(call=self.call)))
            reply = await self.queue.get()
            if self.closed:
                raise StopAsyncIteration
            kind = reply.WhichOneof("result")
            if kind == "failure":
                await self.aclose()
                raise OperationError(reply.failure)
            if kind == "payload":
                await self.aclose()
                raise StopAsyncIteration
            return self.row.FromString(reply.item)
        finally:
            self.reading = False

    async def aclose(self) -> None:
        if self.closed:
            return
        self.closed = True
        if self.context._watches.pop(self.call, None) is not None:
            self.context._retired.add(self.call)
            await self.output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=self.call)))
        if self.queue.empty():
            self.queue.put_nowait(ChildResponse(call=self.call, payload=b""))


class Context:
    """Background Work Outside This Context Is Invisible to Virtual Time."""

    def __init__(
        self,
        request: Request,
        plugin: str,
        requires: dict[str, bytes],
        output: "_Output | None" = None,
    ) -> None:
        self.invocation = request.invocation
        self.capturing_executions = tuple(request.capturing_executions)
        self.attempt = request.attempt
        self._scope = request.scope
        self._wind_down = Event()
        self.sequence = 0
        self._operation = request.operation
        self._plugin = plugin
        self._requires = requires
        self._output = output
        self._pending: dict[int, Future[ChildResponse]] = {}
        self._next = [0]
        self._watches: dict[int, tuple[Queue[ChildResponse], int]] = {}
        self._retired: set[int] = set()
        self._cancelled = Event()
        self._reason = ""
        self._resources: dict[type, object] = {}
        self._instance: object | None = None

    @property
    def plugin(self) -> object:
        """The Plugin Instance Shared by Lifecycle Hooks, Operations and Streams."""
        if self._instance is None:
            raise ValueError("Read Plugin State: Use a Context Created by the Plugin Server")
        return self._instance

    @property
    def wind_down_requested(self) -> bool:
        """The Host Requested Normal Completion of This Scope's Work."""
        return self._wind_down.is_set()

    def scope(self) -> str:
        """Require a Caller-Bound Scope for This Invocation."""
        if not self._scope:
            raise OperationError(
                failure_pb2.Failure(
                    classification=CLASSIFICATION_REFUSED,
                    code="scope_required",
                    message="Invocation Scope Is Required: the caller must bind a scope",
                )
            )
        return self._scope

    @property
    def host(self) -> HostRoot:
        return HostRoot(self)

    @property
    def state(self) -> HostPluginsStateResource:
        """Read and Durably Replace This Entry's Opaque State With an Expected Revision."""
        return self.host.plugins.state

    def set[T](self, value: T) -> None:
        self._resources[type(value)] = value

    def get[T](self, kind: type[T]) -> T:
        value = self._resources[kind]
        if not isinstance(value, kind):
            raise TypeError("Invalid Resource Type")
        return value

    async def _call[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, *, policy: OperationPolicy | None = None) -> R:
        if self._output is None:
            raise ValueError(f"Call {operation.id}: No Active Invocation or Unanswered Window")
        if operation.watch:
            raise ValueError(f"Call {operation.id}: Use Watch for a Streaming Contract")
        if not isinstance(request, operation.request):
            raise TypeError("Request Does Not Match the Operation")
        child = ChildRequest(
            operation=operation.id,
            payload=request.SerializeToString(),
            fingerprint=operation.fingerprint,
            policy=policy,
        )
        reply = await self._send(child)
        return operation.response.FromString(reply)

    async def operation(
        self, operation: str, payload: dict[str, object] | None = None, *, policy: OperationPolicy | None = None
    ) -> dict[str, object]:
        """Call a Served Operation by Identifier; the Host Converts Its JSON Payload."""
        child = ChildRequest(operation=operation, json=json.dumps({} if payload is None else payload, allow_nan=False), policy=policy)
        reply = await self._send(child)
        value = json.loads(reply)
        if not isinstance(value, dict):
            raise ProtocolError(f"Call {operation}: Host Must Return a JSON Object")
        return value

    async def _send(self, child: ChildRequest) -> bytes:
        if self._output is None:
            raise ValueError(f"Call {child.operation}: No Active Invocation or Unanswered Window")
        if self._cancelled.is_set():
            raise self._cancel_failure()
        self._next[0] += 1
        call = self._next[0]
        child.call = call
        complete: Future[ChildResponse] = asyncio.get_running_loop().create_future()
        self._pending[call] = complete
        try:
            await self._output.put(InstanceInvokeResponse(child=child))
            reply = await complete
            if reply.WhichOneof("result") == "failure":
                raise OperationError(reply.failure)
            return reply.payload
        except CancelledError:
            if self._pending.pop(call, None) is not None:
                self._retired.add(call)
                await self._output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=call)))
            raise

    async def subscribe[R: Message](self, stream: str, row: type[R], *, max_item_bytes: int) -> Subscription[R]:
        request = SubscriptionsSubscribeRequest(stream=stream, row=row.DESCRIPTOR.full_name, types=message_types(row).SerializeToString())
        incoming = await self._open_watch(_SUBSCRIPTIONS_SUBSCRIBE, request, max_item_bytes=max_item_bytes)
        try:
            opened = await anext(incoming)
            if opened.WhichOneof("item") != "opened" or not opened.opened:
                raise ValueError("Subscribe to Stream: Host Must Acknowledge the Admitted Demand")
        except BaseException:
            await incoming.aclose()
            raise
        return Subscription(incoming, row)

    async def watch[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, *, max_item_bytes: int) -> AsyncGenerator[R]:
        incoming = await self._open_watch(operation, request, max_item_bytes=max_item_bytes)
        try:
            async for item in incoming:
                yield item
        finally:
            await incoming.aclose()

    async def _open_watch[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, *, max_item_bytes: int) -> "_Watching[R]":
        if self._cancelled.is_set():
            raise self._cancel_failure()
        output = self._output
        if output is None or self._requires.get(operation.id) != operation.fingerprint:
            raise ValueError(f"Watch {operation.id}: Undeclared Requirement or Missing Invocation")
        if not operation.watch:
            raise ValueError(f"Watch {operation.id}: Server-Streaming Contract Required")
        if max_item_bytes <= 0:
            raise ValueError(f"Watch {operation.id}: Positive Item Limit Required")
        if not isinstance(request, operation.request):
            raise TypeError(f"Watch {operation.id}: Request Does Not Match the Operation")
        self._next[0] += 1
        call = self._next[0]
        queue: Queue[ChildResponse] = Queue(maxsize=1)
        self._watches[call] = (queue, max_item_bytes)
        await output.put(
            InstanceInvokeResponse(
                child=ChildRequest(
                    call=call,
                    operation=operation.id,
                    payload=request.SerializeToString(),
                    fingerprint=operation.fingerprint,
                    watch=True,
                    max_response_bytes=max_item_bytes,
                )
            )
        )

        return _Watching(self, operation.response, output, call, queue)

    async def settings(self, *, at: ReadCut | None = None) -> SettingsListResponse:
        """Read the Calling Plugin's Complete Settings and Revision."""
        return await self.host.plugins.settings.list(at=at)

    async def sleep(self, duration: Duration) -> None:
        await self._call(SLEEP, TimeSleepRequest(duration=duration))

    async def now(self) -> Timestamp:
        reply = await self._call(GET_TIME, TimeGetRequest())
        if not reply.HasField("now"):
            raise ProtocolError("Read Host Time: Missing Value")
        validate_timestamp(reply.now)
        return reply.now

    async def cancelled(self) -> str:
        await self._cancelled.wait()
        return self._reason

    async def _reply(self, reply: ChildResponse) -> None:
        if reply.call in self._retired:
            return
        if reply.WhichOneof("result") is None:
            raise ProtocolError("Read Child Result: Missing Payload")
        watching = self._watches.get(reply.call)
        if watching is not None:
            queue, limit = watching
            if len(reply.item) > limit or len(reply.payload) > limit:
                raise ProtocolError("Watch: Item Exceeds Consumer Limit")
            if reply.WhichOneof("result") != "item":
                self._watches.pop(reply.call)
            await queue.put(reply)
            return
        complete = self._pending.pop(reply.call, None)
        if complete is None or complete.done():
            raise ProtocolError("Reply to Child: Unknown or Duplicate Identity")
        complete.set_result(reply)

    async def _cancel(self, reason: str) -> None:
        if self._cancelled.is_set():
            return
        self._reason = reason
        self._cancelled.set()
        watches, self._watches = self._watches, {}
        for call, (queue, _) in watches.items():
            self._retired.add(call)
            if self._output is not None:
                await self._output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=call)))
            if queue.full():
                queue.get_nowait()
            queue.put_nowait(ChildResponse(call=call, failure=self._cancel_failure().value))
        pending, self._pending = self._pending, {}
        for call, complete in pending.items():
            self._retired.add(call)
            if self._output is not None:
                await self._output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=call)))
            if not complete.done():
                complete.set_exception(self._cancel_failure())

    def _cancel_failure(self) -> OperationError:
        return OperationError(
            failure_pb2.Failure(
                classification=CLASSIFICATION_CANCELLED,
                code="cancelled",
                message=self._reason,
                action="Retry If Needed",
                operation=self._operation,
                plugin=self._plugin,
            )
        )


class Publisher:
    def __init__(self, row: type[Message], output: "Queue[Sample | InstanceSamplesResponse | _FaultError | None]") -> None:
        self._row = row
        self._output = output

    async def publish(self, row: Message, *, at: Timestamp | None = None) -> None:
        if not isinstance(row, self._row):
            raise TypeError("Invalid Stream Row")
        if at is not None:
            validate_timestamp(at)
        await self._output.put(Sample(row=row.SerializeToString(), at=at))


# === Public Functions === #


def validate_timestamp(at: Timestamp) -> None:
    if not -62_135_596_800 <= at.seconds <= 253_402_300_799 or not 0 <= at.nanos < 1_000_000_000:
        raise ProtocolError("Validate Timestamp: Invalid Value")


# === Private Functions === #

# === Entry Point === #
