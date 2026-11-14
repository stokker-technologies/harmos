# @generated from protocol descriptors; do not edit.
from collections.abc import AsyncGenerator
from enum import Enum
from typing import TYPE_CHECKING, cast

from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from harmos._contracts import Operation
from harmos._wire.plugin.host.v1.invocations_pb2 import (
    InvocationsCancelRequest,
    InvocationsCancelResponse,
    InvocationsGetRequest,
    InvocationsGetResponse,
    InvocationsWatchRequest,
    InvocationsWatchResponse,
)
from harmos._wire.plugin.host.v1.operations_pb2 import (
    OperationsGetRequest,
    OperationsGetResponse,
    OperationsListRequest,
    OperationsListResponse,
)
from harmos._wire.plugin.host.v1.settings_pb2 import SettingsListRequest, SettingsListResponse
from harmos._wire.plugin.host.v1.time_pb2 import TimeGetRequest, TimeGetResponse, TimeSleepRequest, TimeSleepResponse
from harmos._wire.v1.invocation_pb2 import InstanceState
from harmos._wire.v1.operation_pb2 import VISIBILITY_PUBLIC, DiscoveredOperation
from harmos._wire.v1.policy_pb2 import OperationPolicy
from harmos._wire.v1.position_pb2 import ReadCut

if TYPE_CHECKING:
    from harmos._invocation import Context


class HostOperation(Enum):
    INVOCATIONS_GET = "harmos.plugin.host.v1.Invocations/Get"
    INVOCATIONS_CANCEL = "harmos.plugin.host.v1.Invocations/Cancel"
    INVOCATIONS_WATCH = "harmos.plugin.host.v1.Invocations/Watch"
    OPERATIONS_LIST = "harmos.plugin.host.v1.Operations/List"
    OPERATIONS_GET = "harmos.plugin.host.v1.Operations/Get"
    SETTINGS_LIST = "harmos.plugin.host.v1.Settings/List"
    TIME_GET = "harmos.plugin.host.v1.Time/Get"
    TIME_SLEEP = "harmos.plugin.host.v1.Time/Sleep"


_INVOCATIONS_GET = Operation(
    HostOperation.INVOCATIONS_GET.value, InvocationsGetRequest, InvocationsGetResponse, visibility=VISIBILITY_PUBLIC, watch=False
)
_INVOCATIONS_CANCEL = Operation(
    HostOperation.INVOCATIONS_CANCEL.value, InvocationsCancelRequest, InvocationsCancelResponse, visibility=VISIBILITY_PUBLIC, watch=False
)
_INVOCATIONS_WATCH = Operation(
    HostOperation.INVOCATIONS_WATCH.value, InvocationsWatchRequest, InvocationsWatchResponse, visibility=VISIBILITY_PUBLIC, watch=True
)
_OPERATIONS_LIST = Operation(
    HostOperation.OPERATIONS_LIST.value, OperationsListRequest, OperationsListResponse, visibility=VISIBILITY_PUBLIC, watch=False
)
_OPERATIONS_GET = Operation(
    HostOperation.OPERATIONS_GET.value, OperationsGetRequest, OperationsGetResponse, visibility=VISIBILITY_PUBLIC, watch=False
)
_SETTINGS_LIST = Operation(
    HostOperation.SETTINGS_LIST.value, SettingsListRequest, SettingsListResponse, visibility=VISIBILITY_PUBLIC, watch=False
)
_TIME_GET = Operation(HostOperation.TIME_GET.value, TimeGetRequest, TimeGetResponse, visibility=VISIBILITY_PUBLIC, watch=False)
_TIME_SLEEP = Operation(HostOperation.TIME_SLEEP.value, TimeSleepRequest, TimeSleepResponse, visibility=VISIBILITY_PUBLIC, watch=False)


