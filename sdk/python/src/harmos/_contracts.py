# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Operation and Stream Declarations."""

# === Imports === #

# Standard Library Imports
from collections.abc import Callable
from dataclasses import dataclass
from hashlib import sha256
from typing import cast

# Third Party Imports
from google.protobuf.descriptor import Descriptor, EnumDescriptor
from google.protobuf.descriptor_pb2 import DescriptorProto, FileDescriptorProto, FileDescriptorSet
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from harmos._wire.model.v1.description_pb2 import LaneDeclaration
from harmos._wire.model.v1.operation_pb2 import OperationContract, Visibility
from harmos._wire.model.v1.policy_pb2 import OperationPolicy
from harmos._wire.model.v1.stream_pb2 import StreamDeclaration

# Local Imports
from ._columns import check_columns, columns_type

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


@dataclass(frozen=True, init=False)
class Lane:
    """A Plugin Lane Declaration; the Host Owns Admission."""

    name: str
    serial: bool
    concurrent: int | None

    def __init__(self, name: str, *, serial: bool = False, concurrent: int | None = None) -> None:
        if not name or name in ("Open", "harmos.resident") or any(ord(character) < 32 or ord(character) == 127 for character in name):
            raise ValueError(f"Declare Lane {name!r}: Choose a Nonempty, Nonreserved Name Without Control Characters")
        if concurrent is not None and (serial or concurrent <= 0 or concurrent > 0xFFFFFFFF):
            raise ValueError(f"Declare Lane {name!r}: Choose Either Serial or a Positive Concurrent Width")
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "serial", serial)
        object.__setattr__(self, "concurrent", concurrent)

    def declaration(self) -> LaneDeclaration:
        if self.serial:
            return LaneDeclaration(name=self.name, serial=True)
        if self.concurrent is not None:
            return LaneDeclaration(name=self.name, concurrent=self.concurrent)
        return LaneDeclaration(name=self.name, unbounded=True)


@dataclass(frozen=True)
class Operation[Q: Message, R: Message]:
    id: str
    request: type[Q]
    response: type[R]
    visibility: Visibility
    policy: OperationPolicy | None = None
    description: str = ""
    replay_safe: bool = False
    watch: bool = False
    lane: Lane | None = None

    @property
    def fingerprint(self) -> bytes:
        request = message_types(self.request).SerializeToString(deterministic=True)
        response = message_types(self.response).SerializeToString(deterministic=True)
        base = sha256(self.id.encode() + b"\0" + request + response).digest()
        if not self.watch:
            return base
        item = sha256(self.id.encode() + b"\0" + response + response).digest()
        return sha256(b"harmos.protobuf.stream.v1\0" + base + b"\0\1" + item).digest()

    def contract(self) -> OperationContract:
        return OperationContract(
            id=self.id,
            request=self.request.DESCRIPTOR.full_name,
            response=self.response.DESCRIPTOR.full_name,
            visibility=self.visibility,
            policy=self.policy,
            fingerprint=self.fingerprint,
            description=self.description,
            replay_safe=self.replay_safe,
            lane=self.lane.name if self.lane is not None else "",
        )


@dataclass(frozen=True)
class Stream:
    id: str
    row: type[Message]
    interval: Duration | None = None
    interval_setting: str = ""
    description: str = ""
    window_setting: str = ""
    columns: type[Message] | None = None

    def declaration(self) -> StreamDeclaration:
        return StreamDeclaration(
            id=self.id,
            row=self.row.DESCRIPTOR.full_name,
            interval=self.interval,
            interval_setting=self.interval_setting,
            description=self.description,
            window_setting=self.window_setting,
        )


@dataclass(frozen=True)
class Window:
    """One Granted Interval Window: `count` Samples From `start`, One `period` Apart."""

    start: Timestamp
    period: Duration
    count: int
    sequence: int


# === Public Functions === #


