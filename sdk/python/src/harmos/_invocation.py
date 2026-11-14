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
import copy
from collections.abc import AsyncGenerator, AsyncIterator, Callable, Coroutine
from typing import TYPE_CHECKING, Protocol

# Third Party Imports
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from harmos._wire.host.v1.plugins.settings_pb2 import SettingsListResponse
from harmos._wire.model.v1.failure_pb2 import (
    CLASSIFICATION_CANCELLED,
    CLASSIFICATION_REFUSED,
    CLASSIFICATION_TIMED_OUT,
    CLASSIFICATION_TRANSIENT,
)
from harmos._wire.model.v1.failure_pb2 import Failure as WireFailure
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PUBLIC
from harmos._wire.model.v1.policy_pb2 import OperationPolicy
from harmos._wire.model.v1.position_pb2 import ReadCut
from harmos._wire.plugin.v1.invocation_pb2 import ChildCancel, ChildDemand, ChildRequest, ChildResponse, Request
from harmos._wire.plugin.v1.stream_pb2 import Sample

# Local Imports
from ._contracts import Operation

# Type Checking Imports
if TYPE_CHECKING:
    from ._authoring import _FaultError

from asyncio import FIRST_COMPLETED, CancelledError, Event, Future, Queue

from harmos._wire.host.v1.time_pb2 import TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse
from harmos._wire.plugin.v1.instance_pb2 import InstanceInvokeResponse, InstanceSamplesResponse

from ._generated.host import HostRoot

# === Public Exports === #

# === Constants === #

SLEEP = Operation("harmos.host.v1.Time/Sleep", TimeSleepRequest, TimeSleepResponse, visibility=VISIBILITY_PUBLIC)
GET_TIME = Operation("harmos.host.v1.Time/Get", TimeGetRequest, TimeGetResponse, visibility=VISIBILITY_PUBLIC)


# === Types === #

type Handler = Callable[[Context, Message], Coroutine[object, object, Message]]

# === Classes === #


class FailureError(Exception):
    """An Application Failure, Preserved Without Reclassification."""

    def __init__(self, value: WireFailure) -> None:
        super().__init__(value.message)
        self.value = WireFailure()
        self.value.CopyFrom(value)


class ProtocolError(Exception):
    pass


