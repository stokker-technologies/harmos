# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Schema Resource Surfaces."""

# === Imports === #

# Standard Library Imports
import re
from pathlib import Path
from typing import Any, TypedDict, cast

# isort: split

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FieldDescriptorProto, FileDescriptorSet
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message_factory import GetMessageClass

# isort: split

# Local Imports

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
    map: bool
    oneof: str
    wire_optional: bool
    copy: bool


class Method(TypedDict):
    surface: bool
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


def caller_options(descriptors: FileDescriptorSet) -> dict[str, str]:
    """Read the Annotated Caller Catalog Without Generated SDK Imports."""
    pool = _descriptor_inventory(descriptors)
    methods = [
        (f"{file.package}.{service.name}/{method.name}", method)
        for file in descriptors.file
        if "host" in Path(file.name).parts[:-1] or "harmos/model/v1/callers.proto" in file.dependency
        for service in file.service
        for method in service.method
    ]
    if not methods:
        return {}

    # Resolve the Shared Option Schema Once for the Entire Descriptor Inventory.
    try:
        extension = pool.FindExtensionByName("harmos.model.v1.callers")
    except KeyError as error:
        identity = methods[0][0]
        raise ValueError(
            f"Protocol Callers: {identity} Has No Callers Declaration; Correct the source protocol declarations or explicit public exports before regenerating"
        ) from error
    if extension.enum_type is None:
        raise ValueError(
            "Protocol Callers: Option Must Be an Enum; Correct the source protocol declarations or explicit public exports before regenerating"
        )
    options_type = GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))

    result = {}
    for identity, method in methods:
        options = cast(Any, options_type.FromString(method.options.SerializeToString()))
        value = options.Extensions[extension]
        allowed = extension.enum_type.values_by_number.get(value)
        if allowed is None:
            raise ValueError(
                f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        declared = options.HasExtension(extension)
        valid = allowed.name in {"CLIENTS", "PLUGINS", "ALL"}
        if not declared or not valid:
            raise ValueError(
                f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        result[identity] = allowed.name
    return result


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
            for enum in message.enum_type:
                csharp_types[f"{package}.{message.name}.{enum.name}"] = f"{namespace}.{message.name}.Types.{enum.name}"
            record_messages(list(message.nested_type), f"{package}.{message.name}", f"{namespace}.{message.name}.Types")

    for file in descriptors.file:
        namespace = file.options.csharp_namespace or ".".join(part[0].upper() + part[1:] for part in file.package.split("."))
        record_messages(list(file.message_type), f".{file.package}", namespace)
        for enum in file.enum_type:
            csharp_types[f".{file.package}.{enum.name}"] = f"{namespace}.{enum.name}"

    def is_copy(value: FieldDescriptorProto, pending: set[str]) -> bool:
        if value.label == value.LABEL_REPEATED or value.type in {value.TYPE_STRING, value.TYPE_BYTES}:
            return False
        if value.type != value.TYPE_MESSAGE:
            return True
        if value.type_name in pending:
            return False
        definition = definitions.get(value.type_name)
        return definition is not None and all(is_copy(child, pending | {value.type_name}) for child in definition.field)

    def field(value: FieldDescriptorProto) -> Field:
        definition = definitions.get(value.type_name)
        is_map = definition is not None and definition.options.map_entry
        item = definition.field[1] if is_map else value
        return Field(
            name=value.name,
            kind=item.type,
            type=item.type_name.lstrip("."),
            csharp=csharp_types.get(item.type_name, ""),
            repeated=value.label == value.LABEL_REPEATED and not is_map,
            optional=value.proto3_optional or value.name == "request_id",
            map=is_map,
            oneof="",
            wire_optional=value.proto3_optional,
            copy=is_copy(value, set()),
        )

    for file in descriptors.file:
        parts = file.package.split(".")
        version = next((index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part)), None)
        if version is None:
            continue
        if version == 0:
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
                    ["context"]
                    if executor == "plugin" and identity in callers
                    else ["plugin"]
                    if executor == "plugin"
                    else [
                        face for face, allowed in [("client", {"CLIENTS", "ALL"}), ("context", {"PLUGINS", "ALL"})] if audience in allowed
                    ]
                )
                for face in faces:
                    request_definition = definitions[method.input_type]
                    projected_fields = [field(value) for value in request_definition.field]
                    for item, value in zip(projected_fields, request_definition.field, strict=True):
                        if value.HasField("oneof_index") and not value.proto3_optional:
                            item["oneof"] = request_definition.oneof_decl[value.oneof_index].name
                            item["optional"] = True
                    leaf = (face, *path)
                    if leaf in leaves:
                        raise ValueError(
                            f"Generate Resource Bindings: Client Resource Collision: {'.'.join(path)}; Correct the Conflicting Protocol Paths Before Regenerating"
                        )
                    leaves.add(leaf)
                    methods.append(
                        Method(
                            surface=identity != "harmos.plugin.v1.Subscriptions/Subscribe",
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
                            fields=projected_fields,
                            payload_field=field(payload) if payload is not None else None,
                        )
                    )
    for leaf in leaves:
        if any(leaf[:length] in leaves for length in range(2, len(leaf))):
            raise ValueError(
                f"Generate Resource Bindings: Client Resource Collision: {'.'.join(leaf[1:])}; Correct the Conflicting Protocol Paths Before Regenerating"
            )
    return methods


