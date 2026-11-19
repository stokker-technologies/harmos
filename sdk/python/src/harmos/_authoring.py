# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Remote Plugin Declarations Construction and Plain gRPC Serving."""

# === Imports === #

# Standard Library Imports
import asyncio
import inspect
import re
from asyncio import Queue, Task
from collections.abc import AsyncIterator, Awaitable, Callable, Sequence
from hashlib import sha256
from importlib.metadata import version
from typing import Protocol, cast

# Third Party Imports
import grpc
from google.protobuf.descriptor_pb2 import FileDescriptorProto
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.duration_pb2 import Duration
from google.protobuf.empty_pb2 import Empty
from google.protobuf.message import DecodeError, Message
from google.protobuf.timestamp_pb2 import Timestamp
from grpc import StatusCode
from grpc.aio import Server, ServicerContext

from harmos._wire.model.v1.description_pb2 import Declarations
from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PRIVATE, VISIBILITY_PROTECTED, VISIBILITY_PUBLIC
from harmos._wire.model.v1.setting_pb2 import SettingDeclaration, SettingsDeclarations
from harmos._wire.plugin.v1 import stream_pb2
from harmos._wire.plugin.v1.instance_pb2 import (
    InstanceGetRequest,
    InstanceGetResponse,
    InstanceInvokeRequest,
    InstanceInvokeResponse,
    InstanceSamplesRequest,
    InstanceSamplesResponse,
    InstanceStopRequest,
    InstanceStopResponse,
)
from harmos._wire.plugin.v1.invocation_pb2 import Request, Response
from harmos._wire.plugin.v1.stream_pb2 import Sample

from ._columns import OversizedSampleError, append, batches, extract
from ._contracts import Lane, Operation, Stream, Window, message_types
from ._generated.host import HostOperation, host_requirement

# Local Imports
from ._invocation import Context, FailureError, Handler, ProtocolError, Publisher, validate_timestamp
from ._transfer import message_bytes
from ._wire.model.v1.transfer_pb2 import StorageTransferSettings

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STARTED_AT = Timestamp()
STARTED_AT.GetCurrentTime()

# === Types === #

type Observer = Callable[[Context], Awaitable[Message]]
type WindowObserver = Callable[[Context, Window], Awaitable[Message]]
type Producer = Callable[[Context, Publisher], Awaitable[None]]
type Hook = Callable[[Context], Awaitable[None]]

# === Classes === #


class Registration(Protocol):
    def register(self, ctx: "Registrar") -> None: ...


class Registrar:
    """Declaration-Only Accumulation; Runtime Facilities Are Unavailable."""

    def __init__(self) -> None:
        self.operations: list[tuple[Operation[Message, Message], Handler]] = []
        self.streams: list[tuple[Stream, Observer | WindowObserver | Producer, bool]] = []
        self.hooks: dict[str, list[Hook]] = {}
        self.requires: list[Operation] = []
        self.sections: list[Message] = []
        self.settings: list[SettingDeclaration] = []
        self.messages: list[type[Message]] = []

    def register(self, item: Registration) -> None:
        item.register(self)


class Catalog:
    def __init__(
        self,
        instance: object,
        *,
        requires: Sequence[Operation | HostOperation] = (),
        sections: Sequence[Message] = (),
        settings: Sequence[SettingDeclaration] = (),
        messages: Sequence[type[Message]] = (),
    ) -> None:
        self.instance = instance
        self.requires = tuple(
            host_requirement(requirement) if isinstance(requirement, HostOperation) else requirement for requirement in requires
        )
        self.sections = tuple(sections)
        self.settings = tuple(settings)
        self.messages = tuple(messages)

    def _registrations(
        self,
    ) -> tuple[list[tuple[Operation[Message, Message], Handler]], list[tuple[Stream, Observer | WindowObserver | Producer, bool]]]:
        # Preserve Declaration Order While Honoring Subclass Overrides.
        methods = {}
        for cls in reversed(type(self.instance).__mro__):
            methods.update(vars(cls))
        operations: list[tuple[Operation[Message, Message], Handler]] = []
        streams: list[tuple[Stream, Observer | WindowObserver | Producer, bool]] = []
        for name, method in methods.items():
            definition = getattr(method, "__harmos_operation__", None)
            if isinstance(definition, Operation):
                operations.append((cast(Operation[Message, Message], definition), cast(Handler, getattr(self.instance, name))))
            definition = getattr(method, "__harmos_observer__", None)
            if isinstance(definition, Stream):
                streams.append((definition, cast(Observer, getattr(self.instance, name)), True))
            definition = getattr(method, "__harmos_publisher__", None)
            if isinstance(definition, Stream):
                streams.append((definition, cast(Producer, getattr(self.instance, name)), False))
        return operations, streams

    def register(self, ctx: Registrar) -> None:
        operations, streams = self._registrations()
        ctx.operations.extend(operations)
        ctx.streams.extend(streams)
        for name in ("initialize", "finalize"):
            hook = getattr(self.instance, name, None)
            if hook is None:
                continue
            if not inspect.iscoroutinefunction(hook):
                raise ValueError(f"Register {name}: Define an Async Lifecycle Hook")
            ctx.hooks.setdefault(name, []).append(cast(Hook, hook))
        ctx.requires.extend(self.requires)
        ctx.sections.extend(self.sections)
        ctx.settings.extend(self.settings)
        ctx.messages.extend(self.messages)


