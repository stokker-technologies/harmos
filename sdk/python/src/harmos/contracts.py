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

# Third Party Imports
from google.protobuf import descriptor_pb2
from google.protobuf.duration_pb2 import Duration
from google.protobuf.message import Message

# Local Imports
from .v1 import plugin_pb2 as wire

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class Operation[Q: Message, R: Message]:
    id: str
    request: type[Q]
    response: type[R]
    visibility: wire.Visibility = wire.VISIBILITY_PRIVATE
    policy: wire.OperationPolicy | None = None
    description: str = ""
    replay_safe: bool = False

    @property
    def fingerprint(self) -> bytes:
        request = message_types(self.request).SerializeToString(deterministic=True)
        response = message_types(self.response).SerializeToString(deterministic=True)
        return sha256(self.id.encode() + b"\0" + request + response).digest()

    def contract(self) -> wire.OperationContract:
        return wire.OperationContract(
            id=self.id,
            request=self.request.DESCRIPTOR.full_name,
            response=self.response.DESCRIPTOR.full_name,
            visibility=self.visibility,
            policy=self.policy,
            fingerprint=self.fingerprint,
            description=self.description,
            replay_safe=self.replay_safe,
        )


@dataclass(frozen=True)
class Stream:
    id: str
    row: type[Message]
    interval: Duration | None = None
    interval_setting: str = ""
    description: str = ""

    def declaration(self) -> wire.StreamDeclaration:
        return wire.StreamDeclaration(
            id=self.id,
            row=self.row.DESCRIPTOR.full_name,
            interval=self.interval,
            interval_setting=self.interval_setting,
            description=self.description,
        )


# === Public Functions === #


def operation[Q: Message, R: Message, F: Callable](
    id: str,
    *,
    request: type[Q],
    response: type[R],
    visibility: wire.Visibility = wire.VISIBILITY_PRIVATE,
    policy: wire.OperationPolicy | None = None,
    description: str = "",
    replay_safe: bool = False,
) -> Callable[[F], F]:
    definition = Operation(id, request, response, visibility, policy, description, replay_safe)

    def decorate(handler: F) -> F:
        handler.__dict__["__harmos_operation__"] = definition
        return handler

    return decorate


def observer[F: Callable](
    id: str, *, row: type[Message], interval: Duration | None = None, interval_setting: str = "", description: str = ""
) -> Callable[[F], F]:
    definition = Stream(id, row, interval, interval_setting, description)

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


def on_stop[F: Callable](handler: F) -> F:
    handler.__dict__["__harmos_stop__"] = True
    return handler


def message_types(message: type[Message]) -> descriptor_pb2.FileDescriptorSet:
    """Project Reachable Definitions Without Compiler Presentation Metadata."""
    files = {}
    seen = set()
    selected = {}

    def visit(node):
        root = node
        while root.containing_type is not None:
            root = root.containing_type
        if root.full_name in seen:
            return
        seen.add(root.full_name)
        source = descriptor_pb2.FileDescriptorProto.FromString(root.file.serialized_pb)
        files[root.file.name] = source
        selected.setdefault(root.file.name, set()).add(root.name)
        if hasattr(root, "fields"):
            visit_fields(root)

    def visit_fields(node):
        for field in (*node.fields, *node.extensions):
            if field.message_type is not None:
                visit(field.message_type)
            elif field.enum_type is not None:
                visit(field.enum_type)
            if field.is_extension:
                visit(field.containing_type)
        for child in node.nested_types:
            visit_fields(child)

    visit(message.DESCRIPTOR)
    result = descriptor_pb2.FileDescriptorSet()
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


def _normalize(message: descriptor_pb2.DescriptorProto) -> None:
    for field in (*message.field, *message.extension):
        field.ClearField("json_name")
    for nested in message.nested_type:
        _normalize(nested)


# === Entry Point === #
