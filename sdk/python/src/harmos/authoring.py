# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Remote Plugin Catalog Construction and Plain gRPC Serving."""

# === Imports === #

# Standard Library Imports
import asyncio
import inspect
import re
from collections.abc import Sequence
from hashlib import sha256
from importlib.metadata import version

# Third Party Imports
import grpc
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message import DecodeError, Message
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from .invocation import Context, Failure, ProtocolError, Publisher, validate_timestamp
from .contracts import Operation, message_types
from .v1 import types_pb2 as wire
from .plugin.v1.instance_pb2_grpc import InstanceServicer, add_InstanceServicer_to_server

from harmos.plugin.v1 import instance_pb2 as carrier

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STARTED_AT = Timestamp()
STARTED_AT.GetCurrentTime()

# === Types === #

# === Classes === #


class Plugin:
    def __init__(
        self,
        instance: object,
        *,
        id: str,
        version: str,
        requires: Sequence[Operation] = (),
        sections: Sequence[Message] = (),
        messages: Sequence[type[Message]] = (),
    ):
        self.instance = instance
        self.id = id
        self.version = version
        self.requires = tuple(requires)
        self.sections = tuple(sections)
        self.messages = tuple(messages)

    def _registrations(self):
        # Preserve Declaration Order While Honoring Subclass Overrides.
        methods = {}
        for cls in reversed(type(self.instance).__mro__):
            methods.update(vars(cls))
        operations = []
        streams = []
        cleanup = []
        for name, method in methods.items():
            if definition := getattr(method, "__harmos_operation__", None):
                operations.append((definition, getattr(self.instance, name)))
            if definition := getattr(method, "__harmos_observer__", None):
                streams.append((definition, getattr(self.instance, name), True))
            if definition := getattr(method, "__harmos_publisher__", None):
                streams.append((definition, getattr(self.instance, name), False))
            if getattr(method, "__harmos_stop__", False):
                cleanup.append(getattr(self.instance, name))
        return operations, streams, cleanup

    def catalog(self) -> wire.Catalog:
        if not re.fullmatch(r"[a-z0-9._]+", self.id):
            raise ValueError("Plugin Identity Must Be Lowercase with Dots or Underscores")
        core = r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
        prerelease = r"(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
        if not re.fullmatch(core + rf"(?:-{prerelease}(?:\.{prerelease})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?", self.version):
            raise ValueError("Invalid Plugin Version")
        catalog = wire.Catalog()
        files = {}
        _merge(files, *self.messages)
        operations, streams, _ = self._registrations()
        for definition, _ in operations:
            if definition.watch:
                raise ValueError(f"Register {definition.id}: Plugin Operations Are Unary")
            if definition.visibility not in (wire.VISIBILITY_PRIVATE, wire.VISIBILITY_PROTECTED, wire.VISIBILITY_PUBLIC):
                raise ValueError(f"Invalid Visibility: {definition.id}")
            if definition.visibility == wire.VISIBILITY_PRIVATE:
                continue
            catalog.operations.add().CopyFrom(definition.contract())
            _merge(files, definition.request, definition.response)
        for definition in self.requires:
            if definition.visibility == wire.VISIBILITY_PRIVATE:
                raise ValueError(f"Require {definition.id}: Private Calls Are Local")
            catalog.requires.add(id=definition.id, fingerprint=definition.fingerprint)
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
            catalog.streams.add().CopyFrom(definition.declaration())
            _merge(files, definition.row)
        for section in self.sections:
            catalog.sections.add(type=section.DESCRIPTOR.full_name, payload=section.SerializeToString())
            _merge(files, type(section))
        for entries, key in ((catalog.operations, "id"), (catalog.requires, "id"), (catalog.streams, "id"), (catalog.sections, "type")):
            names = [getattr(entry, key) for entry in entries]
            if any(not name for name in names) or len(set(names)) != len(names):
                raise ValueError("Duplicate or Empty Catalog Identifier")
        catalog.types.file.extend(files.values())
        pool = DescriptorPool()
        installed = set()

        def install(name):
            if name in installed:
                return
            definition = files[name]
            for dependency in definition.dependency:
                install(dependency)
            pool.Add(definition)
            installed.add(name)

        for name in files:
            install(name)
        catalog.digest = sha256(catalog.SerializeToString(deterministic=True)).digest()
        return catalog

    async def start(self, address: str) -> "Endpoint":
        """Bind the Author's Address; Stop Runs Cleanup and Keeps Serving."""
        service = _Service(self, self.catalog())
        server = grpc.aio.server()
        add_InstanceServicer_to_server(service, server)
        port = server.add_insecure_port(address)
        if not port:
            raise ValueError(f"Cannot Bind: {address}")
        await server.start()
        host = address.rsplit(":", 1)[0]
        return Endpoint(server, f"{host}:{port}")

    async def serve(self, address: str) -> None:
        endpoint = await self.start(address)
        try:
            await endpoint.server.wait_for_termination()
        finally:
            await endpoint.close()


class Endpoint:
    def __init__(self, server: grpc.aio.Server, address: str):
        self.server = server
        self.address = address

    async def close(self) -> None:
        await self.server.stop(None)


class _Fault(Exception):
    def __init__(self, code: grpc.StatusCode, message: str):
        self.code = code
        self.message = message