class _Output(Protocol):
    async def put(self, frame: "InstanceInvokeResponse | _FaultError", /) -> None: ...


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
        self.attempt = request.attempt
        self._scope = request.scope
        self.sequence = 0
        self._operation = request.operation
        self._plugin = plugin
        self._requires = requires
        self._output = output
        self._pending: dict[int, Future[ChildResponse]] = {}
        self._next = [0]
        self._local: dict[str, tuple[Operation[Message, Message], Handler]] = {}
        self._watches: dict[int, tuple[Queue[ChildResponse], int]] = {}
        self._background = False
        self._retired: set[int] = set()
        self._children: list[Context] = []
        self._cancelled = Event()
        self._reason = ""
        self._resources: dict[type, object] = {}

    def scope(self) -> str:
        """Require a Caller-Bound Scope for This Invocation."""
        if not self._scope:
            raise FailureError(
                WireFailure(
                    classification=CLASSIFICATION_REFUSED,
                    code="scope_required",
                    message="Invocation Scope Is Required: the caller must bind a scope",
                )
            )
        return self._scope

    @property
    def host(self) -> HostRoot:
        return HostRoot(self)

    def set[T](self, value: T) -> None:
        self._resources[type(value)] = value

    def get[T](self, kind: type[T]) -> T:
        value = self._resources[kind]
        if not isinstance(value, kind):
            raise TypeError("Invalid Resource Type")
        return value

    async def call[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, *, policy: OperationPolicy | None = None) -> R:
        if self._output is None:
            raise ValueError(f"Call {operation.id}: No Active Invocation or Unanswered Tick")
        if operation.watch:
            raise ValueError(f"Call {operation.id}: Use Watch for a Streaming Contract")
        if not isinstance(request, operation.request):
            raise TypeError("Request Does Not Match the Operation")
        if operation.id in self._local:
            return await self._invoke_local(operation, request, policy)
        if self._requires.get(operation.id) != operation.fingerprint:
            raise ValueError(f"Undeclared or Incompatible Requirement: {operation.id}")
        if self._cancelled.is_set():
            raise self._cancel_failure()

        self._next[0] += 1
        call = self._next[0]
        complete: Future[ChildResponse] = asyncio.get_running_loop().create_future()
        self._pending[call] = complete
        child = ChildRequest(
            call=call,
            operation=operation.id,
            payload=request.SerializeToString(),
            fingerprint=operation.fingerprint,
            policy=policy,
            background=self._background,
        )
        try:
            await self._output.put(InstanceInvokeResponse(child=child))
            reply = await complete
            if reply.WhichOneof("result") == "failure":
                raise FailureError(reply.failure)
            return operation.response.FromString(reply.payload)
        except CancelledError:
            if self._pending.pop(call, None) is not None:
                self._retired.add(call)
                await self._output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=call)))
            raise

    async def watch[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, *, max_item_bytes: int) -> AsyncGenerator[R]:
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

        async def items() -> AsyncIterator[R]:
            while True:
                await output.put(InstanceInvokeResponse(child_demand=ChildDemand(call=call)))
                reply = await queue.get()
                kind = reply.WhichOneof("result")
                if kind == "failure":
                    raise FailureError(reply.failure)
                if kind == "payload":
                    return
                yield operation.response.FromString(reply.item)

        try:
            async for item in items():
                yield item
        finally:
            if self._watches.pop(call, None) is not None:
                await output.put(InstanceInvokeResponse(child_cancel=ChildCancel(call=call)))

    async def _invoke_local[Q: Message, R: Message](self, operation: Operation[Q, R], request: Q, override: OperationPolicy | None) -> R:
        definition, handler = self._local[operation.id]
        if operation.fingerprint != definition.fingerprint:
            raise ValueError(f"Invoke {operation.id}: Local Contract Differs")
        policy = override if override is not None else definition.policy
        policy = policy if policy is not None else OperationPolicy()
        context = copy.copy(self)
        context._operation = operation.id
        context._cancelled = Event()
        context._reason = ""
        context._resources = {}
        context._pending = {}
        context._watches = {}
        context._children = []

        # Replies Stay on the Parent Carrier and Are Forwarded by Correlation.
        def duration_ns(value: Duration | Timestamp) -> int:
            if value.seconds < 0 or not 0 <= value.nanos < 1_000_000_000 or value.seconds > 315_576_000_000:
                raise ValueError(f"Invoke {operation.id}: Invalid Policy Duration")
            return value.seconds * 1_000_000_000 + value.nanos

        timeout = duration_ns(policy.timeout) if policy.HasField("timeout") else None
        attempt_timeout = duration_ns(policy.attempt_timeout) if policy.HasField("attempt_timeout") else None
        grace = duration_ns(policy.cancellation_timeout)
        if timeout is not None and timeout <= 0 or attempt_timeout is not None and attempt_timeout <= 0 or grace < 0:
            raise ValueError(f"Invoke {operation.id}: Invalid Deadline")
        attempts = policy.retries.total_attempts if policy.HasField("retries") else 1
        if attempts < 1 or attempts > 1 and not definition.replay_safe:
            raise ValueError(f"Invoke {operation.id}: Retry Requires Replay Safety")
        if attempts > 1:
            initial = duration_ns(policy.retries.initial)
            maximum = duration_ns(policy.retries.maximum) if policy.retries.HasField("maximum") else initial
            if initial <= 0 or maximum < initial:
                raise ValueError(f"Invoke {operation.id}: Invalid Retry Delay")
        guard = copy.copy(self)
        guard._background = True
        guard._pending = {}
        guard._watches = {}
        guard._children = []
        guard._cancelled = Event()
        self._children.extend([context, guard])
        try:
            started = duration_ns(await self.now()) if timeout is not None else 0
            for attempt in range(attempts):
                if self._cancelled.is_set():
                    raise self._cancel_failure()
                context.attempt = attempt + 1
                context._resources = {}
                context._cancelled = Event()
                context._reason = ""
                remaining = timeout - (duration_ns(await self.now()) - started) if timeout is not None else None
                bounds = [value for value in (remaining, attempt_timeout) if value is not None]
                bound = min(bounds) if bounds else None
                if bound is not None and bound <= 0:
                    raise FailureError(
                        WireFailure(
                            classification=CLASSIFICATION_TIMED_OUT,
                            code="deadline",
                            message=f"Invoke {operation.id}: Deadline Reached",
                            action="Invoke Again If Safe",
                        )
                    )
                running = asyncio.create_task(handler(context, request))
                cancelled = asyncio.create_task(self.cancelled())
                timer = (
                    asyncio.create_task(guard.sleep(Duration(seconds=bound // 1_000_000_000, nanos=bound % 1_000_000_000)))
                    if bound is not None
                    else None
                )
                tasks = [running, cancelled] + ([timer] if timer else [])
                try:
                    done, _ = await asyncio.wait(tasks, return_when=FIRST_COMPLETED)
                    if running not in done:
                        reason = cancelled.result() if cancelled in done else "Deadline Reached"
                        await context._cancel(reason)
                        deadline_reached = timer in done
                        if grace and not deadline_reached:
                            settling = asyncio.create_task(
                                guard.sleep(Duration(seconds=grace // 1_000_000_000, nanos=grace % 1_000_000_000))
                            )
                            try:
                                settled, _ = await asyncio.wait(
                                    [running, settling] + ([timer] if timer else []), return_when=FIRST_COMPLETED
                                )
                                deadline_reached = timer in settled
                            finally:
                                settling.cancel()
                                await asyncio.gather(settling, return_exceptions=True)
                        if not grace or deadline_reached or not running.done():
                            kind = CLASSIFICATION_CANCELLED if cancelled in done else CLASSIFICATION_TIMED_OUT
                            raise FailureError(
                                WireFailure(
                                    classification=kind,
                                    code="interrupted",
                                    message=f"Invoke {operation.id}: {reason}",
                                    action="Invoke Again If Safe",
                                )
                            )
                    result = await running
                    if not isinstance(result, operation.response):
                        raise TypeError(f"Invoke {operation.id}: Response Does Not Match the Operation")
                    return result
                except FailureError as failure:
                    if failure.value.classification != CLASSIFICATION_TRANSIENT or attempt + 1 >= attempts:
                        raise
                    await context._cancel("Local Attempt Settled")
                    if timer is not None:
                        timer.cancel()
                        await asyncio.gather(timer, return_exceptions=True)
                    delay = duration_ns(policy.retries.initial)
                    maximum = duration_ns(policy.retries.maximum) if policy.retries.HasField("maximum") else None
                    if delay <= 0 or maximum is not None and maximum < delay:
                        raise ValueError(f"Invoke {operation.id}: Invalid Retry Delay") from failure
                    if maximum is not None:
                        delay = min(delay * 2**attempt, maximum)
                    if policy.retries.deterministic_jitter:
                        delay += (attempt * 17 + 11) % (delay // 4 + 1)
                        if maximum is not None:
                            delay = min(delay, maximum)
                    if timeout is not None:
                        delay = min(delay, max(0, timeout - (duration_ns(await self.now()) - started)))
                    if delay:
                        await self.sleep(Duration(seconds=delay // 1_000_000_000, nanos=delay % 1_000_000_000))
                finally:
                    for task in tasks:
                        if not task.done():
                            task.cancel()
                    await asyncio.gather(*tasks, return_exceptions=True)
            raise ProtocolError(f"Invoke {operation.id}: No Attempt Settled")
        finally:
            await context._cancel("Local Attempt Settled")
            await guard._cancel("Policy Guard Settled")
            self._children.remove(context)
            self._children.remove(guard)

    async def settings(self, *, at: ReadCut | None = None) -> SettingsListResponse:
        """Read the Calling Plugin's Complete Settings and Revision."""
        return await self.host.plugins.settings.list(at=at)

    async def sleep(self, duration: Duration) -> None:
        await self.call(SLEEP, TimeSleepRequest(duration=duration))

    async def now(self) -> Timestamp:
        reply = await self.call(GET_TIME, TimeGetRequest())
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
        if complete is None:
            for child in self._children:
                if child._has_pending(reply.call):
                    await child._reply(reply)
                    return
        if complete is None or complete.done():
            raise ProtocolError("Reply to Child: Unknown or Duplicate Identity")
        complete.set_result(reply)

    def _has_pending(self, call: int) -> bool:
        return call in self._pending or call in self._watches or any(child._has_pending(call) for child in self._children)

    async def _cancel(self, reason: str) -> None:
        if self._cancelled.is_set():
            return
        self._reason = reason
        self._cancelled.set()
        for child in tuple(self._children):
            if not child._background:
                await child._cancel(reason)
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

    def _cancel_failure(self) -> FailureError:
        return FailureError(
            WireFailure(
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
