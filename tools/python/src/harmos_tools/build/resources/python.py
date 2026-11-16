# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Python Resource Contracts."""

# === Imports === #

# Standard Library Imports
import re
import subprocess
import sys
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FileDescriptorSet

# Local Imports
import harmos_tools.build.resources.descriptor
from harmos_tools.process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def exports(target: Path, descriptors: FileDescriptorSet) -> None:
    """Generate Private Import Tables for the Three Public Barrel Points."""
    methods = harmos_tools.build.resources.descriptor.table(descriptors)
    audiences = harmos_tools.build.resources.descriptor.payload_audiences(descriptors)
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
    messages: set[tuple[str, str]] = set()
    for file in descriptors.file:
        if not file.package.startswith("harmos.") or file.name.endswith("guest.proto"):
            continue
        if ".plugin." in file.package:
            continue
        module = file.name.removesuffix(".proto").replace("/", ".").replace("harmos.", "harmos._wire.", 1) + "_pb2"
        messages.update((module, definition.name) for definition in file.message_type)
        for definition in [*file.message_type, *file.enum_type]:
            if f"{file.package}.{definition.name}" in requests or definition.name in carriers:
                continue
            name = "FailureValue" if definition.name == "Failure" else definition.name
            groups_for_type = (
                {"shared"}
                if ".model." in file.package or len(audiences.get(f"{file.package}.{definition.name}", set())) > 1
                else {group + "_types" for group in audiences.get(f"{file.package}.{definition.name}", {"client"})}
            )
            for group in groups_for_type:
                groups[group].append((module, definition.name, name))
        for definition in file.enum_type:
            groups_for_type = (
                {"shared"}
                if ".model." in file.package or len(audiences.get(f"{file.package}.{definition.name}", set())) > 1
                else {group + "_types" for group in audiences.get(f"{file.package}.{definition.name}", {"client"})}
            )
            for group in groups_for_type:
                groups[group].extend((module, value.name, value.name) for value in definition.value)
    target.mkdir(parents=True, exist_ok=True)
    for group, entries in groups.items():
        lines = ["# @generated from protocol descriptors; do not edit."]
        lines += [f"from {module} import {original} as {name}" for module, original, name in sorted(entries)]
        public_module = "harmos" if group == "shared" else "harmos." + group.removesuffix("_types")
        lines += [f'{name}.__module__ = "{public_module}"' for module, original, name in sorted(entries) if (module, original) in messages]
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
        "_contracts": ["Operation", "Stream", "Window", "observer", "on_stop", "operation", "publisher", "window_observer"],
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
    config = Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", *map(str, paths)]
    subprocess.run(
        command,
        check=True,
        creationflags=spawn_flags(),
    )
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "format", "--config", str(config), *map(str, paths)]
    subprocess.run(command, check=True, creationflags=spawn_flags())


def client(
    target: Path,
    methods: list[harmos_tools.build.resources.descriptor.Method],
    descriptors: FileDescriptorSet,
    *,
    module_prefixes: dict[str, str] | None = None,
) -> None:
    """Generate Content Arguments and Exactly-One-Field Response Projection."""
    selected: list[harmos_tools.build.resources.descriptor.Method] = []
    for method in methods:
        method = method.copy()
        if method["request"] == "harmos.host.v1.plugins.SettingsListRequest":
            method["fields"] = [
                harmos_tools.build.resources.descriptor.Field(**{**field, "optional": False}) if field["name"] == "name" else field
                for field in method["fields"]
            ]
        selected.append(method)
    methods = selected
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
    library = Path(__file__).resolve().parents[6] / "crates/harmos-protocol/src/descriptors.bin"
    library_methods = harmos_tools.build.resources.descriptor.table(FileDescriptorSet.FromString(library.read_bytes()))
    library_leaves = {tuple(method["path"]) for method in library_methods if method["face"] == "client"}
    narrowed = not library_leaves <= leaves
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

    def field_type(field: harmos_tools.build.resources.descriptor.Field) -> str:
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
    if narrowed:
        lines += ["from harmos._connection import Connection as LibraryConnection"]
    elif composed:
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
        parent = ""
        if narrowed:
            if not node:
                parent = "LibraryConnection"
        elif composed:
            if not node:
                parent = "LibraryClient"
            elif node in library_nodes:
                parent = "Library" + names[node]
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

    config = Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    paths = [target] + (
        [target.parent / "client_types.py", target.parent.parent / "client.py", target.parent.parent / "__init__.py"]
        if composed and target.parent.name == "_generated"
        else []
    )
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", *map(str, paths)]
    subprocess.run(
        command,
        check=True,
        creationflags=spawn_flags(),
    )
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "format", "--config", str(config), *map(str, paths)]
    subprocess.run(command, check=True, creationflags=spawn_flags())