class _Service(InstanceServicer):
    def __init__(self, plugin: Plugin, catalog: wire.Catalog):
        self.description = carrier.InstanceGetResponse(
            id=plugin.id,
            version=plugin.version,
            started_at=STARTED_AT,
            catalog=catalog,
            revision=1,
            sdk="harmos_python",
            sdk_version=version("harmos-plugin"),
        )
        operations, streams, self.cleanup = plugin._registrations()
        self.local = {
            definition.id: (definition, handler) for definition, handler in operations if definition.visibility == wire.VISIBILITY_PRIVATE
        }
        self.operations = {
            definition.id: (definition, handler) for definition, handler in operations if definition.visibility != wire.VISIBILITY_PRIVATE
        }
        self.streams = {definition.id: (definition, handler, observing) for definition, handler, observing in streams}
        self.requires = {requirement.id: requirement.fingerprint for requirement in catalog.requires}

    async def Get(self, request, context):
        if request.revision == 0:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "Revision Must Be at Least One")
        return self.description

    async def Stop(self, request, context):
        try:
            for cleanup in self.cleanup:
                result = cleanup()
                if inspect.isawaitable(result):
                    await result
        except Exception as error:
            await context.abort(grpc.StatusCode.INTERNAL, str(error))
        return carrier.InstanceStopResponse()

    async def Invoke(self, request_iterator, context):
        first = await anext(request_iterator, None)
        if first is None or first.WhichOneof("frame") != "request":
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "Request Must Be First")
            return
        request = first.request
        if not request.invocation or request.attempt == 0:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "Invocation and Attempt Are Required")
        registered = self.operations.get(request.operation)
        if registered is None:
            await context.abort(grpc.StatusCode.NOT_FOUND, "Unknown Operation")
            return
        definition, handler = registered
        try:
            value = definition.request.FromString(request.payload)
        except DecodeError as error:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(error))

        await context.send_initial_metadata(())
        output = asyncio.Queue(maxsize=1)
        execution = Context(request, self.description.id, self.requires, output)
        execution._local = self.local
        running = asyncio.create_task(_execute(handler, execution, value, definition.response, output))
        reading = asyncio.create_task(_read_invoke(request_iterator, execution, output))
        try:
            while True:
                frame = await output.get()
                if isinstance(frame, _Fault):
                    await context.abort(frame.code, frame.message)
                await context.write(frame)
                if frame.WhichOneof("frame") == "response":
                    return
        finally:
            execution._output = None
            await _release(running, reading)

    async def Samples(self, request_iterator, context):
        first = await anext(request_iterator, None)
        if first is None or first.WhichOneof("frame") != "open":
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "Open Must Be First")
            return
        registered = self.streams.get(first.open.stream)
        if registered is None:
            await context.abort(grpc.StatusCode.NOT_FOUND, "Unknown Stream")
            return
        definition, handler, observing = registered
        await context.send_initial_metadata(())
        execution = Context(wire.Request(operation=first.open.stream, scope=first.open.scope), self.description.id, self.requires)
        output = asyncio.Queue(maxsize=1)
        ticks = asyncio.Queue(maxsize=1)
        reading = asyncio.create_task(_read_publish(request_iterator, observing, ticks, output))
        running = asyncio.create_task(_produce(handler, execution, definition.row, observing, ticks, output))
        sequence = 0
        try:
            while True:
                sample = await output.get()
                if sample is None:
                    return
                if isinstance(sample, _Fault):
                    await context.abort(sample.code, sample.message)
                sequence += 1
                sample.sequence = sequence
                await context.write(carrier.InstanceSamplesResponse(samples=[sample]))
        finally:
            await _release(running, reading)


# === Public Functions === #

# === Private Functions === #


def _merge(files: dict, *messages: type[Message]) -> None:
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


async def _release(*tasks: asyncio.Task) -> None:
    for task in tasks:
        task.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)


async def _execute(handler, execution, request, response, output):
    try:
        value = await handler(execution, request)
        if not isinstance(value, response):
            raise TypeError("Handler Returned the Wrong Protobuf Type")
        result = wire.Response(payload=value.SerializeToString())
    except Failure as failure:
        result = wire.Response(failure=failure.value)
    except (ProtocolError, DecodeError) as error:
        await output.put(_Fault(grpc.StatusCode.INVALID_ARGUMENT, str(error)))
        return
    except Exception as error:
        await output.put(_Fault(grpc.StatusCode.INTERNAL, str(error)))
        return
    await output.put(carrier.InstanceInvokeResponse(response=result))


async def _read_invoke(input, execution, output):
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
            await output.put(_Fault(grpc.StatusCode.UNAVAILABLE, "Host Closed Before Child Replies"))
    except ProtocolError as error:
        await output.put(_Fault(grpc.StatusCode.INVALID_ARGUMENT, str(error)))


async def _read_publish(input, observing, ticks, output):
    try:
        async for frame in input:
            if frame.WhichOneof("frame") != "tick":
                raise ProtocolError("Expected Tick")
            if not observing:
                raise ProtocolError("Device Streams Do Not Accept Ticks")
            if not frame.tick.HasField("at"):
                raise ProtocolError("Tick Time Required")
            validate_timestamp(frame.tick.at)
            await ticks.put(frame.tick.at)
        await output.put(None)
    except ProtocolError as error:
        await output.put(_Fault(grpc.StatusCode.INVALID_ARGUMENT, str(error)))


async def _produce(handler, execution, row_type, observing, ticks, output):
    try:
        if not observing:
            await handler(execution, Publisher(row_type, output))
            return
        while True:
            at = await ticks.get()
            execution.sequence += 1
            row = await handler(execution)
            if not isinstance(row, row_type):
                raise TypeError("Observer Returned the Wrong Protobuf Type")
            await output.put(wire.Sample(at=at, row=row.SerializeToString()))
    except Exception as error:
        await output.put(_Fault(grpc.StatusCode.INTERNAL, str(error)))


# === Entry Point === #
