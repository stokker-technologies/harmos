# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Cpp Resource Surfaces."""

# === Imports === #

# Standard Library Imports
import re
import subprocess
from dataclasses import dataclass
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
    1: "double",
    2: "float",
    3: "std::int64_t",
    4: "std::uint64_t",
    5: "std::int32_t",
    6: "std::uint64_t",
    7: "std::uint32_t",
    8: "bool",
    9: "std::string",
    12: "std::string",
    13: "std::uint32_t",
    15: "std::int32_t",
    16: "std::int64_t",
    17: "std::int32_t",
    18: "std::int64_t",
}


# === Types === #

# === Classes === #


@dataclass(frozen=True)
class PayloadExport:
    boundary: str
    name: str
    qualified: str
    constant: bool = False


# === Public Functions === #


def exports(root: Path, descriptors: FileDescriptorSet, *, configuration: Configuration | None = None) -> dict[str, str]:
    configuration = configuration or Configuration()
    public_paths = descriptor.public_paths(descriptors, namespace=configuration.namespace)
    type_names: dict[str, str] = {}
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
    private = ["// @generated from protocol descriptors; do not edit.", "#pragma once"]
    public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "detail/messages.hpp"']
    entries: list[PayloadExport] = []
    for file in descriptors.file:
        if file.name.endswith("guest.proto") or not file.package.startswith(configuration.namespace + ".") or ".plugin." in file.package:
            continue
        private += [f'#include "{file.name.removesuffix(".proto")}.pb.h"']
        for definition in [*file.message_type, *file.enum_type]:
            groups_for_type = audiences.get(f"{file.package}.{definition.name}", {"client"})
            boundary = "" if ".model." in file.package or len(groups_for_type) > 1 else next(iter(groups_for_type))
            if f"{file.package}.{definition.name}" in requests or definition.name in carriers:
                continue
            name = public_paths[f"{file.package}.{definition.name}"].replace(".", "::")
            type_names[f"{file.package}.{definition.name}"] = (
                f"::{configuration.namespace}::" + (boundary + "::" if boundary else "") + name
            )
            entries.append(PayloadExport(boundary, name, f"::{file.package.replace('.', '::')}::{definition.name}"))
            if isinstance(definition, DescriptorProto):
                pending = [(f"{file.package}.{definition.name}", type_names[f"{file.package}.{definition.name}"], definition)]
                while pending:
                    full, parent, message = pending.pop()
                    for nested in [*message.nested_type, *message.enum_type]:
                        type_names[f"{full}.{nested.name}"] = f"{parent}::{nested.name}"
                        if isinstance(nested, DescriptorProto):
                            pending.append((f"{full}.{nested.name}", f"{parent}::{nested.name}", nested))

        for definition in file.enum_type:
            groups_for_type = audiences.get(f"{file.package}.{definition.name}", {"client"})
            boundary = "" if ".model." in file.package or len(groups_for_type) > 1 else next(iter(groups_for_type))
            scope = public_paths[f"{file.package}.{definition.name}"].rpartition(".")[0].replace(".", "::")
            for value in definition.value:
                prefix = descriptor.snake(definition.name).upper() + "_"
                name = value.name if value.name.startswith(prefix) else prefix + value.name
                entries.append(
                    PayloadExport(
                        boundary,
                        (scope + "::" if scope else "") + name,
                        f"static_cast<::{file.package.replace('.', '::')}::{definition.name}>({value.number})",
                        True,
                    )
                )
    private += [
        f"namespace {configuration.namespace}::detail {{",
        'inline constexpr char stream_fingerprint_prefix[] = "harmos.protobuf.stream.v1\\0";',
        "namespace payloads {",
    ]
    for boundary in ("", "client", "plugin"):
        if boundary:
            private.append(f"namespace {boundary} {{")
        for entry in entries:
            if entry.boundary != boundary:
                continue
            prefix, _, name = entry.name.rpartition("::")
            declaration = f"inline constexpr auto {name} = {entry.qualified};" if entry.constant else f"using {entry.qualified};"
            if prefix:
                declaration = f"namespace {prefix} {{ {declaration} }}"
            private.append(declaration)
        if boundary:
            private.append("}")
    private += ["}", "}"]
    for boundary in ("", "client", "plugin"):
        public += [f"namespace {configuration.namespace}" + (f"::{boundary}" if boundary else "") + " {"]
        for entry in entries:
            if entry.boundary != boundary:
                continue
            qualified = (boundary + "::" if boundary else "") + entry.name
            prefix, _, name = entry.name.rpartition("::")
            declaration = f"using ::{configuration.namespace}::detail::payloads::{qualified};"
            if prefix:
                declaration = f"namespace {prefix} {{ {declaration} }}"
            public.append(declaration)
        public += ["}"]
    destination = root / f"sdk/cpp/include/{configuration.namespace}"
    (destination / "detail").mkdir(parents=True, exist_ok=True)
    (destination / "detail/messages.hpp").write_text("\n".join(private) + "\n", encoding="utf-8", newline="\n")
    (destination / "messages.hpp").write_text("\n".join(public) + "\n", encoding="utf-8", newline="\n")
    format_code([destination / "messages.hpp", destination / "detail/messages.hpp"])
    return type_names


