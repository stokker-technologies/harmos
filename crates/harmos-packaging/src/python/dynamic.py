# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Project Live Finite Operation Contracts Into Private Protobuf Classes"""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import re
from collections.abc import Mapping
from typing import Any

# Third Party Imports
from google.protobuf import descriptor_pb2 as pb
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message import Message
from google.protobuf.message_factory import GetMessageClass

# Local Imports
from .operation import Mode, Operation

# Type Checking Imports

# === Public Exports === #

__all__ = ["from_contract"]

# === Constants === #

IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z_0-9]*\Z")
RESERVED_FIELDS = frozenset(
    {
        "ByteSize",
        "Clear",
        "ClearExtension",
        "ClearField",
        "CopyFrom",
        "DESCRIPTOR",
        "DiscardUnknownFields",
        "Extensions",
        "FindInitializationErrors",
        "FromString",
        "HasExtension",
        "HasField",
        "IsInitialized",
        "ListFields",
        "MergeFrom",
        "MergeFromString",
        "ParseFromString",
        "SerializePartialToString",
        "SerializeToString",
        "SetInParent",
        "UnknownFields",
        "WhichOneof",
    }
)
SCALARS = {
    "bool": pb.FieldDescriptorProto.TYPE_BOOL,
    "u32": pb.FieldDescriptorProto.TYPE_UINT32,
    "u64": pb.FieldDescriptorProto.TYPE_UINT64,
    "i32": pb.FieldDescriptorProto.TYPE_INT32,
    "i64": pb.FieldDescriptorProto.TYPE_INT64,
    "f32": pb.FieldDescriptorProto.TYPE_FLOAT,
    "f64": pb.FieldDescriptorProto.TYPE_DOUBLE,
    "string": pb.FieldDescriptorProto.TYPE_STRING,
    "bytes": pb.FieldDescriptorProto.TYPE_BYTES,
}

# === Types === #

# === Classes === #

# === Public Functions === #