class Plugin:
    def __init__(
        self,
        *,
        id: str,
        version: str,
        register: Callable[[Registrar], None],
        lanes: Sequence[Lane] = (),
        transfer: StorageTransferSettings | None = None,
    ) -> None:
        self.id = id
        self.lanes = tuple(lanes)
        self.version = version
        registrar = Registrar()
        register(registrar)
        self._operations = list(registrar.operations)
        self._streams = list(registrar.streams)
        for definition, _ in self._operations:
            if definition.id in (f"{id}.initialize", f"{id}.finalize"):
                raise ValueError(f"Register {definition.id}: Lifecycle Operation Names Are Reserved; Define an Async Hook Instead")
        for name, hooks in registrar.hooks.items():
            definition = Operation(f"{id}.{name}", Empty, Empty, VISIBILITY_PROTECTED)
            self._operations.append((cast(Operation[Message, Message], definition), _lifecycle(hooks)))
        self.requires = tuple(registrar.requires)
        self.sections = tuple(registrar.sections)
        self.settings = tuple(registrar.settings)
        self.messages = tuple(registrar.messages)
        self.transfer = StorageTransferSettings()
        if transfer is not None:
            self.transfer.CopyFrom(transfer)

    def _registrations(
        self,
    ) -> tuple[list[tuple[Operation[Message, Message], Handler]], list[tuple[Stream, Observer | WindowObserver | Producer, bool]]]:
        return self._operations, self._streams

    def declarations(self) -> Declarations:
        if not re.fullmatch(r"[a-z0-9._]+", self.id):
            raise ValueError("Plugin Identity Must Be Lowercase with Dots or Underscores")
        core = r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
        prerelease = r"(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
        if not re.fullmatch(core + rf"(?:-{prerelease}(?:\.{prerelease})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?", self.version):
            raise ValueError("Invalid Plugin Version")
        declarations = Declarations()
        lanes: dict[str, Lane] = {}
        for lane in self.lanes:
            if lane.name in lanes:
                raise ValueError(f"Declare Lane {lane.name!r}: Declare Each Name Once")
            lanes[lane.name] = lane
            declarations.lanes.add().CopyFrom(lane.declaration())
        files: dict[str, FileDescriptorProto] = {}
        _merge(files, *self.messages)
        operations, streams = self._registrations()
        for definition, _ in operations:
            if definition.lane is not None:
                lane = definition.lane
                if definition.id in (f"{self.id}.initialize", f"{self.id}.finalize"):
                    raise ValueError(f"Register {definition.id} Lane {lane.name!r}: Remove the Lifecycle Lane Assignment")
                if lanes.get(lane.name) != lane:
                    raise ValueError(f"Register {definition.id} Lane {lane.name!r}: Declare This Lane in the Plugin")
            if definition.watch:
                raise ValueError(f"Register {definition.id}: Plugin Operations Are Unary")
            if definition.visibility not in (VISIBILITY_PRIVATE, VISIBILITY_PROTECTED, VISIBILITY_PUBLIC):
                raise ValueError(f"Invalid Visibility: {definition.id}")
            if definition.visibility == VISIBILITY_PRIVATE:
                continue
            declarations.operations.add().CopyFrom(definition.contract())
            _merge(files, definition.request, definition.response)
        for definition in self.requires:
            if definition.visibility == VISIBILITY_PRIVATE:
                raise ValueError(f"Require {definition.id}: Private Calls Are Local")
            declarations.requires.add(id=definition.id, fingerprint=definition.fingerprint)
            _merge(files, definition.request, definition.response)
        for definition, _, observing in streams:
            interval = definition.interval
            if observing and interval is None and not definition.interval_setting:
                raise ValueError(f"Observer Interval Required: {definition.id}")
            if interval is not None:
                if not 0 <= interval.seconds <= 315_576_000_000 or not 0 <= interval.nanos < 1_000_000_000:
                    raise ValueError("Invalid Observer Interval")
                if interval.seconds == 0 and interval.nanos == 0:
                    raise ValueError("Positive Observer Interval Required")
            declarations.streams.add().CopyFrom(definition.declaration())
            _merge(files, definition.row)
        names = [setting.name for setting in self.settings]
        if any(not name for name in names) or len(set(names)) != len(names):
            raise ValueError("Declare Settings: Duplicate or Empty Name")
        for setting in self.settings:
            if not setting.HasField("default") or setting.default.WhichOneof("value") is None:
                raise ValueError(f"Declare Setting {setting.name}: Typed Default Required")
            if setting.default.WhichOneof("value") == "duration":
                duration = setting.default.duration
                if abs(duration.seconds) > 315_576_000_000 or abs(duration.nanos) >= 1_000_000_000:
                    raise ValueError(f"Declare Setting {setting.name}: Invalid Duration")
                if duration.seconds * duration.nanos < 0:
                    raise ValueError(f"Declare Setting {setting.name}: Invalid Duration Sign")
        sections = list(self.sections)
        if self.settings:
            sections.append(SettingsDeclarations(values=self.settings))
        for section in sections:
            declarations.sections.add(type=section.DESCRIPTOR.full_name, payload=section.SerializeToString())
            _merge(files, type(section))
        for entries, key in (
            (declarations.operations, "id"),
            (declarations.requires, "id"),
            (declarations.streams, "id"),
            (declarations.sections, "type"),
        ):
            names = [getattr(entry, key) for entry in entries]
            if any(not name for name in names) or len(set(names)) != len(names):
                raise ValueError("Duplicate or Empty Declarations Identifier")
        declarations.types.file.extend(files.values())
        pool = DescriptorPool()
        installed = set()

        def install(name: str) -> None:
            if name in installed:
                return
            definition = files[name]
            for dependency in definition.dependency:
                install(dependency)
            pool.Add(definition)
            installed.add(name)

        for name in files:
            install(name)
        declarations.digest = sha256(declarations.SerializeToString(deterministic=True)).digest()
        return declarations

    async def start(self, address: str) -> "Endpoint":
        """Bind the Author's Address; the Host Invokes Lifecycle Operations Before Stop."""
        service = _Service(self, self.declarations())
        maximum = message_bytes(self.transfer)
        server = grpc.aio.server(options=[("grpc.max_send_message_length", maximum), ("grpc.max_receive_message_length", maximum)])
        server.add_generic_rpc_handlers(
            (
                grpc.method_handlers_generic_handler(
                    "harmos.plugin.v1.Instance",
                    {
                        "Get": grpc.unary_unary_rpc_method_handler(
                            service.get,
                            request_deserializer=InstanceGetRequest.FromString,
                            response_serializer=InstanceGetResponse.SerializeToString,
                        ),
                        "Stop": grpc.unary_unary_rpc_method_handler(
                            service.stop,
                            request_deserializer=InstanceStopRequest.FromString,
                            response_serializer=InstanceStopResponse.SerializeToString,
                        ),
                        "Invoke": grpc.stream_stream_rpc_method_handler(
                            service.invoke,
                            request_deserializer=InstanceInvokeRequest.FromString,
                            response_serializer=InstanceInvokeResponse.SerializeToString,
                        ),
                        "Samples": grpc.stream_stream_rpc_method_handler(
                            service.samples,
                            request_deserializer=InstanceSamplesRequest.FromString,
                            response_serializer=InstanceSamplesResponse.SerializeToString,
                        ),
                    },
                ),
            )
        )
        port = server.add_insecure_port(address)
        if not port:
            raise ValueError(f"Cannot Bind: {address}")
        await server.start()
        host = address.rsplit(":", 1)[0]
        return Endpoint(server, f"{host}:{port}")

    async def run(self, address: str) -> None:
        endpoint = await self.start(address)
        try:
            await endpoint.server.wait_for_termination()
        finally:
            await endpoint.stop()


