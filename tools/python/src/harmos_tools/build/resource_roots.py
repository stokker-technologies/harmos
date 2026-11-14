# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Descriptor-Derived Resource Roots"""

# === Imports === #

# Standard Library Imports
import re
import subprocess
import sys
from argparse import ArgumentParser
from pathlib import Path
from typing import TypedDict

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FieldDescriptorProto, FileDescriptorSet

# Local Imports
from ..process import spawn_flags

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
        if boundary[-2:] == ["plugin", "host"]:
            face = "context"
            owner = ".".join(boundary[:-2])
        elif boundary[-1] == "plugin":
            face = "plugin"
            owner = ".".join(boundary[:-1])
        else:
            face = "client"
            owner = ".".join(boundary[:-1] if boundary[-1] == "client" else boundary)
        for service in file.service:
            for method in service.method:
                response = definitions[method.output_type]
                payload = response.field[0] if len(response.field) == 1 else None
                path = [*parts[version + 1 :], snake(service.name), snake(method.name)]
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


def snake(name: str) -> str:
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)).lower()


def generate(root: Path, descriptors: FileDescriptorSet | None = None, *, namespace: str = "harmos") -> None:
    if descriptors is None:
        descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
    for folder in [
        "sdk",
        "sdk/python/src/harmos",
        "sdk/rust/src/connection",
        "sdk/csharp/src/Harmos/Generated",
        "sdk/cpp/include/harmos/_generated",
    ]:
        (root / folder).mkdir(parents=True, exist_ok=True)
    for name in ("client.hpp", "client_wire.hpp", "host.hpp"):
        obsolete = root / "sdk/cpp/include/harmos/generated" / name
        if obsolete.is_file():
            obsolete.unlink()
    obsolete = root / "sdk/rust/src/connection/types.rs"
    if obsolete.is_file():
        obsolete.unlink()
    methods = table(descriptors)
    cpp_exports(root, descriptors)
    rust_exports(root, descriptors)
    python_exports(root / "sdk/python/src/harmos/_generated", descriptors)
    conformance = root / "sdk/rust/tests/fixtures/conformance/descriptors.bin"
    if conformance.is_file():
        schema = FileDescriptorSet.FromString(conformance.read_bytes())
        lines = [
            "// @generated from protocol descriptors; do not edit.",
            "#pragma once",
            '#include "harmos/conformance/v1/fixture.pb.h"',
            "namespace harmos::conformance {",
        ]
        lines += [
            f"using {message.name} = ::{file.package.replace('.', '::')}::{message.name};"
            for file in schema.file
            if file.package == "harmos.conformance.v1"
            for message in file.message_type
        ]
        lines += ["}"]
        target = root / "sdk/cpp/examples/conformance/_wire/types.hpp"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        facade = target.parent.parent / "generated/types.hpp"
        facade.parent.mkdir(parents=True, exist_ok=True)
        facade.write_text(
            '// @generated from protocol descriptors; do not edit.\n#pragma once\n#include "../_wire/types.hpp"\n',
            encoding="utf-8",
            newline="\n",
        )
        format_cpp([target, facade])
    clients = [method for method in methods if method["face"] == "client"]
    python(root / "sdk/python/src/harmos/_generated/client.py", clients, descriptors)
    if namespace == "harmos":
        python_host(
            root / "sdk/python/src/harmos/_generated/host.py", [method for method in methods if method["face"] == "context"], descriptors
        )
    rust(root, clients)
    if namespace == "harmos":
        rust_context(root, [method for method in methods if method["face"] == "context"])
        rust_host(root, [method for method in methods if method["face"] == "context"])
    csharp(root, clients, namespace=namespace)
    if namespace == "harmos":
        csharp_host(root, [method for method in methods if method["face"] == "context"])
    cpp(root, clients, namespace=namespace)
    if namespace == "harmos":
        cpp_host(root, [method for method in methods if method["face"] == "context"])
    fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
    if fixture.is_file():
        fixture_methods = clients + table(FileDescriptorSet.FromString(fixture.read_bytes()))
        merged = FileDescriptorSet()
        merged.file.extend(descriptors.file)
        merged.file.extend(FileDescriptorSet.FromString(fixture.read_bytes()).file)
        python(
            root / "sdk/python/tests/fixtures/client/_generated/client.py",
            fixture_methods,
            merged,
            module_prefixes={"application": "tests.fixtures.client._wire.application"},
        )
        for folder in ["sdk/csharp/tests/Harmos.Tests/Generated"]:
            (root / folder).mkdir(parents=True, exist_ok=True)
        rust(root, fixture_methods, target=fixture.parent / "resources.rs")
        csharp(root, fixture_methods, namespace="fixture", target=root / "sdk/csharp/tests/Harmos.Tests/Generated/Client.g.cs")
    paths = [
        root / "sdk/rust/src" / name
        for name in (
            "exports.rs",
            "client/types.rs",
            "client/resources.rs",
            "connection/resources.rs",
            "context/contracts.rs",
            "plugin/host.rs",
            "plugin/host_exports.rs",
            "plugin/types.rs",
        )
    ]
    paths = [path for path in paths if path.is_file()]
    if fixture.is_file():
        paths.extend([fixture.parent / "resources.rs", fixture.parent / "client_types.rs"])
    subprocess.run(
        ["rustfmt", "--edition", "2024", "--config-path", str(root / "sdk/rust/rustfmt.toml"), *map(str, paths)],
        check=True,
        creationflags=spawn_flags(),
    )