def host(
    root: Path,
    methods: list[descriptor.Method],
    type_names: dict[str, str] | None = None,
    *,
    target: Path | None = None,
    namespace: str = "harmos",
    configuration: Configuration | None = None,
) -> None:
    configuration = configuration or Configuration()
    type_names = dict(type_names or {})

    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: descriptor.Field) -> str:
        value = message(field["type"], type_names) if field["type"] else SCALARS.get(field["kind"], "std::int64_t")
        return f"std::map<std::string, {value}>" if field["map"] else f"std::vector<{value}>" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods if method["surface"] for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
    header = [
        "// @generated from protocol descriptors; do not edit.",
        "#pragma once",
        f"namespace {configuration.namespace}::plugin {{",
        "enum class HostOperation {",
    ]
    header += ["  " + "".join(title(part) for part in method["path"]) + "," for method in methods]
    header += ["};"]
    source = [
        "// @generated from protocol descriptors; do not edit.",
        f'#include "{configuration.namespace}/{configuration.namespace}.hpp"',
    ]
    source_items = []
    for source_file in sorted({method["file"] for method in methods}):
        source_items.append(f'''#include "{source_file.removesuffix(".proto")}.pb.h"''')
    source += source_items
    used = sorted({method[key] for method in methods for key in ("request", "response")})
    # Source Files Import Unambiguous Wire Types; Clashes Keep Their Qualified Path.
    imports: dict[str, set[str]] = {}
    reserved = {"Context", "Plugin", "Operation", "Watching", "HostRoot", "HostOperation", *names.values()}
    for full in used:
        _, container = _type_import("::" + full.replace(".", "::"))
        imports.setdefault(container.rsplit("::", 1)[-1], set()).add(container)
    clashes = reserved | {name for name, containers in imports.items() if len(containers) > 1}
    wire_names = {}
    for full in used:
        qualified = "::" + full.replace(".", "::")
        name, container = _type_import(qualified)
        wire_names[full] = qualified if container.rsplit("::", 1)[-1] in clashes else name
    source += sorted({f"using {container};" for name, containers in imports.items() if name not in clashes for container in containers})
    source += [f"namespace {configuration.namespace}::plugin {{"]
    if configuration.namespace != "harmos":
        source += ["using ::harmos::plugin::Operation;", "using ::harmos::plugin::Watching;", "using ::harmos::VISIBILITY_PUBLIC;"]
    for method in methods:
        if not method["surface"]:
            continue
        name = "".join(title(part) for part in method["path"])
        header += [f"struct Host{name}Options {{"]
        header_items = []
        for header_field in method["fields"]:
            if not (header_field["optional"]):
                continue
            header_items.append(f"  std::optional<{field_type(header_field)}> {header_field['name']};")
        header += header_items
        if not method["server_streaming"]:
            header += [
                "  std::optional<"
                + ("OperationPolicy" if configuration.namespace == "harmos" else "::harmos::OperationPolicy")
                + "> policy;"
            ]
        header += ["};"]
        if method["server_streaming"]:
            payload = method["payload_field"]
            value = field_type(payload) if payload else message(method["response"], type_names)
            header += [
                f"class Host{name} {{",
                "public:",
                f"  explicit Host{name}(std::function<std::optional<{value}>()> next_value) : next_(std::move(next_value)) {{}}",
                f"  std::optional<{value}> next() {{ return next_(); }}",
                "private:",
                f"  std::function<std::optional<{value}>()> next_;",
                "};",
            ]
    for node in reversed(nodes):
        children = []
        for children_child in nodes:
            if children_child[:-1] != node:
                continue
            children.append(children_child)

        header += [f"struct {names[node]} {{", "  Context context;"]
        header += [f"  {names[child]} {_identifier(child[-1])};" for child in children]
        members = {"context", *(_identifier(child[-1]) for child in children)}
        members.update(_identifier(method["path"][-1]) for method in methods if tuple(method["path"][:-1]) == node)
        parameter = _parameter("context", members)
        initializers = [f"context({parameter})", *[f"{_identifier(child[-1])}({parameter})" for child in children]]
        header += [f"  {names[node]}(Context {parameter}) : {', '.join(initializers)} {{}}"]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            name = "".join(title(part) for part in method["path"])
            q, r = (wire_names[method[key]] for key in ("request", "response"))
            payload = method["payload_field"]
            result = (
                f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else message(method["response"], type_names)
            )
            parameters = []
            arguments = {}
            options_parameter = _parameter("options", members)
            maximum_parameter = _parameter("max_item_bytes", members)
            occupied = (
                members
                | {options_parameter, maximum_parameter}
                | {
                    "options",
                    "max_item_bytes",
                    "request",
                    "response",
                    "context",
                    "operation",
                    "incoming",
                    "reader",
                    "status",
                    "key",
                    "item",
                }
            )
            for field in method["fields"]:
                if field["optional"]:
                    continue
                arguments[field["name"]] = _parameter(field["name"], occupied)
                occupied.add(arguments[field["name"]])
            for parameters_field in method["fields"]:
                if parameters_field["optional"]:
                    continue
                parameters.append(f"const {field_type(parameters_field)}& {arguments[parameters_field['name']]}")

            if method["server_streaming"]:
                parameters += [f"std::size_t {maximum_parameter}"]
            header += [
                f"  {result} {method['path'][-1] + ('_' if method['path'][-1] in {'delete', 'export', 'import', 'new', 'class', 'default'} else '')}({', '.join([*parameters, 'Host' + name + 'Options ' + options_parameter + ' = {}'])});"
            ]
            source += [
                f"{result} {names[node]}::{method['path'][-1] + ('_' if method['path'][-1] in {'delete', 'export', 'import', 'new', 'class', 'default'} else '')}({', '.join([*parameters, 'Host' + name + 'Options ' + options_parameter])}) {{",
                f"  {q} request;",
            ]
            for field in method["fields"]:
                if field["name"] == "request_id":
                    source += [
                        f"  request.set_request_id({options_parameter}.request_id ? *{options_parameter}.request_id : ::harmos::detail::request_id());"
                    ]
                    continue
                value = f"*{options_parameter}." + _identifier(field["name"]) if field["optional"] else arguments[field["name"]]
                if field["optional"]:
                    source += [f"  if ({options_parameter}.{_identifier(field['name'])}) {{"]
                assignment = (
                    f"for (const auto& [key, item] : {value}) {{ (*request.mutable_{_identifier(field['name'])}())[key] = item; }}"
                    if field["map"]
                    else f"request.mutable_{_identifier(field['name'])}()->Assign({value}.begin(), {value}.end());"
                    if field["repeated"]
                    else f"*request.mutable_{_identifier(field['name'])}() = {value};"
                    if field["kind"] == 11
                    else f"request.set_{_identifier(field['name'])}({value});"
                )
                source += ["    " + assignment]
                if field["optional"]:
                    source += ["  }"]
            source += [
                f'  auto operation = Operation<{q}, {r}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC);'
            ]
            if method["server_streaming"]:
                source += [
                    f"  (void){options_parameter};",
                    "  operation.watch();",
                    f"  auto incoming = std::make_shared<Watching<{r}>>(context.watch(operation, request, {maximum_parameter}));",
                    f"  return Host{name}([incoming]() -> std::optional<{field_type(payload) if payload else message(method['response'], type_names)}> {{",
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
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    source += [
                        f'    if (!response->has_{payload["name"]}()) {{ throw std::runtime_error("Read {name}: Missing Payload"); }}'
                    ]
                source += [f"    return {value};", "  });"]
            else:
                source += [f"  auto response = context.call(operation, request, {options_parameter}.policy).get();"]
                if payload and payload["repeated"]:
                    value = f"{field_type(payload)}(response.{payload['name']}().begin(), response.{payload['name']}().end())"
                elif payload and payload["kind"] == 11 and not payload["repeated"]:
                    source += [f'  if (!response.has_{payload["name"]}()) {{ throw std::runtime_error("Read {name}: Missing Payload"); }}']
                    value = f"std::move(*response.mutable_{payload['name']}())"
                else:
                    value = f"response.{payload['name']}()" if payload else "response"
                source += [f"  return {value};"]
            source += ["}"]
        header += ["};"]
    header += ["struct HostRoot {"]
    header_items = []
    for header_node in nodes:
        if len(header_node) != 1:
            continue
        header_items.append(f"  {names[header_node]} {header_node[-1]};")
    header += header_items
    header += ["};", "}"]
    source += [
        "}",
        f"namespace {configuration.namespace}::plugin {{",
        "HostRoot Context::host() {",
        "  return {" + ", ".join("{*this}" for node in nodes if len(node) == 1) + "};",
        "}",
        "Registrar& Registrar::add(HostOperation operation) {",
        "  switch (operation) {",
    ]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        q, r = (wire_names[method[key]] for key in ("request", "response"))
        expression = f'Operation<{q}, {r}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC)' + (
            ".watch()" if method["server_streaming"] else ""
        )
        source += [f"    case HostOperation::{name}: return add(Dependency{{{expression}}});"]
    source += ['    default: throw std::invalid_argument("Declare Host Requirement: Unknown Operation");', "  }", "}", "}"]
    boundary = source.index(f"namespace {configuration.namespace}::plugin {{")
    body = "\n".join(source[boundary:])
    named = set()
    for named_method in methods:
        if named_method["payload_field"] is not None:
            continue
        named.add(message(named_method["response"], type_names))

    named.update(
        message(field["type"], type_names)
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
    for qualified in named:
        _, container = _type_import(qualified)
        imports.setdefault(container.rsplit("::", 1)[-1], set()).add(container)
    clashes = reserved | {name for name, containers in imports.items() if len(containers) > 1}
    declarations: set[str] = set()
    for qualified in sorted(named, key=len, reverse=True):
        if qualified not in body:
            continue
        name, imported_type = _type_import(qualified)
        if imported_type.rsplit("::", 1)[-1] in clashes:
            continue
        body = re.sub(re.escape(qualified) + r"\b", name, body)
        declaration = f"using {imported_type};"
        if declaration not in source:
            declarations.add(declaration)
    source = [*source[:boundary], *sorted(declarations), body]
    if target is not None:
        public = "\n".join(header).replace("namespace harmos::plugin {", f"namespace {namespace.replace('.', '::')}::plugin {{")
        public = re.sub(r"(?<![:\w])Context\b", "::harmos::plugin::Context", public)
        public = public.replace("  ::harmos::plugin::Context context;", "private:\n  ::harmos::plugin::Context context;\npublic:")
        public = public.rsplit("}", 1)[0]
        public += "struct PluginContext {\n  HostRoot host;\n  explicit PluginContext(::harmos::plugin::Context context) : host{"
        public += ", ".join("{context}" for node in nodes if len(node) == 1) + "} {}\n};\n}\n"
        public = '#include "harmos/harmos.hpp"\n' + public
        # SDK Payloads Keep Their One Public Barrel Path.
        public = public.replace("std::optional<OperationPolicy>", "std::optional<::harmos::OperationPolicy>")
        implementation = "\n".join(source)
        implementation = implementation[: implementation.index("HostRoot Context::host()")]
        implementation += "}\n"
        implementation = implementation.replace(
            "namespace harmos::plugin {",
            f"namespace {namespace.replace('.', '::')}::plugin {{\nusing namespace ::harmos::plugin;\nusing namespace ::harmos;",
        )
        implementation = f'#include "{target.name}"\n' + implementation
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(public, encoding="utf-8", newline="\n")
        target.with_suffix(".cpp").write_text(implementation, encoding="utf-8", newline="\n")
        format_code([target, target.with_suffix(".cpp")])
        return
    folder = root / f"sdk/cpp/include/{configuration.namespace}/_generated"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "host.hpp").write_text("\n".join(header) + "\n", encoding="utf-8", newline="\n")
    folder = root / "sdk/cpp/src/generated"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "host.cpp").write_text("\n".join(source) + "\n", encoding="utf-8", newline="\n")
    format_code([root / f"sdk/cpp/include/{configuration.namespace}/_generated/host.hpp", folder / "host.cpp"])


