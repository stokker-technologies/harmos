# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Python Resource Surfaces."""

# === Imports === #

# Standard Library Imports
import keyword
import subprocess
import sys
from pathlib import Path

# isort: split

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FileDescriptorSet

# isort: split

# Local Imports
from harmos_tools.process import spawn_flags

from . import descriptor
from .configuration import Configuration

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SCALARS = {
    1: "float",
    2: "float",
    3: "int",
    4: "int",
    5: "int",
    6: "int",
    7: "int",
    8: "bool",
    9: "str",
    12: "bytes",
    13: "int",
    15: "int",
    16: "int",
    17: "int",
    18: "int",
}


# === Types === #

# === Classes === #

# === Public Functions === #


def exports(target: Path, descriptors: FileDescriptorSet, *, configuration: Configuration | None = None) -> None:
    """Generate Private Import Tables for the Three Public Barrel Points."""
    configuration = configuration or Configuration()
    python_package = configuration.python_package if configuration.python_package is not None else configuration.namespace
    public_paths = descriptor.public_paths(descriptors, namespace=configuration.namespace)
    audiences = descriptor.payload_audiences(descriptors, library=configuration.library)
    requests = descriptor.request_names(descriptors)

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
        if not file.package.startswith((f"{configuration.namespace}.", "harmos.")):
            continue
        if file.name.endswith("guest.proto"):
            continue
        if file.package.startswith("harmos.plugin."):
            continue
        owner = file.package.partition(".")[0]
        package = python_package if owner == configuration.namespace else owner
        module = file.name.removesuffix(".proto").replace("/", ".").replace(f"{owner}.", f"{package}._wire.", 1) + "_pb2"
        for definition in [*file.message_type, *file.enum_type]:
            if f"{file.package}.{definition.name}" in requests:
                continue
            if definition.name in carriers:
                continue
            name = public_paths[f"{file.package}.{definition.name}"]
            groups_for_type = (
                {"plugin_types"}
                if ".plugin." in file.package
                else {"shared"}
                if ".model." in file.package or len(audiences.get(f"{file.package}.{definition.name}", set())) > 1
                else {group + "_types" for group in audiences.get(f"{file.package}.{definition.name}", {"client"})}
            )
            for group in groups_for_type:
                groups[group].append((module, definition.name, name))
        for definition in file.enum_type:
            groups_for_type = (
                {"plugin_types"}
                if ".plugin." in file.package
                else {"shared"}
                if ".model." in file.package or len(audiences.get(f"{file.package}.{definition.name}", set())) > 1
                else {group + "_types" for group in audiences.get(f"{file.package}.{definition.name}", {"client"})}
            )
            for group in groups_for_type:
                prefix = (
                    public_paths[f"{file.package}.{definition.name}"].rsplit(".", 1)[0]
                    if "." in public_paths[f"{file.package}.{definition.name}"]
                    else ""
                )
                groups[group].extend((module, value.name, (prefix + "." if prefix else "") + value.name) for value in definition.value)
    messages = set()
    for messages_file in descriptors.file:
        if not messages_file.package.startswith(f"{configuration.namespace}."):
            continue
        for messages_definition in messages_file.message_type:
            messages.add(
                (
                    messages_file.name.removesuffix(".proto")
                    .replace("/", ".")
                    .replace(f"{configuration.namespace}.", f"{python_package}._wire.", 1)
                    + "_pb2",
                    messages_definition.name,
                )
            )

    target.mkdir(parents=True, exist_ok=True)
    for path in (target / "_exports").rglob("*.py"):
        path.unlink()
    for group, entries in groups.items():
        modules: dict[tuple[str, ...], list[str]] = {(): []}
        exports: dict[tuple[str, ...], set[str]] = {(): set()}
        metadata: dict[tuple[str, ...], list[str]] = {}
        for module, original, name in sorted(entries):
            parts = name.split(".")
            node = tuple(parts[:-1])
            for length in range(1, len(node) + 1):
                parent, child = node[: length - 1], node[length - 1]
                modules.setdefault(parent, []).append(f"from . import {child}")
                exports.setdefault(parent, set()).add(child)
            modules.setdefault(node, []).append(f"from {module} import {original}")
            exports.setdefault(node, set()).add(original)
            if (module, original) in messages:
                public_module = python_package if group == "shared" else python_package + "." + group.removesuffix("_types")
                metadata.setdefault(node, []).append(f'{original}.__module__ = "{public_module}"')
                metadata[node].append(f'{original}.__qualname__ = "{name}"')
        for node, imports in modules.items():
            if node:
                path = target / "_exports" / group / Path(*node) / "__init__.py"
            else:
                path = target / f"{group}.py"
                imports = [line.replace("from . import ", f"from ._exports.{group} import ") for line in imports]
            path.parent.mkdir(parents=True, exist_ok=True)
            lines = [
                "# @generated from protocol descriptors; do not edit.",
                *dict.fromkeys(imports),
                "",
                *metadata.get(node, []),
                "",
                f"__all__ = {sorted(exports[node])!r}",
                "",
            ]
            path.write_text("\n".join(lines), encoding="utf-8", newline="\n")

    if configuration.namespace != "harmos":
        return
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

    shared = sorted({name.split(".", 1)[0] for _, _, name in groups["shared"]})
    # Export the Authored Exception Under Its Original Name.
    barrel(
        package / "__init__.py",
        [
            "from ._connection import Connection",
            "from ._invocation import FailureError",
            "from ._invocation import ProtocolError",
            *[f"from ._generated.shared import {name}" for name in shared],
        ],
        ["Connection", "FailureError", "ProtocolError", *shared],
    )
    client = sorted({name.split(".", 1)[0] for _, _, name in groups["client_types"]})
    barrel(
        package / "client.py",
        [f"from ._generated.client_types import {name}" for name in client],
        client,
    )
    plugin = sorted({name.split(".", 1)[0] for _, _, name in groups["plugin_types"]})
    authored = {
        "_authoring": ["Catalog", "Endpoint", "Plugin", "Registrar", "Registration"],
        "_contracts": ["Lane", "Operation", "Stream", "Window", "observer", "operation", "publisher", "window_observer"],
        "_invocation": ["Context", "Publisher"],
        "_generated.host": ["HostOperation"],
    }
    imports = [f"from ..{module} import {name}" for module, names in authored.items() for name in names]
    barrel(
        package / "plugin/__init__.py",
        [*imports, *[f"from .._generated.plugin_types import {name}" for name in plugin]],
        [*plugin, *[name for names in authored.values() for name in names]],
    )
    paths = [target / f"{group}.py" for group in groups] + [package / "__init__.py", package / "client.py", package / "plugin/__init__.py"]
    paths += sorted((target / "_exports").rglob("*.py"))
    config = configuration.python_style or Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    subprocess.run(
        [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", *map(str, paths)],
        check=True,
        creationflags=spawn_flags(),
    )
    subprocess.run(
        [sys.executable, "-m", "ruff", "format", "--config", str(config), *map(str, paths)], check=True, creationflags=spawn_flags()
    )


def client(
    target: Path,
    methods: list[descriptor.Method],
    descriptors: FileDescriptorSet,
    *,
    module_prefixes: dict[str, str] | None = None,
    configuration: Configuration | None = None,
) -> None:
    """Generate Content Arguments and Exactly-One-Field Response Projection."""
    configuration = configuration or Configuration()
    if not methods and target.parent.name == "_generated":
        target.unlink(missing_ok=True)
        return
    selected: list[descriptor.Method] = []
    for method in methods:
        method = method.copy()
        if method["request"] == "harmos.host.v1.plugins.SettingsListRequest":
            method["fields"] = [
                descriptor.Field(**{**field, "optional": False}) if field["name"] == "name" else field for field in method["fields"]
            ]
        selected.append(method)
    methods = selected
    nodes: set[tuple[str, ...]] = {()}
    leaves: set[tuple[str, ...]] = set()
    for method in methods:
        path = tuple(method["path"])
        if method["face"] != "client" or path in leaves:
            raise ValueError(
                f"Generate Python Client {'.'.join(path)}: Invalid or Duplicate Resource; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        leaves.add(path)
        if method["client_streaming"]:
            raise ValueError(
                f"Generate Python Client {method['method']}: Client Streaming Requires a Managed Contract; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    if leaves & nodes:
        raise ValueError(
            "Generate Python Client: Resource Collides With a Method; Correct the source protocol declarations or explicit public exports before regenerating"
        )
    root_name = "Client"
    names = {node: root_name if not node else "".join(part.title().replace("_", "") for part in node) + "Resource" for node in nodes}
    messages = _message_types(descriptors, configuration, module_prefixes)
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    imports, message_names = _message_imports(messages, used, reserved=set(names.values()))

    def field_type(field: descriptor.Field) -> str:
        value = message_names[field["type"]] if field["type"] else SCALARS.get(field["kind"], "int")
        return f"dict[str, {value}]" if field["map"] else f"Sequence[{value}]" if field["repeated"] else value

    lines = [
        "# @generated from protocol descriptors; do not edit.",
        '"""Typed Client Resources."""',
        "from collections.abc import AsyncGenerator, Sequence",
        "from uuid import uuid4",
        "from grpc import CallCredentials",
        "from grpc.aio import AioRpcError, Channel",
        "from harmos._client_transport import Metadata, call_error, stream, unary",
    ]
    lines += ["from harmos import _connection"]
    lines += imports
    for node in sorted(nodes, key=lambda value: (-len(value), value)):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        parent = "_connection.Connection" if not node else ""
        lines += ["", f"class {names[node]}" + (f"({parent})" if parent else "") + ":", "    def __init__(self, channel: Channel) -> None:"]
        if parent:
            lines += ["        super().__init__(channel)"]
        lines += ["        self._channel = channel"]
        storage = "_resource_"
        lines += [f"        self.{storage}{_identifier(child[-1])} = {names[child]}(channel)" for child in children]
        for child in children:
            lines += [
                "",
                "    @property",
                f"    def {_identifier(child[-1])}(self) -> {names[child]}:",
                f"        return self.{storage}{_identifier(child[-1])}",
            ]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            request, response = (message_names[method[key]] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            optional = [field for field in method["fields"] if field["optional"]]
            parameters = [f"{_identifier(field['name'])}: {field_type(field)}" for field in required]
            parameters += ["*"]
            parameters += [f"{_identifier(field['name'])}: {field_type(field)} | None = None" for field in optional]
            occupied = {_identifier(field["name"]) for field in method["fields"]} | {"self"}
            transport = {}
            for name in ("metadata", "timeout", "credentials"):
                parameter = "call_" + name if configuration.namespace != "harmos" and name == "metadata" else name
                while parameter in occupied:
                    parameter = "call_" + parameter
                occupied.add(parameter)
                transport[name] = parameter
            parameters += [
                f"{transport['metadata']}: Metadata | None = None",
                f"{transport['timeout']}: float | None = None",
                f"{transport['credentials']}: CallCredentials | None = None",
            ]
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            result = result.replace("Sequence[", "list[")
            stream_call = method["server_streaming"]
            returns = f"AsyncGenerator[{result}]" if stream_call else result
            lines += ["", f"    async def {_identifier(method['path'][-1])}(self, {', '.join(parameters)}) -> {returns}:"]
            values = [
                f"{field['name']}="
                + (
                    "request_id if request_id is not None else str(uuid4())"
                    if field["name"] == "request_id"
                    else _identifier(field["name"])
                )
                for field in method["fields"]
            ]
            for index, field in enumerate(method["fields"]):
                if keyword.iskeyword(field["name"]):
                    values[index] = f"**{{{field['name']!r}: {_identifier(field['name'])}}}"
            for group in sorted({field["oneof"] for field in method["fields"] if field["oneof"]}):
                variants = ", ".join(_identifier(field["name"]) for field in method["fields"] if field["oneof"] == group)
                lines += [
                    f"        if sum(value is not None for value in ({variants},)) > 1:",
                    f'            raise ValueError("Choose At Most One {group} Variant")',
                ]
            lines += [f"        request = {request}({', '.join(values)})"]
            path = f"/{method['package']}.{method['service']}/{method['method']}"
            call = f"{'stream' if stream_call else 'unary'}(self._channel, {path!r}, request, {response}, metadata={transport['metadata']}, timeout={transport['timeout']}, credentials={transport['credentials']})"
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
    source = "\n".join(lines) + "\n"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(source, encoding="utf-8", newline="\n")
    if target.parent.name == "_generated" and configuration.namespace == "harmos":
        package = target.parent.parent
        requests = {method["request"] for method in methods}
        exports = []
        for full, (module, name) in sorted(messages.items()):
            if full.startswith(("harmos.", "google.")) or full in requests or "." in name:
                continue
            exports.append((module, name))
        private_types = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from {module} import {name}" for module, name in exports],
            "",
            f"__all__ = {sorted(name for _, name in exports)!r}",
        ]
        (target.parent / "client_types.py").write_text("\n".join(private_types) + "\n", encoding="utf-8", newline="\n")
        client_exports = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from ._generated.client_types import {name}" for _, name in exports],
            "",
            f"__all__ = {sorted(name for _, name in exports)!r}",
        ]
        (package / "client.py").write_text("\n".join(client_exports) + "\n", encoding="utf-8", newline="\n")
        # A Composed Package Exposes Its Generated Root Through the Same Client Entry Point.
        (package / "__init__.py").write_text(
            "# @generated from protocol descriptors; do not edit.\nfrom ._generated.client import Client\n\n__all__ = ['Client']\n",
            encoding="utf-8",
            newline="\n",
        )

    config = configuration.python_style or Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    paths = [target] + (
        [target.parent / "client_types.py", target.parent.parent / "client.py", target.parent.parent / "__init__.py"]
        if target.parent.name == "_generated" and configuration.namespace == "harmos"
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


def host(
    target: Path,
    methods: list[descriptor.Method],
    descriptors: FileDescriptorSet,
    *,
    module_prefixes: dict[str, str] | None = None,
    context_wrapper: bool = False,
    configuration: Configuration | None = None,
) -> None:
    configuration = configuration or Configuration()
    messages = _message_types(descriptors, configuration, module_prefixes)
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )

    reserved = {
        "Host" + "".join(part.title().replace("_", "") for part in method["path"][:length]) + "Resource"
        for method in methods
        for length in range(1, len(method["path"]))
    }
    imports, message_names = _message_imports(messages, used, reserved=reserved)

    def field_type(field: descriptor.Field) -> str:
        value = message_names[field["type"]] if field["type"] else SCALARS.get(field["kind"], "int")
        return f"dict[str, {value}]" if field["map"] else f"list[{value}]" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(part.title().replace("_", "") for part in node) + "Resource" for node in nodes}
    lines = [
        "# @generated from protocol descriptors; do not edit.",
        "from collections.abc import AsyncGenerator",
        "from enum import Enum",
        "from uuid import uuid4",
        "from typing import TYPE_CHECKING, cast",
        "from google.protobuf.message import Message",
        "from harmos._contracts import Operation",
        "from harmos._wire.model.v1.operation_pb2 import VISIBILITY_PUBLIC",
        "from harmos._wire.model.v1.policy_pb2 import OperationPolicy",
    ]
    lines += imports
    lines += ["if TYPE_CHECKING:", "    from harmos._invocation import Context", "class HostOperation(Enum):"]
    if not methods:
        lines += ["    pass"]
    for method in methods:
        name = "_".join(method["path"]).upper()
        lines += [f'    {name} = "{method["package"]}.{method["service"]}/{method["method"]}"']
    for method in methods:
        name = "_".join(method["path"]).upper()
        request_type, response_type = (message_names[method[key]] for key in ("request", "response"))
        lines += [
            f"_{name} = Operation(HostOperation.{name}.value, {request_type}, {response_type}, visibility=VISIBILITY_PUBLIC, watch={method['server_streaming']})"
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
    lines += [f"        self.{_identifier(node[-1])} = {names[node]}(context)" for node in nodes if len(node) == 1]
    if not nodes:
        lines += ["        pass"]
    for node in nodes:
        lines += [
            f"class {names[node]}:",
            "    def __init__(self, context: 'Context') -> None:",
            "        self._context = context",
        ]
        lines += [f"        self.{_identifier(child[-1])} = {names[child]}(context)" for child in nodes if child[:-1] == node]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            name = "_".join(method["path"]).upper()
            request_type, response_type = (message_names[method[key]] for key in ("request", "response"))
            parameters = [f"{_identifier(field['name'])}: {field_type(field)}" for field in method["fields"] if not field["optional"]]
            parameters += ["*"]
            parameters += [
                f"{_identifier(field['name'])}: {field_type(field)} | None = None" for field in method["fields"] if field["optional"]
            ]
            if method["server_streaming"]:
                parameters += ["max_item_bytes: int"]
            else:
                parameters += ["policy: OperationPolicy | None = None"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else response_type
            if method["server_streaming"]:
                result = f"AsyncGenerator[{result}]"
            values = []
            for field in method["fields"]:
                value = (
                    "request_id if request_id is not None else str(uuid4())"
                    if field["name"] == "request_id"
                    else _identifier(field["name"])
                )
                values.append(f"**{{{field['name']!r}: {value}}}" if keyword.iskeyword(field["name"]) else f"{field['name']}={value}")
            lines += [
                f"    async def {_identifier(method['path'][-1])}(self{', ' if parameters else ''}{', '.join(parameters)}) -> {result}:",
                f"        request = {request_type}({', '.join(values)})",
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
    config = configuration.python_style or Path(__file__).resolve().parents[6] / "sdk/python/pyproject.toml"
    subprocess.run(
        [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", str(target)],
        check=True,
        creationflags=spawn_flags(),
    )
    subprocess.run([sys.executable, "-m", "ruff", "format", "--config", str(config), str(target)], check=True, creationflags=spawn_flags())


# === Private Functions === #


def _message_types(
    descriptors: FileDescriptorSet, configuration: Configuration, module_prefixes: dict[str, str] | None
) -> dict[str, tuple[str, str]]:
    python_package = configuration.python_package if configuration.python_package is not None else configuration.namespace
    types: dict[str, tuple[str, str]] = {}

    def record(definitions: list[DescriptorProto], package: str, module: str, parent: str = "") -> None:
        for definition in definitions:
            name = parent + definition.name
            types[f"{package}.{name}"] = (module, name)
            for enum in definition.enum_type:
                types[f"{package}.{name}.{enum.name}"] = (module, f"{name}.{enum.name}")
            record(list(definition.nested_type), package, module, name + ".")

    for file in descriptors.file:
        module = file.name.removesuffix(".proto").replace("/", ".") + "_pb2"
        for owner in {"harmos", configuration.namespace}:
            if module.startswith(owner + "."):
                package = python_package if owner == configuration.namespace else owner
                module = module.replace(owner + ".", package + "._wire.", 1)
                break
        for prefix, replacement in (module_prefixes or {}).items():
            if module.startswith(prefix + "."):
                module = replacement + module.removeprefix(prefix)
        record(list(file.message_type), file.package, module)
        for enum in file.enum_type:
            types[f"{file.package}.{enum.name}"] = (module, enum.name)
    return types


def _message_imports(
    types: dict[str, tuple[str, str]], used: set[str], *, reserved: set[str] | None = None
) -> tuple[list[str], dict[str, str]]:
    names = {full: types[full][1] for full in used}
    modules: dict[str, set[str]] = {}
    for full in used:
        module, name = types[full]
        modules.setdefault(name.split(".", 1)[0], set()).add(module)
    duplicates = {name for name, values in modules.items() if len(values) > 1} | (reserved or set())
    duplicates.update(
        {
            "Client",
            "Call",
            "Metadata",
            "Channel",
            "CallCredentials",
            "ClientRoot",
            "Sequence",
            "Operation",
            "Message",
            "OperationPolicy",
            "HostOperation",
            "Context",
            "Enum",
            "AsyncGenerator",
        }
    )
    imports = set()
    for full in used:
        module, name = types[full]
        if name.split(".", 1)[0] in duplicates:
            names[full] = f"{module}.{name}"
            imports.add(f"import {module}")
        else:
            imports.add(f"from {module} import {name.split('.', 1)[0]}")
    return sorted(imports), names


def _identifier(name: str) -> str:
    return name + "_" if keyword.iskeyword(name) else name


# === Entry Point === #