class Endpoint:
    def __init__(self, server: Server, address: str) -> None:
        self.server = server
        self.address = address

    async def stop(self) -> None:
        await self.server.stop(None)


class _FaultError(Exception):
    def __init__(self, code: StatusCode, message: str) -> None:
        self.code = code
        self.message = message


class _SamplesOutput:
    def __init__(self, output: Queue[Sample | InstanceSamplesResponse | _FaultError | None]) -> None:
        self.output = output
        self.active = True

    async def put(self, frame: InstanceInvokeResponse | _FaultError) -> None:
        if not self.active:
            raise ProtocolError("Call Samples Child: Window Already Answered")
        if isinstance(frame, _FaultError):
            await self.output.put(frame)
            return
        kind = frame.WhichOneof("frame")
        if kind == "child":
            response = InstanceSamplesResponse(child=frame.child)
        elif kind == "child_cancel":
            response = InstanceSamplesResponse(child_cancel=frame.child_cancel)
        elif kind == "child_demand":
            response = InstanceSamplesResponse(child_demand=frame.child_demand)
        else:
            raise ProtocolError("Call Samples Child: Child Frame Required")
        await self.output.put(response)


class _SamplesScope:
    def __init__(
        self,
        stream: str,
        scope: str,
        plugin: str,
        requires: dict[str, bytes],
        local: dict[str, tuple[Operation[Message, Message], Handler]],
        instance: object | None,
    ) -> None:
        self.stream = stream
        self.scope = scope
        self.plugin = plugin
        self.requires = requires
        self.local = local
        self.instance = instance
        self.context: Context | None = None
        self.next = [0]
        self.retired: set[int] = set()