def host_requirement(operation: HostOperation) -> Operation[Message, Message]:
    match operation:
        case HostOperation.INVOCATIONS_GET:
            return cast(Operation[Message, Message], _INVOCATIONS_GET)
        case HostOperation.INVOCATIONS_CANCEL:
            return cast(Operation[Message, Message], _INVOCATIONS_CANCEL)
        case HostOperation.INVOCATIONS_WATCH:
            return cast(Operation[Message, Message], _INVOCATIONS_WATCH)
        case HostOperation.OPERATIONS_LIST:
            return cast(Operation[Message, Message], _OPERATIONS_LIST)
        case HostOperation.OPERATIONS_GET:
            return cast(Operation[Message, Message], _OPERATIONS_GET)
        case HostOperation.SETTINGS_LIST:
            return cast(Operation[Message, Message], _SETTINGS_LIST)
        case HostOperation.TIME_GET:
            return cast(Operation[Message, Message], _TIME_GET)
        case HostOperation.TIME_SLEEP:
            return cast(Operation[Message, Message], _TIME_SLEEP)
    raise ValueError("Declare Host Requirement: Unknown Operation")


class HostRoot:
    def __init__(self, context: "Context") -> None:
        self.invocations = HostInvocationsResource(context)
        self.operations = HostOperationsResource(context)
        self.settings = HostSettingsResource(context)
        self.time = HostTimeResource(context)


class HostInvocationsResource:
    def __init__(self, context: "Context") -> None:
        self._context = context

    async def get(self, instance: int, *, policy: OperationPolicy | None = None) -> InstanceState:
        request = InvocationsGetRequest(instance=instance)
        response = await self._context.call(_INVOCATIONS_GET, request, policy=policy)
        if not response.HasField("invocation"):
            raise ValueError("Read INVOCATIONS_GET: Missing Payload")
        return response.invocation

    async def cancel(self, instance: int, reason: str, *, policy: OperationPolicy | None = None) -> InstanceState:
        request = InvocationsCancelRequest(instance=instance, reason=reason)
        response = await self._context.call(_INVOCATIONS_CANCEL, request, policy=policy)
        if not response.HasField("invocation"):
            raise ValueError("Read INVOCATIONS_CANCEL: Missing Payload")
        return response.invocation

    async def watch(self, instance: int, *, max_item_bytes: int) -> AsyncGenerator[InstanceState]:
        request = InvocationsWatchRequest(instance=instance)
        call = self._context.watch(_INVOCATIONS_WATCH, request, max_item_bytes=max_item_bytes)
        try:
            async for response in call:
                if not response.HasField("invocation"):
                    raise ValueError("Read INVOCATIONS_WATCH: Missing Payload")
                yield response.invocation
        finally:
            await call.aclose()


class HostOperationsResource:
    def __init__(self, context: "Context") -> None:
        self._context = context

    async def list(self, *, policy: OperationPolicy | None = None) -> list[DiscoveredOperation]:
        request = OperationsListRequest()
        response = await self._context.call(_OPERATIONS_LIST, request, policy=policy)
        return list(response.operations)

    async def get(self, id: str, *, policy: OperationPolicy | None = None) -> DiscoveredOperation:
        request = OperationsGetRequest(id=id)
        response = await self._context.call(_OPERATIONS_GET, request, policy=policy)
        if not response.HasField("operation"):
            raise ValueError("Read OPERATIONS_GET: Missing Payload")
        return response.operation


class HostSettingsResource:
    def __init__(self, context: "Context") -> None:
        self._context = context

    async def list(self, *, at: ReadCut | None = None, policy: OperationPolicy | None = None) -> SettingsListResponse:
        request = SettingsListRequest(at=at)
        response = await self._context.call(_SETTINGS_LIST, request, policy=policy)
        return response


class HostTimeResource:
    def __init__(self, context: "Context") -> None:
        self._context = context

    async def get(self, *, policy: OperationPolicy | None = None) -> Timestamp:
        request = TimeGetRequest()
        response = await self._context.call(_TIME_GET, request, policy=policy)
        if not response.HasField("now"):
            raise ValueError("Read TIME_GET: Missing Payload")
        return response.now

    async def sleep(self, duration: Duration, *, policy: OperationPolicy | None = None) -> TimeSleepResponse:
        request = TimeSleepRequest(duration=duration)
        response = await self._context.call(_TIME_SLEEP, request, policy=policy)
        return response