def python_exports(target: Path, descriptors: FileDescriptorSet) -> None:
    """Generate Private Import Tables for the Three Public Barrel Points."""
    methods = table(descriptors)
    requests = {method["request"] for method in methods}
    carriers = {
        "Request",
        "Response",
        "ChildRequest",
        "ChildResponse",
        "ChildCancel",
        "ChildWatch",
        "ChildWatchResponse",
        "ChildWatchCancel",
    }
    groups: dict[str, list[tuple[str, str, str]]] = {"shared": [], "client_types": [], "plugin_types": []}
    for file in descriptors.file:
        if not file.package.startswith("harmos.") or file.name.endswith("guest.proto"):
            continue
        group = "client_types" if ".client." in file.package else "plugin_types" if ".plugin.host." in file.package else "shared"
        if ".plugin." in file.package and ".plugin.host." not in file.package:
            continue
        module = file.name.removesuffix(".proto").replace("/", ".").replace("harmos.", "harmos._wire.", 1) + "_pb2"
        for definition in [*file.message_type, *file.enum_type]:
            if f"{file.package}.{definition.name}" in requests or definition.name in carriers:
                continue
            name = "FailureValue" if definition.name == "Failure" else definition.name
            groups[group].append((module, definition.name, name))
        for definition in file.enum_type:
            groups[group].extend((module, value.name, value.name) for value in definition.value)
    target.mkdir(parents=True, exist_ok=True)
    for group, entries in groups.items():
        lines = ["# @generated from protocol descriptors; do not edit."]
        lines += [f"from {module} import {original} as {name}" for module, original, name in sorted(entries)]
        lines += ["", "__all__ = [", *[f"    {name!r}," for name in sorted(name for _, _, name in entries)], "]", ""]
        (target / f"{group}.py").write_text("\n".join(lines), encoding="utf-8", newline="\n")

    # The Public Barrels Are Generated From the Same Inventory as Call Signatures.
    package = target.parent

    def barrel(path: Path, imports: list[str], names: list[str]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            "\n".join(
                [
                    "# @generated from protocol descriptors; do not edit.",
                    *imports,
                    "",
                    "__all__ = [",
                    *[f"    {name!r}," for name in sorted(names)],
                    "]",
                    "",
                ]
            ),
            encoding="utf-8",
            newline="\n",
        )

    shared = sorted(name for _, _, name in groups["shared"])
    barrel(
        package / "__init__.py",
        [
            "from ._connection import Client as Client",
            "from ._invocation import FailureError as Failure",
            "from ._invocation import ProtocolError as ProtocolError",
            *[f"from ._generated.shared import {name} as {name}" for name in shared],
        ],
        ["Client", "Failure", "ProtocolError", *shared],
    )
    client = sorted(name for _, _, name in groups["client_types"])
    nodes = {
        tuple(method["path"][:length])
        for method in methods
        if method["face"] == "client" and method["owner"] == "harmos"
        for length in range(1, len(method["path"]))
    }
    resources = sorted("".join(part.title().replace("_", "") for part in node) + "Resource" for node in nodes)
    barrel(
        package / "client.py",
        [
            *[f"from ._generated.client_types import {name} as {name}" for name in client],
            *[f"from ._generated.client import {name} as {name}" for name in resources],
        ],
        [*client, *resources],
    )
    plugin = sorted(name for _, _, name in groups["plugin_types"])
    authored = {
        "_authoring": ["Endpoint", "Plugin"],
        "_contracts": ["Operation", "Stream", "observer", "on_stop", "operation", "publisher"],
        "_invocation": ["Context", "Publisher"],
        "_generated.host": ["HostOperation"],
    }
    imports = [f"from ..{module} import {name} as {name}" for module, names in authored.items() for name in names]
    barrel(
        package / "plugin/__init__.py",
        [*imports, *[f"from .._generated.plugin_types import {name} as {name}" for name in plugin]],
        [*plugin, *[name for names in authored.values() for name in names]],
    )
    paths = [target / f"{group}.py" for group in groups] + [package / "__init__.py", package / "client.py", package / "plugin/__init__.py"]
    config = Path(__file__).resolve().parents[5] / "sdk/python/pyproject.toml"
    subprocess.run(
        [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", *map(str, paths)],
        check=True,
        creationflags=spawn_flags(),
    )
    subprocess.run(
        [sys.executable, "-m", "ruff", "format", "--config", str(config), *map(str, paths)], check=True, creationflags=spawn_flags()
    )


def python(target: Path, methods: list[Method], descriptors: FileDescriptorSet, *, module_prefixes: dict[str, str] | None = None) -> None:
    """Generate Content Arguments and Exactly-One-Field Response Projection."""
    nodes: set[tuple[str, ...]] = {()}
    leaves: set[tuple[str, ...]] = set()
    for method in methods:
        path = tuple(method["path"])
        if method["face"] != "client" or path in leaves:
            raise ValueError(f"Generate Python Client {'.'.join(path)}: Invalid or Duplicate Resource")
        leaves.add(path)
        if method["client_streaming"]:
            raise ValueError(f"Generate Python Client {method['method']}: Client Streaming Requires a Managed Contract")
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    if leaves & nodes:
        raise ValueError("Generate Python Client: Resource Collides With a Method")
    names = {node: "ClientRoot" if not node else "".join(part.title().replace("_", "") for part in node) + "Resource" for node in nodes}
    composed = any(method["owner"] != "harmos" for method in methods)
    library_nodes: set[tuple[str, ...]] = {()}
    for method in methods:
        if method["owner"] == "harmos":
            library_nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    messages: dict[str, tuple[str, str]] = {}

    def record(definitions: list[DescriptorProto], package: str, module: str, parent: str = "") -> None:
        for definition in definitions:
            name = parent + definition.name
            messages[f"{package}.{name}"] = (module, name)
            record(list(definition.nested_type), package, module, name + ".")

    for file in descriptors.file:
        module = file.name.removesuffix(".proto").replace("/", ".") + "_pb2"
        if module.startswith("harmos."):
            module = module.replace("harmos.", "harmos._wire.", 1)
        for prefix, replacement in (module_prefixes or {}).items():
            if module.startswith(prefix + "."):
                module = module.replace(prefix + ".", replacement + ".", 1)
        record(list(file.message_type), file.package, module)
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    aliases = {full: full.rsplit(".", 1)[-1] for full in used}
    for full in sorted(used):
        if sum(value == aliases[full] for value in aliases.values()) > 1:
            aliases[full] = (
                "".join(part.title().replace("_", "") for part in full.split(".")[:-1] if not re.fullmatch(r"v[0-9]+", part))
                + full.rsplit(".", 1)[-1]
            )

    def field_type(field: Field) -> str:
        value = (
            aliases[field["type"]]
            if field["type"]
            else {1: "float", 2: "float", 8: "bool", 9: "str", 12: "bytes"}.get(field["kind"], "int")
        )
        return f"Sequence[{value}]" if field["repeated"] else value

    lines = [
        "# @generated from protocol descriptors; do not edit.",
        '"""Typed Client Resources."""',
        "from collections.abc import AsyncGenerator, Sequence",
        "from uuid import uuid4",
        "from grpc import CallCredentials",
        "from grpc.aio import AioRpcError, Channel",
        "from harmos._client_transport import Metadata, call_error, stream, unary",
    ]
    if composed:
        lines += ["from harmos import Client as LibraryClient"]
        lines += [f"from harmos.client import {names[node]} as Library{names[node]}" for node in sorted(library_nodes) if node]
    for full in sorted(used):
        module, name = messages[full]
        if "." in name:
            parent, nested = name.split(".", 1)
            imported = "_" + parent + aliases[full]
            lines += [f"from {module} import {parent} as {imported}", f"{aliases[full]} = {imported}.{nested}"]
        else:
            lines += [f"from {module} import {name}" + (f" as {aliases[full]}" if aliases[full] != name else "")]
    for node in sorted(nodes, key=lambda value: (-len(value), value)):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        parent = "LibraryClient" if composed and not node else "Library" + names[node] if composed and node in library_nodes else ""
        lines += ["", f"class {names[node]}" + (f"({parent})" if parent else "") + ":", "    def __init__(self, channel: Channel) -> None:"]
        if parent:
            lines += ["        super().__init__(channel)"]
        lines += ["        self._channel = channel"]
        storage = "_composed_" if composed else "_library_"
        lines += [f"        self.{storage}{child[-1]} = {names[child]}(channel)" for child in children]
        for child in children:
            lines += ["", "    @property", f"    def {child[-1]}(self) -> {names[child]}:", f"        return self.{storage}{child[-1]}"]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            request, response = (aliases[method[key]] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            optional = [field for field in method["fields"] if field["optional"]]
            parameters = [f"{field['name']}: {field_type(field)}" for field in required]
            parameters += ["*"]
            parameters += [f"{field['name']}: {field_type(field)} | None = None" for field in optional]
            parameters += ["metadata: Metadata | None = None", "timeout: float | None = None", "credentials: CallCredentials | None = None"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            result = result.replace("Sequence[", "list[")
            stream_call = method["server_streaming"]
            returns = f"AsyncGenerator[{result}]" if stream_call else result
            lines += ["", f"    async def {method['path'][-1]}(self, {', '.join(parameters)}) -> {returns}:"]
            values = [
                f"{field['name']}="
                + ("request_id if request_id is not None else str(uuid4())" if field["name"] == "request_id" else field["name"])
                for field in method["fields"]
            ]
            lines += [f"        request = {request}({', '.join(values)})"]
            path = f"/{method['package']}.{method['service']}/{method['method']}"
            call = f"{'stream' if stream_call else 'unary'}(self._channel, {path!r}, request, {response}, metadata=metadata, timeout=timeout, credentials=credentials)"
            if stream_call:
                lines += [f"        call = {call}", "        try:", "            async for response in call:"]
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    lines += [
                        f'                if not response.HasField("{payload["name"]}"):',
                        f'                    raise ValueError("Read {method["service"]}.{method["method"]}: Missing Payload")',
                    ]
                value = f"response.{payload['name']}" if payload else "response"
                if payload and payload["repeated"]:
                    value = f"list({value})"
                lines += [
                    f"                yield {value}",
                    "        except AioRpcError as error:",
                    f"            raise call_error({'.'.join(method['path'])!r}, error) from error",
                    "        finally:",
                    "            call.cancel()",
                ]
            else:
                call = call.removesuffix(")") + f", action={'.'.join(method['path'])!r})"
                lines += [f"        response = await {call}"]
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    lines += [
                        f'        if not response.HasField("{payload["name"]}"):',
                        f'            raise ValueError("Read {method["service"]}.{method["method"]}: Missing Payload")',
                    ]
                value = f"response.{payload['name']}" if payload else "response"
                if payload and payload["repeated"]:
                    value = f"list({value})"
                lines += [f"        return {value}"]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    if composed and target.parent.name == "_generated":
        package = target.parent.parent
        requests = {method["request"] for method in methods}
        exports = []
        for full, (module, name) in sorted(messages.items()):
            if full.startswith(("harmos.", "google.")) or full in requests or "." in name:
                continue
            exports.append((module, name))
        private_types = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from {module} import {name} as {name}" for module, name in exports],
        ]
        (target.parent / "client_types.py").write_text("\n".join(private_types) + "\n", encoding="utf-8", newline="\n")
        client_exports = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from ._generated.client_types import {name} as {name}" for _, name in exports],
        ]
        (package / "client.py").write_text("\n".join(client_exports) + "\n", encoding="utf-8", newline="\n")
        (package / "__init__.py").write_text(
            "# @generated from protocol descriptors; do not edit.\nfrom ._generated.client import ClientRoot as Client\n\n__all__ = ['Client']\n",
            encoding="utf-8",
            newline="\n",
        )

    config = Path(__file__).resolve().parents[5] / "sdk/python/pyproject.toml"
    paths = [target] + (
        [target.parent / "client_types.py", target.parent.parent / "client.py", target.parent.parent / "__init__.py"]
        if composed and target.parent.name == "_generated"
        else []
    )
    subprocess.run(
        [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", *map(str, paths)],
        check=True,
        creationflags=spawn_flags(),
    )
    subprocess.run(
        [sys.executable, "-m", "ruff", "format", "--config", str(config), *map(str, paths)], check=True, creationflags=spawn_flags()
    )


def python_host(target: Path, methods: list[Method], descriptors: FileDescriptorSet) -> None:
    modules = {}
    for file in descriptors.file:
        module = file.name.removesuffix(".proto").replace("/", ".") + "_pb2"
        if module.startswith("harmos."):
            module = module.replace("harmos.", "harmos._wire.", 1)
        for definition in file.message_type:
            modules[f"{file.package}.{definition.name}"] = module
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )

    def field_type(field: Field) -> str:
        value = (
            field["type"].rsplit(".", 1)[-1]
            if field["type"]
            else {1: "float", 2: "float", 8: "bool", 9: "str", 12: "bytes"}.get(field["kind"], "int")
        )
        return f"list[{value}]" if field["repeated"] else value

    nodes = sorted({method["path"][0] for method in methods})
    lines = [
        "# @generated from protocol descriptors; do not edit.",
        "from collections.abc import AsyncGenerator",
        "from enum import Enum",
        "from typing import TYPE_CHECKING, cast",
        "from google.protobuf.message import Message",
        "from harmos._contracts import Operation",
        "from harmos._wire.v1.operation_pb2 import VISIBILITY_PUBLIC",
        "from harmos._wire.v1.policy_pb2 import OperationPolicy",
    ]
    lines += [f"from {modules[full]} import {full.rsplit('.', 1)[-1]}" for full in sorted(used)]
    lines += ["if TYPE_CHECKING:", "    from harmos._invocation import Context", "class HostOperation(Enum):"]
    for method in methods:
        name = "_".join(method["path"]).upper()
        lines += [f'    {name} = "{method["package"]}.{method["service"]}/{method["method"]}"']
    for method in methods:
        name = "_".join(method["path"]).upper()
        q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
        lines += [
            f"_{name} = Operation(HostOperation.{name}.value, {q}, {r}, visibility=VISIBILITY_PUBLIC, watch={method['server_streaming']})"
        ]
    lines += ["def host_requirement(operation: HostOperation) -> Operation[Message, Message]:", "    match operation:"]
    for method in methods:
        name = "_".join(method["path"]).upper()
        lines += [f"        case HostOperation.{name}:", f"            return cast(Operation[Message, Message], _{name})"]
    lines += [
        '    raise ValueError("Declare Host Requirement: Unknown Operation")',
        "class HostRoot:",
        "    def __init__(self, context: 'Context') -> None:",
    ]
    lines += [f"        self.{node} = Host{node.title()}Resource(context)" for node in nodes]
    for node in nodes:
        lines += [
            f"class Host{node.title()}Resource:",
            "    def __init__(self, context: 'Context') -> None:",
            "        self._context = context",
        ]
        for method in (method for method in methods if method["path"][0] == node):
            name = "_".join(method["path"]).upper()
            q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            parameters = [f"{field['name']}: {field_type(field)}" for field in method["fields"] if not field["optional"]]
            parameters += ["*"]
            parameters += [f"{field['name']}: {field_type(field)} | None = None" for field in method["fields"] if field["optional"]]
            if method["server_streaming"]:
                parameters += ["max_item_bytes: int"]
            else:
                parameters += ["policy: OperationPolicy | None = None"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else r
            if method["server_streaming"]:
                result = f"AsyncGenerator[{result}]"
            lines += [
                f"    async def {method['path'][-1]}(self{', ' if parameters else ''}{', '.join(parameters)}) -> {result}:",
                f"        request = {q}({', '.join(field['name'] + '=' + field['name'] for field in method['fields'])})",
            ]
            indent = "        "
            if method["server_streaming"]:
                lines += [
                    f"        call = self._context.watch(_{name}, request, max_item_bytes=max_item_bytes)",
                    "        try:",
                    "            async for response in call:",
                ]
                indent = "                "
            else:
                lines += [f"        response = await self._context.call(_{name}, request, policy=policy)"]
            if payload and payload["kind"] == 11 and not payload["repeated"]:
                lines += [
                    f'{indent}if not response.HasField("{payload["name"]}"):',
                    f'{indent}    raise ValueError("Read {name}: Missing Payload")',
                ]
            value = f"response.{payload['name']}" if payload else "response"
            if payload and payload["repeated"]:
                value = f"list({value})"
            lines += [f"{indent}{'yield' if method['server_streaming'] else 'return'} {value}"]
            if method["server_streaming"]:
                lines += ["        finally:", "            await call.aclose()"]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    config = Path(__file__).resolve().parents[5] / "sdk/python/pyproject.toml"
    subprocess.run(
        [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", str(target)],
        check=True,
        creationflags=spawn_flags(),
    )
    subprocess.run([sys.executable, "-m", "ruff", "format", "--config", str(config), str(target)], check=True, creationflags=spawn_flags())


def rust_exports(root: Path, descriptors: FileDescriptorSet) -> None:
    requests = {method["request"] for method in table(descriptors)}
    groups: dict[str, list[str]] = {"shared": [], "client": [], "plugin": []}
    for file in descriptors.file:
        if (
            not file.package.startswith("harmos.")
            or file.name.endswith("guest.proto")
            or (".plugin." in file.package and ".plugin.host." not in file.package)
        ):
            continue
        group = "client" if ".client." in file.package else "plugin" if ".plugin.host." in file.package else "shared"
        for definition in [*file.message_type, *file.enum_type]:
            full = f"{file.package}.{definition.name}"
            if full in requests or definition.name in {"Request", "Response"} or definition.name.startswith("Child"):
                continue
            source = (
                rust_message(full)
                .replace("harmos_protocol::v1", "crate::wire::shared")
                .replace("harmos_protocol::plugin::host", "crate::wire::host")
                .replace("harmos_protocol::client", "crate::wire::client")
            )
            groups[group].append(f"pub use {source};")
            if isinstance(definition, DescriptorProto) and (
                any(not nested.options.map_entry for nested in definition.nested_type)
                or definition.enum_type
                or any(field.HasField("oneof_index") and not field.proto3_optional for field in definition.field)
            ):
                parent = source.rsplit("::", 1)[0]
                groups[group].append(f"pub use {parent}::{snake(definition.name)};")
    for group, lines in groups.items():
        target = root / "sdk/rust/src" / ("exports.rs" if group == "shared" else f"{group}/types.rs")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            "\n".join(["// @generated from protocol descriptors; do not edit.", *sorted(lines), ""]), encoding="utf-8", newline="\n"
        )


def rust_message(full: str) -> str:
    if full.startswith("harmos.client.v1."):
        return "harmos_protocol::client::" + "::".join(full.split(".")[3:])
    if full.startswith("harmos.plugin.host.v1."):
        return "harmos_protocol::plugin::host::" + "::".join(full.split(".")[4:])
    if full.startswith("harmos.v1."):
        return "harmos_protocol::v1::" + full.rsplit(".", 1)[-1]
    if full.startswith("google.protobuf."):
        return "prost_types::" + full.rsplit(".", 1)[-1]
    parts = full.split(".")
    version = next(index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part))
    return snake(parts[0]) + "_protocol::" + "::".join(parts[version:])


def rust(root: Path, methods: list[Method], *, target: Path | None = None) -> None:
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    names = {node: "Root" if not node else "".join(part.title().replace("_", "") for part in node) for node in nodes}
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    aliases = {full: full.rsplit(".", 1)[-1] for full in used}
    duplicates = {name for name in aliases.values() if list(aliases.values()).count(name) > 1}
    for full in used:
        if aliases[full] in duplicates:
            aliases[full] = "".join(part.title() for part in full.split(".")[:-1] if not re.fullmatch(r"v[0-9]+", part)) + aliases[full]

    def field_type(field: Field) -> str:
        value = (
            aliases[field["type"]]
            if field["type"]
            else {
                1: "f64",
                2: "f32",
                8: "bool",
                9: "String",
                12: "Vec<u8>",
                4: "u64",
                13: "u32",
                6: "u64",
                7: "u32",
                5: "i32",
                15: "i32",
                17: "i32",
            }.get(field["kind"], "i64")
        )
        return f"Vec<{value}>" if field["repeated"] else value

    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use harmos::client::{CallOptions, ClientTransport, Watching, action_error, request_id};",
        "use prost_reflect::ReflectMessage;",
        "use tonic::Status as RpcStatus;",
        "use tonic::codegen::http::uri::PathAndQuery;",
    ]
    lines += [
        f"use {rust_message(full)}" + (f" as {aliases[full]}" if aliases[full] != full.rsplit(".", 1)[-1] else "") + ";"
        for full in sorted(used)
    ]
    for method in methods:
        name = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
        lines += ["#[derive(Default)]", f"pub struct {name} {{"]
        lines += [f"    pub {field['name']}: Option<{field_type(field)}>," for field in method["fields"] if field["optional"]]
        lines += ["    pub context: CallOptions,", "}"]
    for node in sorted(nodes):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        lines += ["#[derive(Clone)]", f"pub struct {names[node]} {{"]
        if operations:
            lines += ["    client: ClientTransport,"]
        lines += [f"    pub {child[-1]}: {names[child]}," for child in children]
        lines += ["}", f"impl {names[node]} {{", "    pub fn new(client: ClientTransport) -> Self {", "        Self {"]
        lines += [f"            {child[-1]}: {names[child]}::new(client.clone())," for child in children]
        if operations:
            lines += ["            client,"]
        lines += ["        }", "    }"]
        for method in operations:
            if method["client_streaming"]:
                raise ValueError(f"Generate Rust Client {method['method']}: Managed Contract Required")
            request, response = (aliases[method[key]] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            parameters = [
                f"{field['name']}: " + ("impl Into<String>" if field_type(field) == "String" else field_type(field)) for field in required
            ]
            args = [field["name"] for field in required]
            options = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            if method["server_streaming"]:
                result = f"Watching<{result}>"
            name = method["path"][-1]
            lines += [
                f"    pub async fn {name}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result} , RpcStatus> {{",
                f"        self.{name}_with({', '.join([*args, options + '::default()'])}).await",
                "    }",
            ]
            lines += [
                f"    pub async fn {name}_with(&self, {', '.join([*parameters, 'options: ' + options])}) -> Result<{result} , RpcStatus> {{",
                f"        let value = {request} {{",
            ]
            for field in method["fields"]:
                name = field["name"]
                if name == "request_id":
                    expression = "match options.request_id { Some(value) => value, None => request_id()?, }"
                elif field["optional"]:
                    expression = f"options.{name}"
                elif field["kind"] == 11 and not field["repeated"]:
                    expression = f"Some({name})"
                elif field["kind"] == 9:
                    expression = f"{name}.into()"
                else:
                    expression = name
                lines += [f"            {name}," if expression == name else f"            {name}: {expression},"]
            lines += ["        };", "        let request = options.context.request(value);"]
            path = f"/{method['package']}.{method['service']}/{method['method']}"
            call = "watch_known" if method["server_streaming"] else "unary"
            lines += [
                f'        let response = self.client.{call}::<{request}, {response}>(PathAndQuery::from_static({path!r}), request, {response}::default().descriptor()).await.map_err(|error| action_error("{".".join(method["path"])}", error))?;'.replace(
                    repr(path), '"' + path + '"'
                )
            ]
            if method["server_streaming"]:
                lines += [f'        let response = response.with_action("{".".join(method["path"])}");']
                if payload:
                    field = payload["name"]
                    value = f"response.{field}"
                    if payload["kind"] == 11 and not payload["repeated"]:
                        value += f'.ok_or_else(|| RpcStatus::data_loss("Read {method["service"]}.{method["method"]}: Missing Payload"))'
                    else:
                        value = f"Ok({value})"
                    lines += [f"        Ok(response.map(|response| {value}))"]
                else:
                    lines += ["        Ok(response)"]
            else:
                lines += ["        let response = response.into_inner();"]
                if payload:
                    value = f"response.{payload['name']}"
                    if payload["kind"] == 11 and not payload["repeated"]:
                        lines += [
                            f'        {value}.ok_or_else(|| RpcStatus::data_loss("Read {method["service"]}.{method["method"]}: Missing Payload"))'
                        ]
                    else:
                        lines += [f"        Ok({value})"]
                else:
                    lines += ["        Ok(response)"]
            lines += ["    }"]
        lines += ["}"]
    destination = target or root / "sdk/rust/src/connection/resources.rs"
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    if target is not None:
        exports = sorted({method["response"] for method in methods if method["owner"] != "harmos"})
        facade = ["// @generated from protocol descriptors; do not edit."]
        facade += [f"pub use {re.sub(r'^\w+_protocol::v[0-9]+', 'crate::wire', rust_message(name))};" for name in exports]
        (target.parent / "client_types.rs").write_text("\n".join(facade) + "\n", encoding="utf-8", newline="\n")
    if target is None:
        exports = [*names.values(), *("".join(part.title().replace("_", "") for part in method["path"]) + "Options" for method in methods)]
        (root / "sdk/rust/src/client/resources.rs").write_text(
            "// @generated from protocol descriptors; do not edit.\npub use crate::connection::resources::{"
            + ", ".join(sorted(exports))
            + "};\n",
            encoding="utf-8",
            newline="\n",
        )


def rust_context(root: Path, methods: list[Method]) -> None:
    lines = ["// @generated from the protocol descriptor table; do not edit.", "use crate::{Message, OperationRequest};"]
    messages = sorted(
        {method[field] for method in methods for field in ("request", "response")}
        | {"harmos.v1.SettingDeclaration", "harmos.v1.SettingsDeclarations"}
    )

    lines += ["use prost::Message as _;", "use prost_types::FileDescriptorSet;"]
    lines += [f"use {rust_message(message)};" for message in messages]

    for message in messages:
        lines += [
            f"impl Message for {message.rsplit('.', 1)[-1]} {{",
            f'    fn name() -> String {{ "{message}".into() }}',
            "    fn types() -> FileDescriptorSet {",
            f'        FileDescriptorSet::decode(&include_bytes!("types/{message}.bin")[..]).expect("generated context wrapper")',
            "    }",
            "}",
        ]
    for method in methods:
        lines += [
            f"impl OperationRequest for {method['request'].rsplit('.', 1)[-1]} {{",
            f"    type Response = {method['response'].rsplit('.', 1)[-1]};",
            f'    const ID: &\'static str = "{method["package"]}.{method["service"]}/{method["method"]}";',
            f"    const WATCH: bool = {str(method['server_streaming']).lower()};",
            "}",
        ]
    (root / "sdk/rust/src/context/contracts.rs").parent.mkdir(parents=True, exist_ok=True)
    (root / "sdk/rust/src/context/contracts.rs").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def rust_host(root: Path, methods: list[Method]) -> None:
    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: Field) -> str:
        value = (
            field["type"].rsplit(".", 1)[-1]
            if field["type"]
            else {1: "f64", 2: "f32", 8: "bool", 9: "String", 12: "Vec<u8>", 4: "u64", 13: "u32"}.get(field["kind"], "i64")
        )
        return f"Vec<{value}>" if field["repeated"] else value

    nodes = sorted({method["path"][0] for method in methods})
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use crate::{Context, Error, Result, Plugin, OperationPolicy};",
        "use std::num::NonZeroUsize;",
    ]
    lines += [f"use {rust_message(full)};" for full in sorted(used)]
    lines += ["pub enum HostOperation {"]
    lines += ["    " + "".join(title(part) for part in method["path"]) + "," for method in methods]
    lines += ["}", "impl Plugin {", "    pub fn requires_host(self, operation: HostOperation) -> Self {", "        match operation {"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += [f"            HostOperation::{name} => self.requires::<{method['request'].rsplit('.', 1)[-1]}>(),"]
    lines += [
        "        }",
        "    }",
        "}",
        "impl Context {",
        "    pub fn host(&self) -> HostRoot {",
        "        HostRoot::new(self.clone())",
        "    }",
        "}",
        "#[derive(Clone)]",
        "pub struct HostRoot {",
    ]
    lines += [f"    pub {node}: Host{title(node)}Resource," for node in nodes]
    lines += ["}", "impl HostRoot {", "    fn new(context: Context) -> Self {", "        Self {"]
    lines += [f"            {node}: Host{title(node)}Resource {{ context: context.clone() }}," for node in nodes]
    lines += ["        }", "    }", "}"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += ["#[derive(Default)]", f"pub struct Host{name}Options {{"]
        lines += [f"    pub {field['name']}: Option<{field_type(field)}>," for field in method["fields"] if field["optional"]]
        lines += (["    pub policy: Option<OperationPolicy>,"] if not method["server_streaming"] else []) + ["}"]
        if method["server_streaming"]:
            q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            payload = method["payload_field"]
            result = field_type(payload) if payload else r
            lines += [
                f"pub struct Host{name} {{",
                f"    incoming: crate::context::Watching<{r}>,",
                "}",
                f"impl Host{name} {{",
                f"    pub async fn next(&mut self) -> Result<Option<{result}>> {{",
                "        let Some(response) = self.incoming.next().await? else { return Ok(None); };",
            ]
            if payload and payload["kind"] == 11:
                lines += [
                    f'        response.{payload["name"]}.map(Some).ok_or_else(|| Error::Invalid("Read {name}: Missing Payload".into()))'
                ]
            else:
                lines += [f"        Ok(Some(response{'.' + payload['name'] if payload else ''}))"]
            lines += ["    }", "}"]
    for node in nodes:
        lines += [
            "#[derive(Clone)]",
            f"pub struct Host{title(node)}Resource {{",
            "    context: Context,",
            "}",
            f"impl Host{title(node)}Resource {{",
        ]
        for method in (method for method in methods if method["path"][0] == node):
            name = "".join(title(part) for part in method["path"])
            q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            parameters = [f"{field['name']}: {field_type(field)}" for field in required]
            args = [field["name"] for field in required]
            if method["server_streaming"]:
                parameters += ["max_item_bytes: NonZeroUsize"]
                args += ["max_item_bytes"]
            payload = method["payload_field"]
            result = f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else r
            action = method["path"][-1]
            lines += [
                f"    pub async fn {action}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result}> {{",
                f"        self.{action}_with({', '.join([*args, 'Host' + name + 'Options::default()'])}).await",
                "    }",
                f"    pub async fn {action}_with(&self, {', '.join([*parameters, 'options: Host' + name + 'Options'])}) -> Result<{result}> {{",
                f"        let request = {q} {{",
            ]
            for field in method["fields"]:
                value = (
                    f"options.{field['name']}" if field["optional"] else f"Some({field['name']})" if field["kind"] == 11 else field["name"]
                )
                lines += [f"            {field['name']}," if value == field["name"] else f"            {field['name']}: {value},"]
            lines += ["        };"]
            if method["server_streaming"]:
                lines += [
                    f"        let Host{name}Options {{}} = options;",
                    f"        Ok(Host{name} {{ incoming: self.context.watch(request, max_item_bytes).await? }})",
                ]
            else:
                lines += [f"        let response: {r} = self.context.call_with(request, options.policy).await?;"]
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    lines += [f'        response.{payload["name"]}.ok_or_else(|| Error::Invalid("Read {name}: Missing Payload".into()))']
                else:
                    lines += [f"        Ok(response{'.' + payload['name'] if payload else ''})"]
            lines += ["    }"]
        lines += ["}"]
    target = root / "sdk/rust/src/plugin/host.rs"
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    exports = ["HostOperation", "HostRoot", *(f"Host{title(node)}Resource" for node in nodes)]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        exports += [f"Host{name}Options"]
        if method["server_streaming"]:
            exports += [f"Host{name}"]
    (target.parent / "host_exports.rs").write_text(
        "// @generated from protocol descriptors; do not edit.\npub use host::{" + ", ".join(sorted(exports)) + "};\n",
        encoding="utf-8",
        newline="\n",
    )


def csharp_message(name: str) -> str:
    return (
        name.replace("Harmos.Plugin.Host.V1", "Harmos.Plugins")
        .replace("Harmos.Plugin.V1", "Harmos.Internal.Plugin")
        .replace("Harmos.Client.V1.StatusResources", "Harmos.Clients")
        .replace("Harmos.Client.V1.PluginsResources", "Harmos.Clients")
        .replace("Harmos.Client.V1.AccessResources", "Harmos.Clients")
        .replace("Harmos.Client.V1", "Harmos.Clients")
        .replace("Harmos.V1", "Harmos")
    )


def csharp(root: Path, methods: list[Method], *, namespace: str = "harmos", target: Path | None = None, check: bool = False) -> None:
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    def title(part: str) -> str:
        return "".join(word.title() for word in part.split("_"))

    def parameter(name: str) -> str:
        value = title(name)
        return value[0].lower() + value[1:]

    names = {node: "Client" if not node else "".join(title(part) for part in node) + "Resource" for node in nodes}
    used = {method[key + "_csharp"] for method in methods for key in ("request", "response")}
    used.update(
        field["csharp"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["csharp"]
    )
    aliases = {full: full.rsplit(".", 1)[-1] for full in used}
    duplicates = {name for name in aliases.values() if list(aliases.values()).count(name) > 1}
    for full in used:
        if aliases[full] in duplicates:
            aliases[full] = full.replace(".", "").replace("V1", "")

    def field_type(field: Field) -> str:
        value = (
            aliases[field["csharp"]]
            if field["csharp"]
            else {
                1: "double",
                2: "float",
                8: "bool",
                9: "string",
                12: "ByteString",
                4: "ulong",
                13: "uint",
                6: "ulong",
                7: "uint",
                5: "int",
                15: "int",
                17: "int",
            }.get(field["kind"], "long")
        )
        if field["type"].startswith("google.protobuf."):
            value = field["type"].rsplit(".", 1)[-1]
        return f"IReadOnlyList<{value}>" if field["repeated"] else value

    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#nullable enable",
        "using System;",
        "using System.Collections.Generic;",
        "using System.Threading;",
        "using System.Threading.Tasks;",
        "using System.Runtime.CompilerServices;",
        "using Grpc.Core;",
        "using Grpc.Net.Client;",
        "using Google.Protobuf;",
        "using Google.Protobuf.WellKnownTypes;",
        "using StorageTransferSettings = global::Harmos.StorageTransferSettings;",
        "using Transfer = global::Harmos.Transfer;",
    ]
    lines += [f"using {aliases[full]} = global::{csharp_message(full)};" for full in sorted(used)]
    lines += [f"namespace {'.'.join(part.title() for part in namespace.split('.'))};", ""]
    for node in sorted(nodes):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        builtin = not node
        lines += [f"public sealed class {names[node]}" + (" : IDisposable" if builtin else ""), "{"]
        if builtin:
            lines += ["  private GrpcChannel? ownedChannel;"]
        if operations:
            lines += ["  private readonly CallInvoker invoker;"]
        lines += [f"  public {names[child]} {title(child[-1])} {{ get; }}" for child in children]
        lines += [f"  public {names[node]}(CallInvoker invoker)", "  {"]
        if operations:
            lines += ["    this.invoker = invoker;"]
        lines += [f"    {title(child[-1])} = new {names[child]}(invoker);" for child in children]
        lines += ["  }"]
        if builtin:
            lines += [
                "  public static Client Connect(string address, StorageTransferSettings? transfer = null, int? messageBytes = null)",
                "  {",
                "    var maximum = Transfer.MessageBytes(transfer, messageBytes);",
                "    var channel = GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });",
                "    return new Client(channel.CreateCallInvoker()) { ownedChannel = channel };",
                "  }",
                "  public void Dispose() => ownedChannel?.Dispose();",
            ]
        for method in operations:
            request, response = (aliases[method[key + "_csharp"]] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            optional = [field for field in method["fields"] if field["optional"]]
            parameters = [f"{field_type(field)} {parameter(field['name'])}" for field in required]
            parameters += [f"{field_type(field)}? {parameter(field['name'])} = null" for field in optional]
            parameters += ["CallOptions context = default"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            stream = method["server_streaming"]
            if stream:
                parameters += ["[EnumeratorCancellation] CancellationToken cancellationToken = default"]
            returns = f"IAsyncEnumerable<{result}>" if stream else f"Task<{result}>"
            kind = "ServerStreaming" if stream else "Unary"
            lines += [
                f"  public async {returns} {method['method']}({', '.join(parameters)})",
                "  {",
                f"    var request = new {request}",
                "    {",
            ]
            for field in method["fields"]:
                name = parameter(field["name"])
                if field["optional"] and name != "requestId":
                    continue
                value = 'requestId ?? Guid.NewGuid().ToString("N")' if name == "requestId" else name
                lines += [f"      {title(field['name'])} = {value},"]
            lines += ["    };"]
            for field in optional:
                if field["name"] == "request_id":
                    continue
                name = parameter(field["name"])
                value = name if field["kind"] in (9, 11, 12) else name + ".Value"
                lines += [f"    if ({name} is not null)", "    {", f"      request.{title(name)} = {value};", "    }"]
            lines += [
                f'    var method = new Method<{request}, {response}>(MethodType.{kind}, "{method["package"]}.{method["service"]}", "{method["method"]}",',
                f"        Marshallers.Create<{request}>(value => value.ToByteArray(), bytes => {request}.Parser.ParseFrom(bytes)),",
                f"        Marshallers.Create<{response}>(value => value.ToByteArray(), bytes => {response}.Parser.ParseFrom(bytes)));",
            ]
            if stream:
                lines += [
                    "    using var linked = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken, cancellationToken);",
                    "    using var call = invoker.AsyncServerStreamingCall(method, null, context.WithCancellationToken(linked.Token), request);",
                    "    while (true)",
                    "    {",
                    "      bool available;",
                    "      try",
                    "      {",
                    "        available = await call.ResponseStream.MoveNext(linked.Token);",
                    "      }",
                    "      catch (RpcException error)",
                    "      {",
                    f'        throw new RpcException(new Status(error.StatusCode, "Call {".".join(method["path"])}: " + error.Status.Detail), error.Trailers);',
                    "      }",
                    "      if (!available)",
                    "      {",
                    "        yield break;",
                    "      }",
                    "      var response = call.ResponseStream.Current;",
                ]
                value = f"response.{title(payload['name'])}" if payload else "response"
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    value += f' ?? throw new InvalidOperationException("Read {method["service"]}.{method["method"]}: Missing Payload")'
                lines += [f"      yield return {value};", "    }"]
            else:
                lines += [
                    "    using var call = invoker.AsyncUnaryCall(method, null, context, request);",
                    f"    {response} response;",
                    "    try",
                    "    {",
                    "      response = await call.ResponseAsync;",
                    "    }",
                    "    catch (RpcException error)",
                    "    {",
                    f'      throw new RpcException(new Status(error.StatusCode, "Call {".".join(method["path"])}: " + error.Status.Detail), error.Trailers);',
                    "    }",
                ]
                value = f"response.{title(payload['name'])}" if payload else "response"
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    value += f' ?? throw new InvalidOperationException("Read {method["service"]}.{method["method"]}: Missing Payload")'
                lines += [f"    return {value};"]
            lines += ["  }"]
        lines += ["}"]
    destination = target or root / "sdk/csharp/src/Harmos/Generated/Client.g.cs"
    source = "\n".join(lines) + "\n"
    if check:
        if not destination.is_file() or destination.read_text(encoding="utf-8") != source:
            raise ValueError(f"Check Generated C# Client: Drift at {destination}; Run mise run generate -- --sdk csharp")
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(source, encoding="utf-8", newline="\n")


def csharp_host(root: Path, methods: list[Method], *, check: bool = False) -> None:
    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: Field) -> str:
        if field["type"]:
            value = field["type"].rsplit(".", 1)[-1]
        else:
            value = {1: "double", 2: "float", 8: "bool", 9: "string", 12: "ByteString", 4: "ulong", 13: "uint"}.get(field["kind"], "long")
        return f"IReadOnlyList<{value}>" if field["repeated"] else value

    nodes = sorted({method["path"][0] for method in methods})
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#nullable enable",
        "using System;",
        "using System.Collections.Generic;",
        "using System.Threading;",
        "using System.Threading.Tasks;",
        "using System.Runtime.CompilerServices;",
        "using Google.Protobuf;",
        "using Google.Protobuf.WellKnownTypes;",
        "namespace Harmos.Plugins;",
        "public enum HostOperation",
        "{",
    ]
    lines += [f"  {''.join(title(part) for part in method['path'])}," for method in methods]
    lines += ["}", "internal static class HostContracts", "{"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
        lines += [
            f'  internal static readonly Operation<{q}, {r}> {name} = new("{method["package"]}.{method["service"]}/{method["method"]}", Visibility.Public, watch: {str(method["server_streaming"]).lower()});'
        ]
    lines += [
        "}",
        "public static class HostRequirements",
        "{",
        "  public static Plugin Requires(this Plugin plugin, HostOperation operation) => operation switch",
        "  {",
    ]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += [f"    HostOperation.{name} => plugin.Requires(HostContracts.{name}),"]
    lines += [
        '    _ => throw new ArgumentOutOfRangeException(nameof(operation), "Declare Host Requirement: Unknown Operation")',
        "  };",
        "}",
        "public sealed class HostRoot(Context context)",
        "{",
    ]
    lines += [f"  public Host{title(node)}Resource {title(node)} {{ get; }} = new(context);" for node in nodes]
    lines += ["}"]
    for node in nodes:
        lines += [f"public sealed class Host{title(node)}Resource(Context context)", "{"]
        for method in (method for method in methods if method["path"][0] == node):
            q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            parameters = [f"{field_type(field)} {field['name']}" for field in method["fields"] if not field["optional"]]
            stream = method["server_streaming"]
            if stream:
                parameters += ["ulong maxItemBytes"]
            parameters += [f"{field_type(field)}? {field['name']} = null" for field in method["fields"] if field["optional"]]
            if not stream:
                parameters += ["OperationPolicy? policy = null"]
            parameters += [("[EnumeratorCancellation] " if stream else "") + "CancellationToken cancellationToken = default"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else r
            name = "".join(title(part) for part in method["path"])
            result = f"IAsyncEnumerable<{result}>" if stream else f"Task<{result}>"
            lines += [f"  public async {result} {method['method']}({', '.join(parameters)})", "  {", f"    var request = new {q}", "    {"]
            lines += [f"      {title(field['name'])} = {field['name']}," for field in method["fields"]]
            lines += ["    };"]
            value = f"response.{title(payload['name'])}" if payload else "response"
            if payload and payload["kind"] == 11 and not payload["repeated"]:
                value += f' ?? throw new InvalidOperationException("Read {name}: Missing Payload")'
            if stream:
                lines += [
                    f"    await foreach (var response in context.WatchAsync(HostContracts.{name}, request, maxItemBytes, cancellationToken))",
                    "    {",
                    f"      yield return {value};",
                    "    }",
                ]
            else:
                lines += [
                    f"    var response = await context.CallAsync(HostContracts.{name}, request, policy, cancellationToken);",
                    f"    return {value};",
                ]
            lines += ["  }"]
        lines += ["}"]
    destination = root / "sdk/csharp/src/Harmos/Generated/Host.g.cs"
    source = "\n".join(lines) + "\n"
    if check:
        if not destination.is_file() or destination.read_text(encoding="utf-8") != source:
            raise ValueError("Check C# Host Resources: Generated Drift; Run mise run generate -- --sdk csharp")
    else:
        destination.write_text(source, encoding="utf-8", newline="\n")


def cpp_exports(root: Path, descriptors: FileDescriptorSet) -> None:
    methods = table(descriptors)
    requests = {method["request"] for method in methods}
    carriers = {
        "Request",
        "Response",
        "ChildRequest",
        "ChildResponse",
        "ChildCancel",
        "ChildWatch",
        "ChildWatchResponse",
        "ChildWatchCancel",
    }
    private = ["// @generated from protocol descriptors; do not edit.", "#pragma once"]
    public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "detail/messages_wire.hpp"']
    entries = []
    hidden = []
    for file in descriptors.file:
        if (
            file.name.endswith("guest.proto")
            or not file.package.startswith("harmos.")
            or (".plugin." in file.package and ".plugin.host." not in file.package)
        ):
            continue
        private += [f'#include "{file.name.removesuffix(".proto")}.pb.h"']
        boundary = "client" if ".client." in file.package else "plugin" if ".plugin.host." in file.package else ""
        for definition in [*file.message_type, *file.enum_type]:
            if f"{file.package}.{definition.name}" in requests or definition.name in carriers:
                alias = (boundary.title() or "Shared") + definition.name
                hidden.append(f"using {alias} = ::{file.package.replace(chr(46), chr(58) + chr(58))}::{definition.name};")
                continue
            name = "FailureValue" if definition.name == "Failure" else definition.name
            alias = (boundary.title() or "Shared") + name
            entries.append((boundary, name, alias, f"::{file.package.replace('.', '::')}::{definition.name}", False))
        for definition in file.enum_type:
            for value in definition.value:
                prefix = snake(definition.name).upper() + "_"
                name = value.name if value.name.startswith(prefix) else prefix + value.name
                entries.append(
                    (
                        boundary,
                        name,
                        (boundary.title() or "Shared") + name,
                        f"static_cast<::{file.package.replace('.', '::')}::{definition.name}>({value.number})",
                        True,
                    )
                )
    private += [
        "namespace harmos::detail {",
        'inline constexpr char stream_fingerprint_prefix[] = "harmos.protobuf.stream.v1\\0";',
        *hidden,
    ]
    private += [
        f"inline constexpr auto {alias} = {full};" if constant else f"using {alias} = {full};" for _, _, alias, full, constant in entries
    ]
    private += ["}"]
    for boundary in ("", "client", "plugin"):
        public += ["namespace harmos" + (f"::{boundary}" if boundary else "") + " {"]
        public += [
            f"inline constexpr auto {name} = ::harmos::detail::{alias};" if constant else f"using {name} = ::harmos::detail::{alias};"
            for face, name, alias, _, constant in entries
            if face == boundary
        ]
        public += ["}"]
    destination = root / "sdk/cpp/include/harmos"
    (destination / "detail").mkdir(parents=True, exist_ok=True)
    (destination / "detail/messages_wire.hpp").write_text("\n".join(private) + "\n", encoding="utf-8", newline="\n")
    (destination / "messages.hpp").write_text("\n".join(public) + "\n", encoding="utf-8", newline="\n")
    format_cpp([destination / "messages.hpp", destination / "detail/messages_wire.hpp"])


def cpp_host(root: Path, methods: list[Method]) -> None:
    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: Field) -> str:
        value = (
            cpp_message(field["type"])
            if field["type"]
            else {8: "bool", 9: "std::string", 12: "std::string", 4: "std::uint64_t", 13: "std::uint32_t"}.get(
                field["kind"], "std::int64_t"
            )
        )
        return f"std::vector<{value}>" if field["repeated"] else value

    nodes = sorted({method["path"][0] for method in methods})
    header = [
        "// @generated from protocol descriptors; do not edit.",
        "#pragma once",
        "namespace harmos::plugin {",
        "enum class HostOperation {",
    ]
    header += ["  " + "".join(title(part) for part in method["path"]) + "," for method in methods]
    header += ["};"]
    source = ["// @generated from protocol descriptors; do not edit.", '#include "harmos/harmos.hpp"']
    source += [f'#include "{file.removesuffix(".proto")}.pb.h"' for file in sorted({method["file"] for method in methods})]
    used = sorted({method[key] for method in methods for key in ("request", "response")})
    # Source Files Import Named Wire Types Once; Headers Mention Only Public Types.
    source += [f"using ::{full.replace('.', '::')};" for full in used]
    source += ["namespace harmos::plugin {"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        header += [f"struct Host{name}Options {{"]
        header += [f"  std::optional<{field_type(field)}> {field['name']};" for field in method["fields"] if field["optional"]]
        if not method["server_streaming"]:
            header += ["  std::optional<OperationPolicy> policy;"]
        header += ["};"]
        if method["server_streaming"]:
            payload = method["payload_field"]
            value = field_type(payload) if payload else cpp_message(method["response"])
            header += [
                f"class Host{name} {{",
                "public:",
                f"  explicit Host{name}(std::function<std::optional<{value}>()> next) : next_(std::move(next)) {{}}",
                f"  std::optional<{value}> next() {{ return next_(); }}",
                "private:",
                f"  std::function<std::optional<{value}>()> next_;",
                "};",
            ]
    for node in nodes:
        header += [f"struct Host{title(node)}Resource {{", "  Context context;"]
        for method in (method for method in methods if method["path"][0] == node):
            name = "".join(title(part) for part in method["path"])
            q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            payload = method["payload_field"]
            result = f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else cpp_message(method["response"])
            parameters = [f"const {field_type(field)}& {field['name']}" for field in method["fields"] if not field["optional"]]
            if method["server_streaming"]:
                parameters += ["std::size_t max_item_bytes"]
            header += [f"  {result} {method['path'][-1]}({', '.join([*parameters, 'Host' + name + 'Options options = {}'])});"]
            source += [
                f"{result} Host{title(node)}Resource::{method['path'][-1]}({', '.join([*parameters, 'Host' + name + 'Options options'])}) {{",
                f"  {q} request;",
            ]
            for field in method["fields"]:
                value = "*options." + field["name"] if field["optional"] else field["name"]
                if field["optional"]:
                    source += [f"  if (options.{field['name']}) {{"]
                assignment = (
                    f"*request.mutable_{field['name']}() = {value};" if field["kind"] == 11 else f"request.set_{field['name']}({value});"
                )
                source += ["    " + assignment]
                if field["optional"]:
                    source += ["  }"]
            source += [
                f'  auto operation = Operation<{q}, {r}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC);'
            ]
            if method["server_streaming"]:
                source += [
                    "  (void)options;",
                    "  operation.watch();",
                    f"  auto incoming = std::make_shared<Watching<{r}>>(context.watch(operation, request, max_item_bytes));",
                    f"  return Host{name}([incoming]() -> std::optional<{field_type(payload) if payload else cpp_message(method['response'])}> {{",
                    "    auto response = incoming->next();",
                    "    if (!response) { return {}; }",
                ]
                value = (
                    f"std::move(*response->mutable_{payload['name']}())"
                    if payload and payload["kind"] == 11
                    else f"response->{payload['name']}()"
                    if payload
                    else "*response"
                )
                if payload and payload["kind"] == 11:
                    source += [
                        f'    if (!response->has_{payload["name"]}()) {{ throw std::runtime_error("Read {name}: Missing Payload"); }}'
                    ]
                source += [f"    return {value};", "  });"]
            else:
                source += ["  auto response = context.call(operation, request, options.policy).get();"]
                if payload and payload["repeated"]:
                    value = f"{field_type(payload)}(response.{payload['name']}().begin(), response.{payload['name']}().end())"
                elif payload and payload["kind"] == 11:
                    source += [f'  if (!response.has_{payload["name"]}()) {{ throw std::runtime_error("Read {name}: Missing Payload"); }}']
                    value = f"std::move(*response.mutable_{payload['name']}())"
                else:
                    value = f"response.{payload['name']}()" if payload else "response"
                source += [f"  return {value};"]
            source += ["}"]
        header += ["};"]
    header += ["struct HostRoot {"]
    header += [f"  Host{title(node)}Resource {node};" for node in nodes]
    header += ["};", "}"]
    source += [
        "}",
        "namespace harmos::plugin {",
        "HostRoot Context::host() {",
        "  return {" + ", ".join("{*this}" for _ in nodes) + "};",
        "}",
        "Plugin& Plugin::requires_operation(HostOperation operation) {",
        "  switch (operation) {",
    ]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
        expression = f'Operation<{q}, {r}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC)' + (
            ".watch()" if method["server_streaming"] else ""
        )
        source += [f"    case HostOperation::{name}: return requires_operation({expression});"]
    source += ['    default: throw std::invalid_argument("Declare Host Requirement: Unknown Operation");', "  }", "}", "}"]
    boundary = source.index("namespace harmos::plugin {")
    body = "\n".join(source[boundary:])
    named = {cpp_message(method["response"]) for method in methods if method["payload_field"] is None}
    named.update(
        cpp_message(field["type"])
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    named.update(
        f"std::{name}"
        for name in (
            "string",
            "vector",
            "optional",
            "size_t",
            "uint64_t",
            "uint32_t",
            "int64_t",
            "int32_t",
            "runtime_error",
            "invalid_argument",
        )
    )
    imported = {method[key].rsplit(".", 1)[-1] for method in methods for key in ("request", "response")}
    declarations = []
    for qualified in sorted(named, key=len, reverse=True):
        if qualified not in body:
            continue
        name = qualified.rsplit("::", 1)[-1]
        body = re.sub(re.escape(qualified) + r"\b", name, body)
        if name not in imported:
            declarations.append(f"using {qualified};")
            imported.add(name)
    source = [*source[:boundary], *sorted(declarations), body]
    folder = root / "sdk/cpp/include/harmos/_generated"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "host.hpp").write_text("\n".join(header) + "\n", encoding="utf-8", newline="\n")
    folder = root / "sdk/cpp/src/generated"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "host.cpp").write_text("\n".join(source) + "\n", encoding="utf-8", newline="\n")
    format_cpp([root / "sdk/cpp/include/harmos/_generated/host.hpp", folder / "host.cpp"])


def cpp_message(full: str) -> str:
    parts = full.split(".")
    if full.startswith("google.protobuf."):
        return "::google::protobuf::" + parts[-1]
    if full.startswith("harmos.client."):
        return "::harmos::client::" + parts[-1]
    if full.startswith("harmos.plugin.host."):
        return "::harmos::plugin::" + parts[-1]
    if full.startswith("harmos.v1."):
        return "::harmos::" + parts[-1]
    return "::" + parts[0] + "::client::" + parts[-1]


def cpp(root: Path, methods: list[Method], *, namespace: str = "harmos", target: Path | None = None) -> None:
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    def title(part: str) -> str:
        return "".join(word.title() for word in part.split("_"))

    def field_type(field: Field) -> str:
        value = (
            cpp_message(field["type"])
            if field["type"]
            else {
                1: "double",
                2: "float",
                8: "bool",
                9: "std::string",
                12: "std::string",
                4: "std::uint64_t",
                13: "std::uint32_t",
                6: "std::uint64_t",
                7: "std::uint32_t",
                5: "std::int32_t",
                15: "std::int32_t",
                17: "std::int32_t",
            }.get(field["kind"], "std::int64_t")
        )
        return f"std::vector<{value}>" if field["repeated"] else value

    names = {node: "Client" if not node else "".join(title(part) for part in node) + "Resource" for node in nodes}
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#pragma once",
        '#include "harmos/client_transport.hpp"',
        '#include "harmos/messages.hpp"',
    ]
    # Wire Stubs and Request Types Stay in a Private Generated Header.
    private = ["// @generated from protocol descriptors; do not edit.", "#pragma once"]
    includes = sorted({method["file"].removesuffix(".proto") + ".grpc.pb.h" for method in methods})
    private += [f'#include "{include}"' for include in includes]
    private += [f"namespace {namespace.replace('.', '::')}::detail {{"]
    for method in methods:
        prefix = "".join(title(part) for part in method["path"])
        private += [
            f"using {prefix}Request = ::{method['request'].replace('.', '::')};",
            f"using {prefix}Response = ::{method['response'].replace('.', '::')};",
            f"using {prefix}Service = ::{method['package'].replace('.', '::')}::{method['service']};",
        ]
    private += ["}"]
    destination = target or root / "sdk/cpp/include/harmos/_generated/client.hpp"
    destination.parent.mkdir(parents=True, exist_ok=True)
    wire = destination.parent / "client_wire.hpp"
    wire.write_text("\n".join(private) + "\n", encoding="utf-8", newline="\n")
    lines += ['#include "client_wire.hpp"']
    if namespace != "harmos":
        public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "client_wire.hpp"']
        for method in methods:
            if method["owner"] == "harmos":
                continue
            prefix = "".join(title(part) for part in method["path"])
            public += [
                f"namespace {method['owner'].replace('.', '::')}::client {{ using {method['response'].rsplit('.', 1)[-1]} = ::{namespace.replace('.', '::')}::detail::{prefix}Response; }}"
            ]
        (destination.parent / "client_messages.hpp").write_text("\n".join(public) + "\n", encoding="utf-8", newline="\n")
        lines += ['#include "client_messages.hpp"']
    lines += [f"namespace {namespace.replace('.', '::')} {{"]
    lines += ["namespace client {"]
    for method in methods:
        options = "".join(title(part) for part in method["path"]) + "Options"
        lines += [f"struct {options} {{"]
        lines += [f"  std::optional<{field_type(field)}> {field['name']};" for field in method["fields"] if field["optional"]]
        lines += ["  std::shared_ptr<grpc::ClientContext> context;", "};"]
    lines += ["}"]
    for node in sorted(nodes, key=lambda node: (-len(node), node)):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        lines += [f"struct {names[node]} {{"]
        if operations:
            lines += ["  std::shared_ptr<grpc::Channel> channel;"]
        lines += [f"  {names[child]} {child[-1]};" for child in children]
        initializers = (["channel(channel)"] if operations else []) + [f"{child[-1]}(channel)" for child in children]
        lines += [f"  explicit {names[node]}(std::shared_ptr<grpc::Channel> channel) : {', '.join(initializers)} {{}}"]
        if not node:
            lines += [
                "  static Client connect(std::string address, const ::harmos::StorageTransferSettings& transfer = {}, std::optional<int> message_bytes = {}) {",
                "    const auto maximum = ::harmos::transfer_message_bytes(transfer, message_bytes);",
                "    grpc::ChannelArguments arguments;",
                "    arguments.SetMaxSendMessageSize(maximum);",
                "    arguments.SetMaxReceiveMessageSize(maximum);",
                '    if (address.starts_with("http://")) {',
                "      address.erase(0, 7);",
                "    }",
                "    return Client(grpc::CreateCustomChannel(address, grpc::InsecureChannelCredentials(), arguments));",
                "  }",
            ]
        for method in operations:
            if method["client_streaming"]:
                raise ValueError(f"Generate C++ Client {method['method']}: Managed Contract Required")
            prefix = "".join(title(part) for part in method["path"])
            request, response, service = (f"detail::{prefix}{suffix}" for suffix in ("Request", "Response", "Service"))
            parameters = [f"const {field_type(field)}& {field['name']}" for field in method["fields"] if not field["optional"]]
            parameters += [f"client::{prefix}Options options = {{}}"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else cpp_message(method["response"])
            stream = method["server_streaming"]
            returns = f"::harmos::ClientStream<{result}>" if stream else result
            name = method["path"][-1]
            if name == "delete":
                name = "delete_"
            lines += [f"  {returns} {name}({', '.join(parameters)}) {{", f"    {request} request;"]
            for field in method["fields"]:
                name = field["name"]
                optional = field["optional"]
                if name == "request_id":
                    lines += ["    request.set_request_id(options.request_id ? *options.request_id : ::harmos::detail::request_id());"]
                    continue
                if optional:
                    lines += [f"    if (options.{name}) {{"]
                value = f"*options.{name}" if optional else name
                indent = "      " if optional else "    "
                if field["repeated"]:
                    lines += [f"{indent}request.mutable_{name}()->Assign({value}.begin(), {value}.end());"]
                elif field["kind"] == 11:
                    lines += [f"{indent}*request.mutable_{name}() = {value};"]
                else:
                    lines += [f"{indent}request.set_{name}({value});"]
                if optional:
                    lines += ["    }"]
            lines += ["    auto context = options.context ? options.context : std::make_shared<grpc::ClientContext>();"]
            action = ".".join(method["path"])
            if stream:
                lines += [
                    f"    auto reader = std::shared_ptr<grpc::ClientReader<{response}>>({service}::NewStub(channel)->{method['method']}(context.get(), request));",
                    f"    return {{context, [reader]() -> std::optional<{result}> {{",
                    f"      {response} response;",
                    "      if (!reader->Read(&response)) {",
                    "        const auto status = reader->Finish();",
                    f'        if (!status.ok()) {{ throw std::runtime_error("Read {action}: " + status.error_message()); }}',
                    "        return {};",
                    "      }",
                ]
                indent = "      "
            else:
                lines += [
                    f"    {response} response;",
                    f"    const auto status = {service}::NewStub(channel)->{method['method']}(context.get(), request, &response);",
                    "    if (!status.ok()) {",
                    f'      throw std::runtime_error("Call {action}: " + status.error_message());',
                    "    }",
                ]
                indent = "    "
            value = "response"
            if payload:
                name = payload["name"]
                if payload["repeated"]:
                    value = f"{result}(response.{name}().begin(), response.{name}().end())"
                elif payload["kind"] == 11:
                    lines += [
                        f"{indent}if (!response.has_{name}()) {{",
                        f'{indent}  throw std::runtime_error("Read {action}: Missing Payload");',
                        f"{indent}}}",
                    ]
                    value = f"std::move(*response.mutable_{name}())"
                else:
                    value = f"response.{name}()"
            lines += [f"{indent}return {value};"]
            if stream:
                lines += ["    }};"]
            lines += ["  }"]
        lines += ["};"]
    lines += ["}"]
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    format_cpp([destination, wire, *([destination.parent / "client_messages.hpp"] if namespace != "harmos" else [])])


def format_cpp(paths: list[Path]) -> None:
    policy = Path(__file__).resolve().parents[5] / "sdk/cpp/.clang-format"
    subprocess.run(["clang-format", f"--style=file:{policy}", "-i", *map(str, paths)], check=True, creationflags=spawn_flags())


def compose(repository: Path, descriptors: list[Path], output: Path, namespace: str, *, language: str | None = None) -> None:
    if not re.fullmatch(r"[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*", namespace):
        raise ValueError("Generate Client: Invalid Output Namespace")
    merged = FileDescriptorSet()
    files = {}
    for path in [repository / "crates/harmos-protocol/src/descriptors.bin", *descriptors]:
        for file in FileDescriptorSet.FromString(path.read_bytes()).file:
            file.ClearField("source_code_info")
            if file.name in files and files[file.name] != file:
                raise ValueError(f"Generate Client: Conflicting Descriptor File {file.name}")
            files[file.name] = file
    merged.file.extend(files[name] for name in sorted(files))
    if language == "python":
        packages = {file.package.split(".", 1)[0] for file in merged.file if not file.package.startswith(("harmos.", "google."))}
        python(
            output,
            [method for method in table(merged) if method["face"] == "client"],
            merged,
            module_prefixes={package: f"{namespace}._wire.{package}" for package in packages},
        )
    else:
        generate(output, merged, namespace=namespace)


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--descriptors", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--namespace", default="harmos")
    parser.add_argument("--language", choices=("python",))
    parser.add_argument("--cpp-fixture", type=Path)
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[5]
    if options.cpp_fixture:
        fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
        library = root / "crates/harmos-protocol/src/descriptors.bin"
        methods = table(FileDescriptorSet.FromString(library.read_bytes())) + table(FileDescriptorSet.FromString(fixture.read_bytes()))
        cpp(root, [method for method in methods if method["face"] == "client"], namespace="fixture", target=options.cpp_fixture)
    elif options.descriptors:
        if options.output is None:
            parser.error("composed client descriptors require --output")
        compose(root, options.descriptors, options.output, options.namespace, language=options.language)
    else:
        generate(root)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