class _Service:
    def __init__(self, plugin: Plugin, declarations: Declarations) -> None:
        self.description = InstanceGetResponse(
            id=plugin.id,
            version=plugin.version,
            started_at=STARTED_AT,
            declarations=declarations,
            revision=1,
            sdk="harmos_python",
            sdk_version=version("harmos-sdk"),
            message_bytes=message_bytes(plugin.transfer),
        )
        operations, streams = plugin._registrations()
        self.local = {
            definition.id: (definition, handler) for definition, handler in operations if definition.visibility == VISIBILITY_PRIVATE
        }
        self.operations = {
            definition.id: (definition, handler) for definition, handler in operations if definition.visibility != VISIBILITY_PRIVATE
        }
        self.streams = {definition.id: (definition, handler, observing) for definition, handler, observing in streams}
        self.requires = {requirement.id: requirement.fingerprint for requirement in declarations.requires}

    async def get(self, request: InstanceGetRequest, context: ServicerContext) -> InstanceGetResponse:
        if request.revision == 0:
            await context.abort(StatusCode.INVALID_ARGUMENT, "Revision Must Be at Least One")
        return self.description

    async def stop(self, request: InstanceStopRequest, context: ServicerContext) -> InstanceStopResponse:
        return InstanceStopResponse()

    async def invoke(self, request_iterator: AsyncIterator[InstanceInvokeRequest], context: ServicerContext) -> None:
        first = await anext(request_iterator, None)
        if first is None or first.WhichOneof("frame") != "request":
            await context.abort(StatusCode.INVALID_ARGUMENT, "Request Must Be First")
            return
        request = first.request
        if not request.invocation or request.attempt == 0:
            await context.abort(StatusCode.INVALID_ARGUMENT, "Invocation and Attempt Are Required")
        registered = self.operations.get(request.operation)
        if registered is None:
            await context.abort(StatusCode.NOT_FOUND, "Unknown Operation")
            return
        definition, handler = registered
        try:
            value = definition.request.FromString(request.payload)
        except DecodeError as error:
            await context.abort(StatusCode.INVALID_ARGUMENT, str(error))

        await context.send_initial_metadata(())
        output = Queue(maxsize=1)
        execution = Context(request, self.description.id, self.requires, output)
        execution._local = self.local
        execution._instance = getattr(handler, "__self__", None)
        running = asyncio.create_task(_execute(handler, execution, value, definition.response, output))
        reading = asyncio.create_task(_read_invoke(request_iterator, execution, output))
        try:
            while True:
                frame = await output.get()
                if isinstance(frame, _FaultError):
                    await context.abort(frame.code, frame.message)
                await context.write(frame)
                if frame.WhichOneof("frame") == "response":
                    return
        finally:
            execution._output = None
            await _release(running, reading)

    async def samples(self, request_iterator: AsyncIterator[InstanceSamplesRequest], context: ServicerContext) -> None:
        first = await anext(request_iterator, None)
        if first is None or first.WhichOneof("frame") != "open":
            await context.abort(StatusCode.INVALID_ARGUMENT, "Open Must Be First")
            return
        registered = self.streams.get(first.open.stream)
        if registered is None:
            await context.abort(StatusCode.NOT_FOUND, "Unknown Stream")
            return
        definition, handler, observing = registered
        await context.send_initial_metadata(())
        output: Queue[Sample | InstanceSamplesResponse | _FaultError | None] = Queue(maxsize=1)
        windows: Queue[tuple[Window, Context]] = Queue(maxsize=1)
        scope = _SamplesScope(
            first.open.stream, first.open.scope, self.description.id, self.requires, self.local, getattr(handler, "__self__", None)
        )
        reading = asyncio.create_task(_read_publish(request_iterator, observing, scope, windows, output))
        running = asyncio.create_task(_produce(handler, scope, definition, observing, windows, output, self.description.message_bytes))
        sequence = 0
        try:
            while True:
                frame = await output.get()
                if frame is None:
                    return
                if isinstance(frame, _FaultError):
                    await context.abort(frame.code, frame.message)
                    return
                if isinstance(frame, InstanceSamplesResponse):
                    await context.write(frame)
                    continue
                sequence += 1
                frame.sequence = sequence
                await context.write(InstanceSamplesResponse(samples=[frame]))
        finally:
            if scope.context is not None:
                scope.context._output = None
                await scope.context._cancel("Samples Demand Closed")
            await _release(running, reading)


