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
    visibility: wire.Visibility = wire.VISIBILITY_PUBLIC
    policy: wire.OperationPolicy | None = None
    description: str = ""

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
    visibility: wire.Visibility = wire.VISIBILITY_PUBLIC,
    policy: wire.OperationPolicy | None = None,
    description: str = "",
) -> Callable[[F], F]:
    definition = Operation(id, request, response, visibility, policy, description)

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
    result = descriptor_pb2.FileDescriptorSet()
    files = {}
    seen = set()

    def visit(node):
        if node.full_name in seen:
            return
        seen.add(node.full_name)
        file = node.file
        if file.name not in files:
            source = descriptor_pb2.FileDescriptorProto.FromString(file.serialized_pb)
            definition = result.file.add(name=file.name, package=file.package, syntax=source.syntax or "proto2")
            files[file.name] = definition
        definition = files[file.name]
        root = node
        while root.containing_type is not None:
            root = root.containing_type
        if root is not node:
            visit(root)
        if hasattr(root, "fields"):
            if not any(item.name == root.name for item in definition.message_type):
                item = definition.message_type.add()
                root.CopyToProto(item)
                _normalize(item)
            for field in getattr(node, "fields", ()):
                dependency = field.message_type or field.enum_type
                if dependency is None:
                    continue
                if dependency.file.name != file.name and dependency.file.name not in definition.dependency:
                    definition.dependency.append(dependency.file.name)
                visit(dependency)
            for nested in getattr(node, "nested_types", ()):
                visit(nested)
        elif not any(item.name == root.name for item in definition.enum_type):
            root.CopyToProto(definition.enum_type.add())

    visit(message.DESCRIPTOR)
    return result


# === Private Functions === #


def _normalize(message: descriptor_pb2.DescriptorProto) -> None:
    for field in message.field:
        field.ClearField("json_name")
    for nested in message.nested_type:
        _normalize(nested)


# === Entry Point === #
