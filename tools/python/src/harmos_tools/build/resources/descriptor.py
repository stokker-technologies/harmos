# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Descriptor Resource Contracts."""

# === Imports === #

# Standard Library Imports
import re
from typing import TypedDict

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FieldDescriptorProto, FileDescriptorSet

# Local Imports
from harmos_tools.source.protocol import caller_options

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Field(TypedDict):
    name: str
    kind: int
    type: str
    csharp: str
    repeated: bool
    optional: bool


class Method(TypedDict):
    owner: str
    face: str
    version: str
    parents: list[str]
    service: str
    method: str
    package: str
    path: list[str]
    request: str
    response: str
    request_csharp: str
    response_csharp: str
    client_streaming: bool
    server_streaming: bool
    file: str
    fields: list[Field]
    payload_field: Field | None


# === Public Functions === #


def table(descriptors: FileDescriptorSet) -> list[Method]:
    methods: list[Method] = []
    callers = caller_options(descriptors)
    leaves = set()
    csharp_types = {}
    definitions: dict[str, DescriptorProto] = {}

    def record_messages(messages: list[DescriptorProto], package: str, namespace: str) -> None:
        for message in messages:
            csharp_types[f"{package}.{message.name}"] = f"{namespace}.{message.name}"
            definitions[f"{package}.{message.name}"] = message
            record_messages(list(message.nested_type), f"{package}.{message.name}", f"{namespace}.{message.name}.Types")

    for file in descriptors.file:
        namespace = file.options.csharp_namespace or ".".join(part[0].upper() + part[1:] for part in file.package.split("."))
        record_messages(list(file.message_type), f".{file.package}", namespace)

    def field(value: FieldDescriptorProto) -> Field:
        return Field(
            name=value.name,
            kind=value.type,
            type=value.type_name.lstrip("."),
            csharp=csharp_types.get(value.type_name, ""),
            repeated=value.label == value.LABEL_REPEATED,
            optional=value.proto3_optional or value.name == "request_id",
        )

    for file in descriptors.file:
        parts = file.package.split(".")
        version = next((index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part)), None)
        if version is None or version == 0:
            continue
        boundary = parts[:version]
        executor = boundary[-1]
        owner = ".".join(boundary[:-1]) if executor in {"host", "plugin", "model"} else ".".join(boundary)
        for service in file.service:
            for method in service.method:
                response = definitions[method.output_type]
                payload = response.field[0] if len(response.field) == 1 else None
                path = [*parts[version + 1 :], snake(service.name), snake(method.name)]
                identity = f"{file.package}.{service.name}/{method.name}"
                audience = callers.get(identity, "CLIENTS")
                faces = (
                    ["plugin"]
                    if executor == "plugin"
                    else [
                        face for face, allowed in [("client", {"CLIENTS", "ALL"}), ("context", {"PLUGINS", "ALL"})] if audience in allowed
                    ]
                )
                for face in faces:
                    leaf = (face, *path)
                    if leaf in leaves:
                        raise ValueError(f"Client Resource Collision: {'.'.join(path)}")
                    leaves.add(leaf)
                    methods.append(
                        Method(
                            owner=owner,
                            face=face,
                            version=parts[version],
                            parents=parts[version + 1 :],
                            service=service.name,
                            method=method.name,
                            package=file.package,
                            path=path,
                            request=method.input_type.lstrip("."),
                            response=method.output_type.lstrip("."),
                            request_csharp=csharp_types[method.input_type],
                            response_csharp=csharp_types[method.output_type],
                            client_streaming=method.client_streaming,
                            server_streaming=method.server_streaming,
                            file=file.name,
                            fields=[field(value) for value in definitions[method.input_type].field],
                            payload_field=field(payload) if payload is not None else None,
                        )
                    )
    for leaf in leaves:
        if any(leaf[:length] in leaves for length in range(2, len(leaf))):
            raise ValueError(f"Client Resource Collision: {'.'.join(leaf[1:])}")
    return methods


def payload_audiences(descriptors: FileDescriptorSet) -> dict[str, set[str]]:
    definitions = {f"{file.package}.{definition.name}": definition for file in descriptors.file for definition in file.message_type}
    audiences: dict[str, set[str]] = {}
    for method in table(descriptors):
        if method["face"] == "plugin":
            continue
        pending = [method["request"], method["response"]]
        visited = set()
        while pending:
            name = pending.pop()
            if name in visited:
                continue
            visited.add(name)
            audiences.setdefault(name, set()).add("client" if method["face"] == "client" else "plugin")
            definition = definitions.get(name)
            if definition is not None:
                pending.extend(field.type_name.lstrip(".") for field in definition.field if field.type_name)
    return audiences


def snake(name: str) -> str:
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)).lower()


# === Private Functions === #

# === Entry Point === #