# === Public Functions === #

# === Private Functions === #


def _lifecycle(hooks: Sequence[Hook]) -> Handler:
    async def invoke(context: Context, request: Message) -> Message:
        for hook in hooks:
            context._instance = getattr(hook, "__self__", None)
            await hook(context)
        return Empty()

    return invoke


def _merge(files: dict[str, FileDescriptorProto], *messages: type[Message]) -> None:
    for message in messages:
        for file in message_types(message).file:
            existing = files.get(file.name)
            if existing is None:
                files[file.name] = file
                continue
            if existing.package != file.package or existing.syntax != file.syntax:
                raise ValueError(f"Conflicting Protobuf File: {file.name}")
            for dependency in file.dependency:
                if dependency not in existing.dependency:
                    existing.dependency.append(dependency)
            for kind in ("message_type", "enum_type"):
                entries = getattr(existing, kind)
                for item in getattr(file, kind):
                    previous = next((entry for entry in entries if entry.name == item.name), None)
                    if previous is not None and previous != item:
                        raise ValueError(f"Conflicting Protobuf Definition: {item.name}")
                    if previous is None:
                        entries.add().CopyFrom(item)


async def _release(*tasks: Task) -> None:
    for task in tasks:
        task.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)


async def _execute(
    handler: Handler,
    execution: Context,
    request: Message,
    response: type[Message],
    output: Queue[InstanceInvokeResponse | _FaultError],
) -> None:
    try:
        value = await handler(execution, request)
        if not isinstance(value, response):
            raise TypeError("Handler Returned the Wrong Protobuf Type")
        result = Response(payload=value.SerializeToString())
    except FailureError as failure:
        result = Response(failure=failure.value)
    except (ProtocolError, DecodeError) as error:
        await output.put(_FaultError(StatusCode.INVALID_ARGUMENT, str(error)))
        return
    except Exception as error:
        await output.put(_FaultError(StatusCode.INTERNAL, str(error)))
        return
    await output.put(InstanceInvokeResponse(response=result))


async def _read_invoke(
    input: AsyncIterator[InstanceInvokeRequest],
    execution: Context,
    output: Queue[InstanceInvokeResponse | _FaultError],
) -> None:
    try:
        async for frame in input:
            match frame.WhichOneof("frame"):
                case "child":
                    await execution._reply(frame.child)
                case "cancel":
                    await execution._cancel(frame.cancel.reason)
                case _:
                    raise ProtocolError("Expected Child Response or Cancel")
        if execution._pending or execution._watches:
            await output.put(_FaultError(StatusCode.UNAVAILABLE, "Host Closed Before Child Replies"))
    except ProtocolError as error:
        await output.put(_FaultError(StatusCode.INVALID_ARGUMENT, str(error)))


