# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate C++ Resources and Public Payload Exports."""

# === Imports === #

# Standard Library Imports
import re
import subprocess
from dataclasses import dataclass
from functools import cache
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from ...process import spawn_flags
from .descriptors import Field, Method, payload_audiences, snake, table

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


@dataclass(frozen=True)
class PayloadExport:
    boundary: str
    name: str
    qualified: str
    constant: bool = False


# === Public Functions === #


def cpp_exports(root: Path, descriptors: FileDescriptorSet) -> None:
    methods = table(descriptors)
    audiences = payload_audiences(descriptors)
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
    public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "detail/messages.hpp"']
    entries: list[PayloadExport] = []
    for file in descriptors.file:
        if file.name.endswith("guest.proto") or not file.package.startswith("harmos.") or ".plugin." in file.package:
            continue
        private += [f'#include "{file.name.removesuffix(".proto")}.pb.h"']
        for definition in [*file.message_type, *file.enum_type]:
            groups_for_type = audiences.get(f"{file.package}.{definition.name}", {"client"})
            boundary = "" if ".model." in file.package or len(groups_for_type) > 1 else next(iter(groups_for_type))
            if f"{file.package}.{definition.name}" in requests or definition.name in carriers:
                continue
            name = "FailureValue" if definition.name == "Failure" else definition.name
            entries.append(PayloadExport(boundary, name, f"::{file.package.replace('.', '::')}::{definition.name}"))
        for definition in file.enum_type:
            for value in definition.value:
                prefix = snake(definition.name).upper() + "_"
                name = value.name if value.name.startswith(prefix) else prefix + value.name
                entries.append(
                    PayloadExport(
                        boundary,
                        name,
                        f"static_cast<::{file.package.replace('.', '::')}::{definition.name}>({value.number})",
                        True,
                    )
                )
    private += [
        "namespace harmos::detail {",
        'inline constexpr char stream_fingerprint_prefix[] = "harmos.protobuf.stream.v1\\0";',
        "namespace payloads {",
    ]
    for boundary in ("", "client", "plugin"):
        if boundary:
            private.append(f"namespace {boundary} {{")
        for entry in entries:
            if entry.boundary != boundary:
                continue
            if entry.constant:
                declaration = f"inline constexpr auto {entry.name} = {entry.qualified};"
            elif entry.name == entry.qualified.rsplit("::", 1)[-1]:
                declaration = f"using {entry.qualified};"
            else:
                # FailureValue Preserves the Public Distinction From Runtime Failures.
                declaration = f"using {entry.name} = {entry.qualified};"
            private.append(declaration)
        if boundary:
            private.append("}")
    private += ["}", "}"]
    for boundary in ("", "client", "plugin"):
        public += ["namespace harmos" + (f"::{boundary}" if boundary else "") + " {"]
        for entry in entries:
            if entry.boundary != boundary:
                continue
            qualified = (boundary + "::" if boundary else "") + entry.name
            public.append(f"using ::harmos::detail::payloads::{qualified};")
        public += ["}"]
    destination = root / "sdk/cpp/include/harmos"
    (destination / "detail").mkdir(parents=True, exist_ok=True)
    (destination / "detail/messages.hpp").write_text("\n".join(private) + "\n", encoding="utf-8", newline="\n")
    (destination / "messages.hpp").write_text("\n".join(public) + "\n", encoding="utf-8", newline="\n")
    format_cpp([destination / "messages.hpp", destination / "detail/messages.hpp"])


