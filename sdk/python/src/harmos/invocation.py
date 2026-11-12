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
from contextlib import asynccontextmanager

# Third Party Imports
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from .contracts import Operation
from .v1 import types_pb2 as wire

from harmos.plugin.v1 import instance_pb2 as carrier

from harmos.context.v1 import time_pb2 as clock

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SLEEP = Operation("harmos.context.v1.Time/Sleep", clock.TimeSleepRequest, clock.TimeSleepResponse, visibility=wire.VISIBILITY_PUBLIC)
GET_TIME = Operation("harmos.context.v1.Time/Get", clock.TimeGetRequest, clock.TimeGetResponse, visibility=wire.VISIBILITY_PUBLIC)

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
        self._next = [0]
        self._local = {}
        self._watches = {}
        self._background = False
        self._retired = set()
        self._children = []
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

    async def call[Q: Message, R: Message](
        self, operation: Operation[Q, R], request: Q, *, policy: wire.OperationPolicy | None = None
    ) -> R:
        if self._output is None:
            raise ValueError("Observers Cannot Call Operations or Wait")
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
        complete = asyncio.get_running_loop().create_future()
        self._pending[call] = complete
        child = wire.ChildRequest(
            call=call,
            operation=operation.id,
            payload=request.SerializeToString(),
            fingerprint=operation.fingerprint,
            policy=policy,
            background=self._background,
        )
        try:
            await self._output.put(carrier.InstanceInvokeResponse(child=child))
            reply = await complete
            if reply.WhichOneof("result") == "failure":
                raise Failure(reply.failure)
            return operation.response.FromString(reply.payload)
        except asyncio.CancelledError:
            if self._pending.pop(call, None) is not None:
                self._retired.add(call)
                await self._output.put(carrier.InstanceInvokeResponse(child_cancel=wire.ChildCancel(call=call)))
            raise

    @asynccontextmanager
    async def watch(self, operation, request, *, max_item_bytes: int):
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
        queue = asyncio.Queue(maxsize=1)
        self._watches[call] = (queue, max_item_bytes)
        await output.put(
            carrier.InstanceInvokeResponse(
                child=wire.ChildRequest(
                    call=call,
                    operation=operation.id,
                    payload=request.SerializeToString(),
                    fingerprint=operation.fingerprint,
                    watch=True,
                    max_response_bytes=max_item_bytes,
                )
            )
        )

        async def items():
            while True:
                await output.put(carrier.InstanceInvokeResponse(child_demand=wire.ChildDemand(call=call)))
                reply = await queue.get()
                kind = reply.WhichOneof("result")
                if kind == "failure":
                    raise Failure(reply.failure)
                if kind == "payload":
                    return
                yield operation.response.FromString(reply.item)

        try:
            yield items()
        finally:
            if self._watches.pop(call, None) is not None:
                await output.put(carrier.InstanceInvokeResponse(child_cancel=wire.ChildCancel(call=call)))

    async def _invoke_local(self, operation, request, override):
        definition, handler = self._local[operation.id]
        if operation.fingerprint != definition.fingerprint:
            raise ValueError(f"Invoke {operation.id}: Local Contract Differs")
        policy = override if override is not None else definition.policy
        policy = policy if policy is not None else wire.OperationPolicy()
        context = copy.copy(self)
        context._operation = operation.id
        context._cancelled = asyncio.Event()
        context._reason = ""
        context._resources = {}
        context._pending = {}
        context._watches = {}
        context._children = []

        # Replies Stay on the Parent Carrier and Are Forwarded by Correlation.
        def duration_ns(value):
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
        guard._cancelled = asyncio.Event()
        self._children.extend([context, guard])
        try:
            started = duration_ns(await self.now()) if timeout is not None else 0
            for attempt in range(attempts):
                if self._cancelled.is_set():
                    raise self._cancel_failure()
                context.attempt = attempt + 1
                context._resources = {}
                context._cancelled = asyncio.Event()
                context._reason = ""
                remaining = timeout - (duration_ns(await self.now()) - started) if timeout is not None else None
                bounds = [value for value in (remaining, attempt_timeout) if value is not None]
                bound = min(bounds) if bounds else None
                if bound is not None and bound <= 0:
                    raise Failure(
                        wire.Failure(
                            classification=wire.CLASSIFICATION_TIMED_OUT,
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
                    done, _ = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
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
                                    [running, settling] + ([timer] if timer else []), return_when=asyncio.FIRST_COMPLETED
                                )
                                deadline_reached = timer in settled
                            finally:
                                settling.cancel()
                                await asyncio.gather(settling, return_exceptions=True)
                        if not grace or deadline_reached or not running.done():
                            kind = wire.CLASSIFICATION_CANCELLED if cancelled in done else wire.CLASSIFICATION_TIMED_OUT
                            raise Failure(
                                wire.Failure(
                                    classification=kind,
                                    code="interrupted",
                                    message=f"Invoke {operation.id}: {reason}",
                                    action="Invoke Again If Safe",
                                )
                            )
                    result = await running
                    if not isinstance(result, definition.response):
                        raise TypeError(f"Invoke {operation.id}: Response Does Not Match the Operation")
                    return result
                except Failure as failure:
                    if failure.value.classification != wire.CLASSIFICATION_TRANSIENT or attempt + 1 >= attempts:
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
        finally:
            await context._cancel("Local Attempt Settled")
            await guard._cancel("Policy Guard Settled")
            self._children.remove(context)
            self._children.remove(guard)

    async def sleep(self, duration: Duration) -> None:
        await self.call(SLEEP, clock.TimeSleepRequest(duration=duration))

    async def now(self) -> Timestamp:
        reply = await self.call(GET_TIME, clock.TimeGetRequest())
        if not reply.HasField("now"):
            raise ProtocolError("Host Time Missing")
        validate_timestamp(reply.now)
        return reply.now

    async def cancelled(self) -> str:
        await self._cancelled.wait()
        return self._reason

    async def _reply(self, reply: wire.ChildResponse) -> None:
        if reply.call in self._retired:
            return
        if reply.WhichOneof("result") is None:
            raise ProtocolError("Child Result Missing")
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
            raise ProtocolError("Unknown or Duplicate Child")
        complete.set_result(reply)

    def _has_pending(self, call):
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
                await self._output.put(carrier.InstanceInvokeResponse(child_cancel=wire.ChildCancel(call=call)))
            await queue.put(wire.ChildResponse(call=call, failure=self._cancel_failure().value))
        pending, self._pending = self._pending, {}
        for call, complete in pending.items():
            self._retired.add(call)
            if self._output is not None:
                await self._output.put(carrier.InstanceInvokeResponse(child_cancel=wire.ChildCancel(call=call)))
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