async def _read_publish(
    input: AsyncIterator[InstanceSamplesRequest],
    observing: bool,
    scope: _SamplesScope,
    windows: Queue[tuple[Window, Context]],
    output: Queue[Sample | InstanceSamplesResponse | _FaultError | None],
) -> None:
    try:
        async for frame in input:
            kind = frame.WhichOneof("frame")
            if kind == "child":
                if scope.context is None:
                    raise ProtocolError("Reply to Samples Child: Unanswered Window Required")
                await scope.context._reply(frame.child)
                continue
            if kind != "window":
                raise ProtocolError("Read Samples: Expected Window or Child Reply")
            if not observing:
                raise ProtocolError("Read Samples: Device Streams Do Not Accept Windows")
            if scope.context is not None:
                raise ProtocolError("Read Samples: Previous Window Is Unanswered")
            window = _window(frame.window)
            execution = Context(Request(operation=scope.stream, scope=scope.scope), scope.plugin, scope.requires, _SamplesOutput(output))
            execution.sequence = window.sequence
            execution._next = scope.next
            execution._retired = scope.retired
            execution._local = scope.local
            execution._instance = scope.instance
            scope.context = execution
            await windows.put((window, execution))
        await output.put(None)
    except ProtocolError as error:
        await output.put(_FaultError(StatusCode.INVALID_ARGUMENT, str(error)))


def _window(window: stream_pb2.Window) -> Window:
    if not window.HasField("start"):
        raise ProtocolError("Read Samples: Window Start Time Required")
    validate_timestamp(window.start)
    period = window.period
    if (
        not window.HasField("period")
        or not 0 <= period.seconds <= 315_576_000_000
        or not 0 <= period.nanos < 1_000_000_000
        or period.seconds == 0
        and period.nanos == 0
    ):
        raise ProtocolError("Read Samples: Window Period Must Be a Positive Duration")
    if window.count == 0:
        raise ProtocolError("Read Samples: Window Count Must Be at Least One")
    start = Timestamp()
    start.CopyFrom(window.start)
    period = Duration()
    period.CopyFrom(window.period)
    return Window(start, period, window.count, window.sequence)


async def _produce(
    handler: Observer | WindowObserver | Producer,
    scope: _SamplesScope,
    definition: Stream,
    observing: bool,
    windows: Queue[tuple[Window, Context]],
    output: Queue[Sample | InstanceSamplesResponse | _FaultError | None],
    limit: int,
) -> None:
    try:
        if not observing:
            execution = Context(Request(operation=scope.stream, scope=scope.scope), scope.plugin, scope.requires)
            execution._local = scope.local
            execution._instance = scope.instance
            await cast(Producer, handler)(execution, Publisher(definition.row, output))
            return
        while True:
            window, execution = await windows.get()
            if definition.columns is None:
                columns = [[] for _ in definition.row.DESCRIPTOR.fields]
                for index in range(window.count):
                    execution.sequence = window.sequence + index
                    row = await cast(Observer, handler)(execution)
                    if not isinstance(row, definition.row):
                        raise TypeError("Answer Samples Window: Observer Returned the Wrong Protobuf Type")
                    append(columns, row)
            else:
                result = await cast(WindowObserver, handler)(execution, window)
                if not isinstance(result, definition.columns):
                    raise TypeError("Answer Samples Window: Window Observer Returned the Wrong Columns Type")
                columns = extract(definition.row, result, window.count)
            parts = batches(definition.row, columns, window.count, limit)
            await execution._cancel("Samples Window Answered")
            if isinstance(execution._output, _SamplesOutput):
                execution._output.active = False
            execution._output = None
            for part in parts:
                if part.last:
                    scope.context = None
                await output.put(InstanceSamplesResponse(batch=part))
    except ProtocolError as error:
        await output.put(_FaultError(StatusCode.INVALID_ARGUMENT, str(error)))
    except OversizedSampleError as error:
        await output.put(_FaultError(StatusCode.RESOURCE_EXHAUSTED, str(error)))
    except Exception as error:
        await output.put(_FaultError(StatusCode.INTERNAL, str(error)))


# === Entry Point === #