def operation[Q: Message, R: Message, F: Callable](
    id: str,
    *,
    request: type[Q],
    response: type[R],
    visibility: Visibility,
    policy: OperationPolicy | None = None,
    description: str = "",
    replay_safe: bool = False,
    lane: Lane | None = None,
) -> Callable[[F], F]:
    definition = Operation(id, request, response, visibility, policy, description, replay_safe, lane=lane)

    def decorate(handler: F) -> F:
        handler.__dict__["__harmos_operation__"] = definition
        return handler

    return decorate


def observer[F: Callable](
    id: str,
    *,
    row: type[Message],
    interval: Duration | None = None,
    interval_setting: str = "",
    window_setting: str = "",
    description: str = "",
) -> Callable[[F], F]:
    """Declare a One-Sample Observer, Called Once per Sample of Each Granted Window."""
    columns_type(row, id)
    definition = Stream(id, row, interval, interval_setting, description, window_setting)

    def decorate(handler: F) -> F:
        handler.__dict__["__harmos_observer__"] = definition
        return handler

    return decorate


def window_observer[F: Callable](
    id: str,
    *,
    row: type[Message],
    columns: type[Message],
    interval: Duration | None = None,
    interval_setting: str = "",
    window_setting: str = "",
    description: str = "",
) -> Callable[[F], F]:
    """Declare an Observer That Answers a Whole Granted Window With Its Columns."""
    columns_type(row, id)
    check_columns(row, columns, id)
    definition = Stream(id, row, interval, interval_setting, description, window_setting, columns)

    def decorate(handler: F) -> F:
        handler.__dict__["__harmos_observer__"] = definition
        return handler

    return decorate


def publisher[F: Callable](id: str, *, row: type[Message], description: str = "") -> Callable[[F], F]:
    definition = Stream(id, row, description=description)

    def decorate(handler: F) -> F:
        handler.__dict__["__harmos_publisher__"] = definition
        return handler

    return decorate


def message_types(message: type[Message]) -> FileDescriptorSet:
    """Project Reachable Definitions Without Compiler Presentation Metadata."""
    files: dict[str, FileDescriptorProto] = {}
    seen: set[str] = set()
    selected: dict[str, set[str]] = {}

    def visit(node: Descriptor | EnumDescriptor) -> None:
        root = node
        while root.containing_type is not None:
            root = root.containing_type
        if root.full_name in seen:
            return
        seen.add(root.full_name)
        source = FileDescriptorProto.FromString(root.file.serialized_pb)
        files[root.file.name] = source
        selected.setdefault(root.file.name, set()).add(root.name)
        if isinstance(root, Descriptor):
            visit_fields(root)

    def visit_fields(node: Descriptor) -> None:
        for field in (*node.fields, *node.extensions):
            if field.message_type is not None:
                visit(field.message_type)
            elif field.enum_type is not None:
                visit(field.enum_type)
            if field.is_extension and field.containing_type is not None:
                visit(field.containing_type)
        for child in node.nested_types:
            visit_fields(child)

    visit(cast(Descriptor, message.DESCRIPTOR))
    result = FileDescriptorSet()
    for name, source in sorted(files.items()):
        definition = result.file.add()
        definition.CopyFrom(source)
        definition.ClearField("message_type")
        definition.ClearField("enum_type")
        for item in sorted(source.message_type, key=lambda item: item.name.encode()):
            if item.name in selected[name]:
                definition.message_type.add().CopyFrom(item)
        for item in sorted(source.enum_type, key=lambda item: item.name.encode()):
            if item.name in selected[name]:
                definition.enum_type.add().CopyFrom(item)
        for field in ("options", "service", "extension", "source_code_info", "public_dependency", "weak_dependency", "dependency"):
            definition.ClearField(field)
        definition.dependency.extend(sorted(set(source.dependency) & files.keys()))
        for item in definition.message_type:
            _normalize(item)
    return result


# === Private Functions === #


def _normalize(message: DescriptorProto) -> None:
    for field in (*message.field, *message.extension):
        field.ClearField("json_name")
    for nested in message.nested_type:
        _normalize(nested)


# === Entry Point === #