def message(full: str, type_names: dict[str, str]) -> str:
    parts = full.split(".")
    if full.startswith("google.protobuf."):
        return "::google::protobuf::" + parts[-1]
    if ".host." in full:
        # Shared Host Replies Belong to the Root; Audience-Exclusive Replies to Their Barrel.
        return type_names.get(full, "::" + full.replace(".", "::"))
    if ".model." in full:
        return type_names.get(full, "::" + parts[0] + "::" + parts[-1])
    return type_names.get(full, "::" + parts[0] + "::plugin::" + parts[-1])


def client(
    root: Path,
    methods: list[descriptor.Method],
    *,
    type_names: dict[str, str] | None = None,
    namespace: str = "harmos",
    target: Path | None = None,
    configuration: Configuration | None = None,
) -> None:
    if target is None and not methods:
        (root / "sdk/cpp/include/harmos/_generated/client.hpp").unlink(missing_ok=True)
        (root / "sdk/cpp/include/harmos/_generated/detail/client.hpp").unlink(missing_ok=True)
        return
    configuration = configuration or Configuration()
    type_names = dict(type_names or {})
    library = configuration.library_descriptors(root)
    if library.file:
        public_paths = descriptor.public_paths(library)
        for full, groups in descriptor.payload_audiences(library, library=configuration.library).items():
            if full not in public_paths:
                continue
            boundary = "" if ".model." in full or len(groups) > 1 else next(iter(groups)) + "::"
            type_names[full] = "::harmos::" + boundary + public_paths[full].replace(".", "::")
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    def title(part: str) -> str:
        return "".join(word.title() for word in part.split("_"))

    def field_type(field: descriptor.Field) -> str:
        value = message(field["type"], type_names) if field["type"] else SCALARS.get(field["kind"], "std::int64_t")
        return f"std::map<std::string, {value}>" if field["map"] else f"std::vector<{value}>" if field["repeated"] else value

    names = {}
    for names_node in nodes:
        names[names_node] = "Client" if not names_node else "".join(title(part) for part in names_node) + "Resource"

    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#pragma once",
        '#include "harmos/client_transport.hpp"',
        '#include "harmos/messages.hpp"',
    ]
    if configuration.namespace != "harmos":
        lines += [f'#include "{configuration.namespace}/messages.hpp"']
    # Wire Stubs and Request Types Stay in a Private Generated Header.
    private = ["// @generated from protocol descriptors; do not edit.", "#pragma once"]
    includes = sorted({method["file"].removesuffix(".proto") + ".grpc.pb.h" for method in methods})
    private += [f'#include "{include}"' for include in includes]
    private += [f"namespace {namespace.replace('.', '::')}::detail::packages {{"]
    for method in methods:
        types = [method["request"], method["response"], method["package"] + "." + method["service"]]
        for full in types:
            parts = full.split(".")
            boundary = next(index for index, part in enumerate(parts) if part[0].isupper())
            package = _private_name(".".join(parts[:boundary]))
            definition = "::".join(parts[: boundary + 1])
            private.append(f"namespace {package} {{ using ::{definition}; }}")
    private += ["}"]
    destination = target or root / "sdk/cpp/include/harmos/_generated/client.hpp"
    destination.parent.mkdir(parents=True, exist_ok=True)
    wire = destination.parent / "detail/client.hpp"
    wire.parent.mkdir(parents=True, exist_ok=True)
    wire.write_text("\n".join(private) + "\n", encoding="utf-8", newline="\n")
    lines += ['#include "detail/client.hpp"']
    if namespace != configuration.namespace:
        public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "detail/client.hpp"']
        for method in methods:
            if method["owner"] == "harmos":
                continue
            _, container = _type_import("::" + method["response"].replace(".", "::"))
            imported_type = _private_name(container.removeprefix("::").replace("::", "."))
            public += [
                f"namespace {method['owner'].replace('.', '::')}::client {{ using ::{namespace.replace('.', '::')}::detail::packages::{imported_type}; }}"
            ]
        (destination.parent / "client_messages.hpp").write_text("\n".join(public) + "\n", encoding="utf-8", newline="\n")
        lines += ['#include "client_messages.hpp"']
    lines += [f"namespace {namespace.replace('.', '::')} {{"]
    lines += ["namespace client {"]
    for method in methods:
        options = "".join(title(part) for part in method["path"]) + "Options"
        lines += [f"struct {options} {{"]
        lines_items = []
        for lines_field in method["fields"]:
            if not (lines_field["optional"]):
                continue
            lines_items.append(f"  std::optional<{field_type(lines_field)}> {_identifier(lines_field['name'])};")
        lines += lines_items
        lines += ["  std::shared_ptr<grpc::ClientContext> context;", "};"]
    lines += ["}"]

    for node in sorted(nodes, key=lambda node: (-len(node), node)):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = []
        for operations_method in methods:
            if tuple(operations_method["path"][:-1]) != node:
                continue
            operations.append(operations_method)

        lines += [f"struct {names[node]} {{"]
        if operations:
            lines += ["  std::shared_ptr<grpc::Channel> channel;"]
        lines += [f"  {names[child]} {_identifier(child[-1])};" for child in children]
        members = {"channel", *(_identifier(child[-1]) for child in children)}
        members.update(_identifier(method["path"][-1]) for method in operations)
        parameter = _parameter("connection", members)
        initializers = ([f"channel({parameter})"] if operations else []) + [f"{_identifier(child[-1])}({parameter})" for child in children]
        lines += [f"  explicit {names[node]}(std::shared_ptr<grpc::Channel> {parameter}) : {', '.join(initializers)} {{}}"]
        if not node:
            address = _parameter("address", members)
            transfer = _parameter("transfer", members)
            message_bytes = _parameter("message_bytes", members)
            lines += [
                f"  static Client connect(std::string {address}, const ::harmos::StorageTransferSettings& {transfer} = {{}}, std::optional<int> {message_bytes} = {{}}) {{",
                f"    const auto maximum = ::harmos::transfer_message_bytes({transfer}, {message_bytes});",
                "    grpc::ChannelArguments arguments;",
                "    arguments.SetMaxSendMessageSize(maximum);",
                "    arguments.SetMaxReceiveMessageSize(maximum);",
                f'    if ({address}.starts_with("http://")) {{',
                f"      {address}.erase(0, 7);",
                "    }",
                f"    return Client(grpc::CreateCustomChannel({address}, grpc::InsecureChannelCredentials(), arguments));",
                "  }",
            ]
        for method in operations:
            if method["client_streaming"]:
                raise ValueError(
                    f"Generate C++ Client {method['method']}: Declare a Managed Contract for Client Streaming Before Regenerating"
                )
            prefix = "".join(title(part) for part in method["path"])
            request = f"detail::packages::{_private_name(method['request'])}"
            response = f"detail::packages::{_private_name(method['response'])}"
            service = f"detail::packages::{_private_name(method['package'])}::{method['service']}"
            parameters = []
            arguments = {}
            options_parameter = _parameter("options", members)
            maximum_parameter = _parameter("max_item_bytes", members)
            occupied = (
                members
                | {options_parameter, maximum_parameter}
                | {
                    "options",
                    "max_item_bytes",
                    "request",
                    "response",
                    "context",
                    "operation",
                    "incoming",
                    "reader",
                    "status",
                    "key",
                    "item",
                }
            )
            for field in method["fields"]:
                if field["optional"]:
                    continue
                arguments[field["name"]] = _parameter(field["name"], occupied)
                occupied.add(arguments[field["name"]])
            for parameters_field in method["fields"]:
                if parameters_field["optional"]:
                    continue
                parameters.append(f"const {field_type(parameters_field)}& {arguments[parameters_field['name']]}")

            parameters += [f"client::{prefix}Options {options_parameter} = {{}}"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else message(method["response"], type_names)
            stream = method["server_streaming"]
            returns = f"::harmos::ClientStream<{result}>" if stream else result
            name = _identifier(method["path"][-1])
            if name in {"delete", "export", "import", "new", "class", "default"}:
                name += "_"
            lines += [f"  {returns} {name}({', '.join(parameters)}) {{", f"    {request} request;"]
            for field in method["fields"]:
                name = _identifier(field["name"])
                optional = field["optional"]
                if name == "request_id":
                    lines += [
                        f"    request.set_request_id({options_parameter}.request_id ? *{options_parameter}.request_id : ::harmos::detail::request_id());"
                    ]
                    continue
                if optional:
                    lines += [f"    if ({options_parameter}.{name}) {{"]
                value = (
                    f"(*{options_parameter}.{name})"
                    if optional and (field["repeated"] or field["map"])
                    else f"*{options_parameter}.{name}"
                    if optional
                    else arguments[field["name"]]
                )
                indent = "      " if optional else "    "
                if field["map"]:
                    lines += [
                        f"{indent}for (const auto& [key, item] : {value}) {{",
                        f"{indent}  (*request.mutable_{name}())[key] = item;",
                        f"{indent}}}",
                    ]
                elif field["repeated"]:
                    lines += [f"{indent}request.mutable_{name}()->Assign({value}.begin(), {value}.end());"]
                elif field["kind"] == 11:
                    lines += [f"{indent}*request.mutable_{name}() = {value};"]
                else:
                    lines += [f"{indent}request.set_{name}({value});"]
                if optional:
                    lines += ["    }"]
            lines += [
                f"    auto context = {options_parameter}.context ? {options_parameter}.context : std::make_shared<grpc::ClientContext>();"
            ]
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
    format_code([destination, wire, *([destination.parent / "client_messages.hpp"] if namespace != configuration.namespace else [])])


def format_code(paths: list[Path]) -> None:
    # The Policy Travels With the Installed Tools; a Consuming Checkout Has No Library Source Tree.
    policy = "{BasedOnStyle: LLVM, ColumnLimit: 140, AllowShortFunctionsOnASingleLine: Empty, AllowShortLambdasOnASingleLine: None}"
    subprocess.run(["clang-format", f"--style={policy}", "-i", *map(str, paths)], check=True, creationflags=spawn_flags())


# === Private Functions === #


def _parameter(name: str, members: set[str]) -> str:
    name = _identifier(name)
    while name in members:
        name += "value" if name.endswith("_") else "_value"
    return name


def _type_import(qualified: str) -> tuple[str, str]:
    parts = qualified.removeprefix("::").split("::")
    boundary = next((index for index, part in enumerate(parts) if part[0].isupper()), len(parts) - 1)
    return "::".join(parts[boundary:]), ("::" if qualified.startswith("::") else "") + "::".join(parts[: boundary + 1])


def _private_name(full: str) -> str:
    """Keep Package Versions Inside the Private Transport Header."""
    return "::".join(part for part in full.split(".") if not re.fullmatch(r"v[0-9]+", part))


def _identifier(name: str) -> str:
    return (
        name + "_"
        if name
        in {
            "case",
            "default",
            "bitand",
            "catch",
            "struct",
            "char",
            "static_cast",
            "class",
            "constexpr",
            "import",
            "alignas",
            "long",
            "xor_eq",
            "virtual",
            "new",
            "continue",
            "private",
            "static",
            "do",
            "synchronized",
            "mutable",
            "reinterpret_cast",
            "false",
            "for",
            "co_yield",
            "volatile",
            "friend",
            "typename",
            "public",
            "bitor",
            "concept",
            "delete",
            "consteval",
            "asm",
            "inline",
            "noexcept",
            "alignof",
            "bool",
            "export",
            "unsigned",
            "atomic_commit",
            "void",
            "template",
            "atomic_cancel",
            "explicit",
            "xor",
            "char32_t",
            "reflexpr",
            "protected",
            "extern",
            "while",
            "true",
            "switch",
            "const_cast",
            "goto",
            "constinit",
            "if",
            "auto",
            "not_eq",
            "and_eq",
            "int",
            "requires",
            "or_eq",
            "break",
            "or",
            "const",
            "union",
            "nullptr",
            "throw",
            "short",
            "return",
            "thread_local",
            "not",
            "char8_t",
            "namespace",
            "co_await",
            "this",
            "dynamic_cast",
            "try",
            "wchar_t",
            "decltype",
            "atomic_noexcept",
            "operator",
            "typeid",
            "static_assert",
            "co_return",
            "char16_t",
            "and",
            "compl",
            "register",
            "using",
            "signed",
            "module",
            "float",
            "typedef",
            "double",
            "else",
            "enum",
            "sizeof",
        }
        else name
    )


# === Entry Point === #