def request_names(descriptors: FileDescriptorSet) -> set[str]:
    """Keep Native Host Requests and Carrier Requests Out of Public Payload Barrels."""
    names = {method["request"] for method in table(descriptors)}
    names.update(
        f"{file.package}.{message.name}"
        for file in descriptors.file
        if file.package.startswith("harmos.host.")
        for message in file.message_type
        if message.name.endswith("Request")
    )
    return names


def payload_audiences(descriptors: FileDescriptorSet, *, library: FileDescriptorSet | None = None) -> dict[str, set[str]]:
    definitions: dict[str, DescriptorProto] = {}

    def record(messages: list[DescriptorProto], package: str) -> None:
        for message in messages:
            name = f"{package}.{message.name}"
            definitions[name] = message
            record(list(message.nested_type), name)

    for file in descriptors.file:
        record(list(file.message_type), file.package)

    audiences: dict[str, set[str]] = {}
    inventory = FileDescriptorSet.FromString(descriptors.SerializeToString())
    originals = {file.name: file for file in library.file if file.package.startswith("harmos.")} if library is not None else {}
    for file in inventory.file:
        if file.name in originals:
            file.CopyFrom(originals[file.name])
    for method in table(inventory):
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
            parent = name.rsplit(".", 1)[0]
            while parent in definitions:
                audiences.setdefault(parent, set()).add("client" if method["face"] == "client" else "plugin")
                parent = parent.rsplit(".", 1)[0]
            definition = definitions.get(name)
            if definition is not None:
                pending.extend(field.type_name.lstrip(".") for field in definition.field if field.type_name)
    if library is not None:
        library_audiences = payload_audiences(library)
        for name in audiences:
            if name.startswith("harmos."):
                audiences[name] = library_audiences.get(name, {"client"})
    return audiences


def public_paths(descriptors: FileDescriptorSet, *, namespace: str = "harmos", reserved_names: set[str] | None = None) -> dict[str, str]:
    """Preserve Type Names; Put Collisions Under Their Unversioned Package Path."""
    definitions = [
        (file, definition)
        for file in descriptors.file
        if file.package.startswith((namespace + ".", "harmos."))
        for definition in [*file.message_type, *file.enum_type]
    ]
    names: dict[str, set[str]] = {}
    for file, definition in definitions:
        names.setdefault(definition.name, set()).add(file.package)
    reserved = {"Call", "Client", "ClientRoot", "Context", "Failure", "OperationError", "HostOperation", "HostRoot", "Plugin", "Window"}
    reserved.update(reserved_names or ())
    paths = {}
    for file, definition in definitions:
        parts = file.package.split(".")
        package = [part for part in parts[1:] if re.fullmatch(r"v[0-9]+", part) is None]
        if parts[0] != namespace:
            package.insert(0, parts[0])
        prefix = package if len(names[definition.name]) > 1 or definition.name in reserved else []
        paths[f"{file.package}.{definition.name}"] = ".".join([*prefix, definition.name])
    return paths


def snake(name: str) -> str:
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)).lower()


# === Private Functions === #


def _descriptor_inventory(descriptors: FileDescriptorSet) -> DescriptorPool:
    """Load Dependencies Before Their Users Without Losing Custom Options."""
    pool = DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        if not ready:
            raise ValueError(
                "Protocol Callers: Supply Resolvable Descriptor Dependencies; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    return pool


# === Entry Point === #