def host(
    target: Path,
    methods: list[harmos_tools.build.resources.descriptor.Method],
    descriptors: FileDescriptorSet,
    *,
    module_prefixes: dict[str, str] | None = None,
    context_wrapper: bool = False,
) -> None:
    modules = {}
    for file in descriptors.file:
        module = file.name.removesuffix(".proto").replace("/", ".") + "_pb2"
        if module.startswith("harmos."):
            module = module.replace("harmos.", "harmos._wire.", 1)
        for package, prefix in (module_prefixes or {}).items():
            if module.startswith(package + "."):
                module = prefix + module.removeprefix(package)
        for definition in file.message_type:
            modules[f"{file.package}.{definition.name}"] = module
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )

    def field_type(field: harmos_tools.build.resources.descriptor.Field) -> str:
        value = (
            field["type"].rsplit(".", 1)[-1]
            if field["type"]
            else {1: "float", 2: "float", 8: "bool", 9: "str", 12: "bytes"}.get(field["kind"], "int")
        )
        return f"list[{value}]" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(part.title().replace("_", "") for part in node) + "Resource" for node in nodes}
    lines = [
        "# @generated from protocol descriptors; do not edit.",
        "from collections.abc import AsyncGenerator",
        "from enum import Enum",
        "from typing import TYPE_CHECKING, cast",
        "from google.protobuf.message import Message",
        "from harmos._contracts import Operation",
        "from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PUBLIC",
        "from harmos._wire.model.v1.policy_pb2 import OperationPolicy",
    ]
    lines += [f"from {modules[full]} import {full.rsplit('.', 1)[-1]}" for full in sorted(used)]
    lines += ["if TYPE_CHECKING:", "    from harmos._invocation import Context", "class HostOperation(Enum):"]
    if not methods:
        lines += ["    pass"]
    for method in methods:
        name = "_".join(method["path"]).upper()
        lines += [f'    {name} = "{method["package"]}.{method["service"]}/{method["method"]}"']
    for method in methods:
        name = "_".join(method["path"]).upper()
        q, r = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
        lines += [
            f"_{name} = Operation(HostOperation.{name}.value, {q}, {r}, visibility=VISIBILITY_PUBLIC, watch={method['server_streaming']})"
        ]
    lines += ["def host_requirement(operation: HostOperation) -> Operation[Message, Message]:"]
    if methods:
        lines += ["    match operation:"]
    for method in methods:
        name = "_".join(method["path"]).upper()
        lines += [f"        case HostOperation.{name}:", f"            return cast(Operation[Message, Message], _{name})"]
    lines += [
        '    raise ValueError("Declare Host Requirement: Unknown Operation")',
        "class HostRoot:",
        "    def __init__(self, context: 'Context') -> None:",
    ]
    lines += [f"        self.{node[-1]} = {names[node]}(context)" for node in nodes if len(node) == 1]
    if not nodes:
        lines += ["        pass"]
    for node in nodes:
        lines += [
            f"class {names[node]}:",
            "    def __init__(self, context: 'Context') -> None:",
            "        self._context = context",
        ]
        lines += [f"        self.{child[-1]} = {names[child]}(context)" for child in nodes if child[:-1] == node]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
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
    if context_wrapper:
        lines += ["class PluginContext:", '    def __init__(self, context: "Context") -> None:', "        self.host = HostRoot(context)"]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    config = Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", str(target)]
    subprocess.run(
        command,
        check=True,
        creationflags=spawn_flags(),
    )
    # Run the Native Command
    command = [sys.executable, "-m", "ruff", "format", "--config", str(config), str(target)]
    subprocess.run(command, check=True, creationflags=spawn_flags())


# === Private Functions === #

# === Entry Point === #