def cpp_host(root: Path, methods: list[Method], *, target: Path | None = None, namespace: str = "harmos") -> None:
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

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
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
                f"  std::optional<{value}> next() {{",
                "    return next_();",
                "  }",
                "private:",
                f"  std::function<std::optional<{value}>()> next_;",
                "};",
            ]
    for node in reversed(nodes):
        children = [child for child in nodes if child[:-1] == node]
        header += [f"struct {names[node]} {{", "  Context context;"]
        header += [f"  {names[child]} {child[-1]};" for child in children]
        initializers = ["context(value)", *[f"{child[-1]}(value)" for child in children]]
        header += [f"  {names[node]}(Context value) : {', '.join(initializers)} {{}}"]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            name = "".join(title(part) for part in method["path"])
            request_type, response_type = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            payload = method["payload_field"]
            result = f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else cpp_message(method["response"])
            parameters = [f"const {field_type(field)}& {field['name']}" for field in method["fields"] if not field["optional"]]
            if method["server_streaming"]:
                parameters += ["std::size_t max_item_bytes"]
            header += [f"  {result} {method['path'][-1]}({', '.join([*parameters, 'Host' + name + 'Options options = {}'])});"]
            source += [
                f"{result} {names[node]}::{method['path'][-1]}({', '.join([*parameters, 'Host' + name + 'Options options'])}) {{",
                f"  {request_type} request;",
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
                f'  auto operation = Operation<{request_type}, {response_type}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC);'
            ]
            if method["server_streaming"]:
                source += [
                    "  (void)options;",
                    "  operation.watch();",
                    f"  auto incoming = std::make_shared<Watching<{response_type}>>(context.watch(operation, request, max_item_bytes));",
                    f"  return Host{name}([incoming]() -> std::optional<{field_type(payload) if payload else cpp_message(method['response'])}> {{",
                    "    auto response = incoming->next();",
                    "    if (!response) {",
                    "      return {};",
                    "    }",
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
                        f"    if (!response->has_{payload['name']}()) {{",
                        f'      throw std::runtime_error("Read {name}: Missing Payload");',
                        "    }",
                    ]
                source += [f"    return {value};", "  });"]
            else:
                source += ["  auto response = context.call(operation, request, options.policy).get();"]
                if payload and payload["repeated"]:
                    value = f"{field_type(payload)}(response.{payload['name']}().begin(), response.{payload['name']}().end())"
                elif payload and payload["kind"] == 11:
                    source += [
                        f"  if (!response.has_{payload['name']}()) {{",
                        f'    throw std::runtime_error("Read {name}: Missing Payload");',
                        "  }",
                    ]
                    value = f"std::move(*response.mutable_{payload['name']}())"
                else:
                    value = f"response.{payload['name']}()" if payload else "response"
                source += [f"  return {value};"]
            source += ["}"]
        header += ["};"]
    header += ["struct HostRoot {"]
    header += [f"  {names[node]} {node[-1]};" for node in nodes if len(node) == 1]
    header += ["};", "}"]
    source += [
        "}",
        "namespace harmos::plugin {",
        "HostRoot Context::host() {",
        "  return {" + ", ".join("{*this}" for node in nodes if len(node) == 1) + "};",
        "}",
        "Plugin& Plugin::requires_operation(HostOperation operation) {",
        "  switch (operation) {",
    ]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        request_type, response_type = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
        expression = (
            f'Operation<{request_type}, {response_type}>("{method["package"]}.{method["service"]}/{method["method"]}", VISIBILITY_PUBLIC)'
            + (".watch()" if method["server_streaming"] else "")
        )
        source += [f"    case HostOperation::{name}:", f"      return requires_operation({expression});"]
    source += [
        "    default:",
        '      throw std::invalid_argument("Declare Host Requirement: Unknown Operation");',
        "  }",
        "}",
        "}",
    ]
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
    if target is not None:
        public = "\n".join(header).replace("namespace harmos::plugin {", f"namespace {namespace.replace('.', '::')}::plugin {{")
        public = re.sub(r"\bContext\b", "::harmos::plugin::Context", public)
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
        format_cpp([target, target.with_suffix(".cpp")])
        return
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
    if full.startswith("harmos.host."):
        # Shared Host Replies Belong to the Root; Audience-Exclusive Replies to Their Barrel.
        return _host_payload_names().get(full, "::harmos::client::" + parts[-1])
    if full.startswith("harmos.model."):
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
    if namespace != "harmos":
        public = ["// @generated from protocol descriptors; do not edit.", "#pragma once", '#include "detail/client.hpp"']
        for method in methods:
            if method["owner"] == "harmos":
                continue
            public += [
                f"namespace {method['owner'].replace('.', '::')}::client {{ using ::{namespace.replace('.', '::')}::detail::packages::{_private_name(method['response'])}; }}"
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
        initializers = (["channel(transport)"] if operations else []) + [f"{child[-1]}(transport)" for child in children]
        lines += [f"  explicit {names[node]}(std::shared_ptr<grpc::Channel> transport) : {', '.join(initializers)} {{}}"]
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
            request = f"detail::packages::{_private_name(method['request'])}"
            response = f"detail::packages::{_private_name(method['response'])}"
            service = f"detail::packages::{_private_name(method['package'])}::{method['service']}"
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
                    "        if (!status.ok()) {",
                    f'          throw std::runtime_error("Read {action}: " + status.error_message());',
                    "        }",
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
    policy = Path(__file__).resolve().parents[6] / "sdk/cpp/.clang-format"
    subprocess.run(["clang-format", f"--style=file:{policy}", "-i", *map(str, paths)], check=True, creationflags=spawn_flags())


# === Private Functions === #


def _private_name(full: str) -> str:
    """Keep Package Versions Inside the Private Transport Header."""
    return "::".join(part for part in full.split(".") if not re.fullmatch(r"v[0-9]+", part))


@cache
def _host_payload_names() -> dict[str, str]:
    library = Path(__file__).resolve().parents[6] / "crates/harmos-protocol/src/descriptors.bin"
    schema = FileDescriptorSet.FromString(library.read_bytes())
    names = {}
    for full, groups in payload_audiences(schema).items():
        if not full.startswith("harmos.host."):
            continue
        boundary = "" if len(groups) > 1 else next(iter(groups)) + "::"
        names[full] = "::harmos::" + boundary + full.rsplit(".", 1)[-1]
    return names


# === Entry Point === #