def from_contract(
    contract: Mapping[str, Any],
    *,
    max_bytes: int = 4 * 1024 * 1024,
    max_work: int = 1024 * 1024,
    max_definitions: int = 65536,
) -> Operation[Message, Message, Message, Message]:
    """Use The Same Row, Presence And Stream Projection As Harmos Reflection"""
    if min(max_bytes, max_work, max_definitions) <= 0:
        raise ValueError("operation projection requires positive limits")
    chunks: list[str] = []
    size = 0
    for chunk in json.JSONEncoder(ensure_ascii=False, separators=(",", ":")).iterencode(
        contract
    ):
        size += len(chunk.encode("utf-8"))
        if size > max_bytes:
            raise ValueError("operation contract exceeds byte ceiling")
        chunks.append(chunk)
    encoded = "".join(chunks)
    if (
        set(contract) != {"operation", "version", "codec", "schema", "grpc"}
        or contract.get("codec") != "postcard"
    ):
        raise ValueError("invalid canonical operation contract")
    grpc = contract.get("grpc")
    if not isinstance(grpc, Mapping) or set(grpc) != {"package", "service", "method"}:
        raise ValueError("operation has no gRPC declaration")
    package, service, method = (
        grpc.get(key) for key in ("package", "service", "method")
    )
    if not isinstance(package, str) or not all(
        IDENTIFIER.fullmatch(part) for part in package.split(".")
    ):
        raise ValueError("invalid protobuf package")
    if not isinstance(service, str) or not IDENTIFIER.fullmatch(service):
        raise ValueError("invalid protobuf service")
    if not isinstance(method, str) or not IDENTIFIER.fullmatch(method):
        raise ValueError("invalid protobuf method")
    schema = contract.get("schema")
    if not isinstance(schema, Mapping) or set(schema) != {"root", "definitions"}:
        raise ValueError("operation has no finite schema")
    root, definitions = schema.get("root"), schema.get("definitions")
    if (
        not isinstance(root, int)
        or isinstance(root, bool)
        or not isinstance(definitions, list)
    ):
        raise TypeError("invalid finite schema table")
    if not 0 <= root < len(definitions) <= max_definitions:
        raise ValueError("invalid or excessive finite schema definitions")
    namespace = f"{package}.{service}Messages.{method}"
    file = pb.FileDescriptorProto(
        name=f"{namespace.replace('.', '/')}/messages.proto",
        package=namespace,
        syntax="proto3",
    )
    references: list[set[int]] = []
    work = len(namespace)
    admitted = len(definitions) * 128
    for index, fields in enumerate(definitions):
        if not isinstance(fields, list):
            raise TypeError("schema definition must contain fields")
        names: set[str] = set()
        projected_names: set[str] = set()
        tags: set[int] = set()
        refs: set[int] = set()
        variants = any(
            isinstance(field, Mapping) and field.get("cardinality") == "variant"
            for field in fields
        )
        row = file.message_type.add(name=_row(index, root))
        all_names = {
            _field_name(str(field.get("name")))
            for field in fields
            if isinstance(field, Mapping)
        }
        variant_name = _available("_harmos_variant", all_names)
        if variants:
            row.oneof_decl.add(name=variant_name)
        for field in fields:
            if not isinstance(field, Mapping) or set(field) != {
                "name",
                "tag",
                "kind",
                "cardinality",
            }:
                raise ValueError("invalid schema field shape")
            name, tag, kind, cardinality = (
                field[key] for key in ("name", "tag", "kind", "cardinality")
            )
            if (
                not isinstance(name, str)
                or not IDENTIFIER.fullmatch(name)
                or len(name.encode("utf-8")) > 256
                or name in names
            ):
                raise ValueError("invalid or duplicate protobuf field name")
            if (
                not isinstance(tag, int)
                or isinstance(tag, bool)
                or not 0 < tag <= 536870911
                or 19000 <= tag <= 19999
                or tag in tags
            ):
                raise ValueError("invalid or duplicate protobuf tag")
            if cardinality not in ("required", "optional", "repeated", "variant"):
                raise ValueError("invalid field cardinality")
            projected_name = _field_name(name)
            if projected_name in projected_names:
                raise ValueError(
                    "protobuf field names collide after reserved-member escaping"
                )
            extra = len(name) + 1 if projected_name != name else 0
            work += len(name.encode("utf-8")) * 2 + extra + len(namespace) + 32
            admitted += 256 + (len(name.encode("utf-8")) + extra) * 4
            if work > max_work or admitted > max_bytes:
                raise ValueError("schema projection exceeds preparation allowance")
            names.add(name)
            projected_names.add(projected_name)
            tags.add(tag)
            target = row.field.add(
                name=projected_name,
                number=tag,
                label=pb.FieldDescriptorProto.LABEL_REPEATED
                if cardinality == "repeated"
                else pb.FieldDescriptorProto.LABEL_OPTIONAL,
            )
            if projected_name != name:
                target.json_name = name
            if (
                isinstance(kind, Mapping)
                and set(kind) == {"type"}
                and isinstance(kind["type"], str)
                and kind["type"] in SCALARS
            ):
                target.type = SCALARS[kind["type"]]
            elif (
                isinstance(kind, Mapping)
                and set(kind) == {"type", "definition"}
                and kind["type"] == "message"
            ):
                reference = kind["definition"]
                if (
                    not isinstance(reference, int)
                    or isinstance(reference, bool)
                    or not 0 <= reference < len(definitions)
                ):
                    raise ValueError("invalid message reference")
                refs.add(reference)
                target.type = pb.FieldDescriptorProto.TYPE_MESSAGE
                target.type_name = f".{namespace}.{_row(reference, root)}"
            else:
                raise ValueError("invalid field kind")
            if variants:
                if (
                    cardinality != "variant"
                    or target.type != pb.FieldDescriptorProto.TYPE_MESSAGE
                ):
                    raise ValueError(
                        "enum definitions require message-valued variants only"
                    )
                target.oneof_index = 0
            elif (
                cardinality == "optional"
                and target.type != pb.FieldDescriptorProto.TYPE_MESSAGE
            ):
                target.oneof_index = len(row.oneof_decl)
                row.oneof_decl.add(
                    name=_available(f"_harmos_presence_{tag}", all_names)
                )
                target.proto3_optional = True
        references.append(refs)
    reachable, pending = set(), [root]
    while pending:
        current = pending.pop()
        if current not in reachable:
            reachable.add(current)
            pending.extend(references[current] - reachable)
    if len(reachable) != len(definitions):
        raise ValueError("finite schema contains unreachable definitions")
    directions = definitions[root]
    direction_names = [field["name"] for field in directions]
    if direction_names not in (
        ["request", "response"],
        ["request", "response", "input"],
        ["request", "response", "output"],
        ["request", "response", "input", "output"],
    ):
        raise ValueError("invalid operation directions")
    if any(
        field["tag"] != index + 1
        or field["cardinality"] != "required"
        or not isinstance(field["kind"], Mapping)
        or field["kind"].get("type") != "message"
        for index, field in enumerate(directions)
    ):
        raise ValueError("invalid operation direction declaration")
    direction_types = {
        field["name"]: f"{namespace}.{_row(field['kind']['definition'], root)}"
        for field in directions
    }
    service_file = pb.FileDescriptorProto(
        name=f"{package.replace('.', '/')}/{service}.proto",
        package=package,
        syntax="proto3",
        dependency=[file.name],
    )
    rpc_request = direction_types["request"]
    rpc_response = direction_types["response"]
    if "input" in direction_types:
        rpc_request = _envelope(
            service_file,
            f"{service}{method}Input",
            [
                ("request", direction_types["request"]),
                ("item", direction_types["input"]),
            ],
        )
    if "output" in direction_types:
        rpc_response = _envelope(
            service_file,
            f"{service}{method}Output",
            [
                ("item", direction_types["output"]),
                ("response", direction_types["response"]),
            ],
        )
    service_file.service.add(name=service).method.add(
        name=method,
        input_type=f".{rpc_request}",
        output_type=f".{rpc_response}",
        client_streaming="input" in direction_types,
        server_streaming="output" in direction_types,
    )
    serialized = file.SerializeToString()
    if len(serialized) > max_bytes:
        raise ValueError("protobuf descriptor exceeds byte ceiling")
    pool = DescriptorPool()
    pool.AddSerializedFile(serialized)
    service_bytes = service_file.SerializeToString()
    if len(serialized) + len(service_bytes) > max_bytes:
        raise ValueError("protobuf service exceeds byte ceiling")
    pool.AddSerializedFile(service_bytes)
    classes = {
        key: GetMessageClass(pool.FindMessageTypeByName(value))
        for key, value in direction_types.items()
    }
    operation, version = contract.get("operation"), contract.get("version")
    if (
        not isinstance(operation, Mapping)
        or not isinstance(operation.get("id"), str)
        or not operation["id"]
        or len(operation["id"].encode("utf-8")) > 256
        or not isinstance(version, int)
        or isinstance(version, bool)
        or not 0 < version <= 4294967295
    ):
        raise ValueError("invalid operation identity or version")
    mode: Mode = (
        "bidi"
        if "input" in classes and "output" in classes
        else "client_stream"
        if "input" in classes
        else "server_stream"
        if "output" in classes
        else "unary"
    )
    return Operation(
        id=operation["id"],
        version=version,
        mode=mode,
        codec="protobuf",
        request=classes["request"],
        response=classes["response"],
        input=classes.get("input"),
        output=classes.get("output"),
        grpc_path=f"/{package}.{service}/{method}",
        rpc_request=GetMessageClass(pool.FindMessageTypeByName(rpc_request)),
        rpc_response=GetMessageClass(pool.FindMessageTypeByName(rpc_response)),
        contract=encoded,
        fingerprint=hashlib.sha256(encoded.encode("utf-8")).hexdigest(),
    )


# === Private Functions === #


def _row(index: int, root: int) -> str:
    return f"Row{0 if index == root else index + 1 if index < root else index}"


def _available(name: str, names: set[str]) -> str:
    while name in names:
        name += "_"
    return name


def _envelope(
    file: pb.FileDescriptorProto, name: str, alternatives: list[tuple[str, str]]
) -> str:
    row = file.message_type.add(name=name)
    row.oneof_decl.add(name="value")
    for index, (field, message) in enumerate(alternatives, 1):
        row.field.add(
            name=field,
            number=index,
            label=pb.FieldDescriptorProto.LABEL_OPTIONAL,
            type=pb.FieldDescriptorProto.TYPE_MESSAGE,
            type_name=f".{message}",
            oneof_index=0,
        )
    return f"{file.package}.{name}"


# === Entry Point === #


def _field_name(name: str) -> str:
    return f"{name}_" if name in RESERVED_FIELDS else name
