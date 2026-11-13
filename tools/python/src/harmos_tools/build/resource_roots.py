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
import argparse
import json
import re
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def table(descriptors: FileDescriptorSet) -> list[dict]:
    methods = []
    leaves = set()
    csharp_types = {}
    for file in descriptors.file:
        namespace = file.options.csharp_namespace or ".".join(part[0].upper() + part[1:] for part in file.package.split("."))
        for message in file.message_type:
            csharp_types[f".{file.package}.{message.name}"] = f"{namespace}.{message.name}"

    for file in descriptors.file:
        parts = file.package.split(".")
        version = next((index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part)), None)
        if version is None or version == 0:
            continue
        face = parts[version - 1] if parts[version - 1] in ("plugin", "context") else "client"
        owner = ".".join(parts[: version - 1] if face != "client" else parts[:version])
        for service in file.service:
            for method in service.method:
                path = [*parts[version + 1 :], snake(service.name), snake(method.name)]
                leaf = (face, *path)
                if leaf in leaves:
                    raise ValueError(f"Client Resource Collision: {'.'.join(path)}")
                leaves.add(leaf)
                methods.append(
                    dict(
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
    for folder in ["sdk", "sdk/python/src/harmos", "sdk/rust/src/client", "sdk/csharp/Harmos", "sdk/cpp/include/harmos/generated"]:
        (root / folder).mkdir(parents=True, exist_ok=True)
    methods = table(descriptors)
    output = root / "sdk/resources.json"
    output.write_text(json.dumps(methods, indent=2) + "\n", encoding="utf-8", newline="\n")
    clients = [method for method in methods if method["face"] == "client"]
    # The Shared Table Is the Only Naming Input to Every Language Generator.
    python = root / "sdk/python/src/harmos/resources.json"
    python.write_text(json.dumps(clients, indent=2) + "\n", encoding="utf-8", newline="\n")
    rust(root, clients)
    if namespace == "harmos":
        rust_context(root, [method for method in methods if method["face"] == "context"])
    csharp(root, clients, namespace=namespace)
    cpp(root, clients, namespace=namespace)
    fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
    if fixture.is_file():
        fixture_methods = table(FileDescriptorSet.FromString(fixture.read_bytes()))
        (root / "sdk/fixtures/client/resources.json").write_text(
            json.dumps(fixture_methods, indent=2) + "\n", encoding="utf-8", newline="\n"
        )
        for folder in ["sdk/csharp/Tests/Generated", "target/generated/cpp"]:
            (root / folder).mkdir(parents=True, exist_ok=True)
        rust(root, fixture_methods, target=fixture.parent / "resources.rs")
        csharp(root, fixture_methods, namespace="fixture", target=root / "sdk/csharp/Tests/Generated/Client.cs")
        cpp(root, fixture_methods, namespace="fixture", target=root / "target/generated/cpp/client.hpp")


def rust(root: Path, methods: list[dict], *, target: Path | None = None) -> None:
    nodes = {()}
    for method in methods:
        for length in range(1, len(method["path"])):
            nodes.add(tuple(method["path"][:length]))
    names = {node: "Root" if not node else "".join(part.title().replace("_", "") for part in node) for node in nodes}
    lines = [
        "// @generated from the protocol descriptor table; do not edit.",
        "use harmos::client::{ClientTransport, Watching};",
        "use prost_reflect::ReflectMessage;",
    ]
    for node in sorted(nodes):
        name = names[node]
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        lines += [
            "#[derive(Clone)]",
            f"pub struct {name} {{",
            *(["    client: ClientTransport,"] if any(tuple(method["path"][:-1]) == node for method in methods) else []),
        ]
        lines += [f"    pub {child[-1]}: {names[child]}," for child in children]
        lines += ["}", f"impl {name} {{", "    pub fn new(client: ClientTransport) -> Self {", "        Self {"]
        lines += [f"            {child[-1]}: {names[child]}::new(client.clone())," for child in children]
        lines += [*(["            client,"] if any(tuple(method["path"][:-1]) == node for method in methods) else []), "        }", "    }"]
        for method in [method for method in methods if tuple(method["path"][:-1]) == node]:
            if method["client_streaming"]:
                raise ValueError(
                    f"Generate Client Root: Explicit Managed Method Required: {method['package']}.{method['service']}/{method['method']}"
                )

            def message(full: str) -> str:
                if full.startswith("harmos.storage.v1."):
                    return "harmos_protocol::storage::" + full.split(".")[-1]
                parts = full.split(".")
                version = next(index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part))
                crate = snake(parts[0]) + "_protocol"
                return crate + "::" + "::".join(parts[version:])

            request, response = message(method["request"]), message(method["response"])
            path = f"/{method['package']}.{method['service']}/{method['method']}"
            result = f"Watching<{response}>" if method["server_streaming"] else f"tonic::Response<{response}>"
            call = "watch_known" if method["server_streaming"] else "unary"
            lines += [
                f"    pub async fn {method['path'][-1]}(&self, request: tonic::Request<{request}>) -> Result<{result}, tonic::Status> {{",
                f'        self.client.{call}(tonic::codegen::http::uri::PathAndQuery::from_static("{path}"), request, {response}::default().descriptor()).await',
                "    }",
            ]
        lines += ["}"]
    target = target or root / "sdk/rust/src/client/resources.rs"
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def rust_context(root: Path, methods: list[dict]) -> None:
    lines = ["// @generated from the protocol descriptor table; do not edit.", "use crate::{Message, OperationRequest};"]
    messages = sorted(
        {method[field] for method in methods for field in ("request", "response")}
        | {"harmos.v1.SettingDeclaration", "harmos.v1.SettingsDeclarations"}
    )

    def path(name):
        parts = name.split(".")
        if parts[1] == "v1":
            return "harmos_protocol::v1::" + "::".join(parts[2:])
        return "harmos_protocol::context::" + "::".join(parts[3:])

    for message in messages:
        lines += [
            f"impl Message for {path(message)} {{",
            f'    fn name() -> String {{ "{message}".into() }}',
            "    fn types() -> prost_types::FileDescriptorSet {",
            f'        prost::Message::decode(&include_bytes!("types/{message}.bin")[..]).expect("generated context wrapper")',
            "    }",
            "}",
        ]
    for method in methods:
        lines += [
            f"impl OperationRequest for {path(method['request'])} {{",
            f"    type Response = {path(method['response'])};",
            f'    const ID: &\'static str = "{method["package"]}.{method["service"]}/{method["method"]}";',
            f"    const WATCH: bool = {str(method['server_streaming']).lower()};",
            "}",
        ]
    (root / "sdk/rust/src/context/contracts.rs").parent.mkdir(parents=True, exist_ok=True)
    (root / "sdk/rust/src/context/contracts.rs").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def csharp(root: Path, methods: list[dict], *, namespace: str = "harmos", target: Path | None = None) -> None:
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    def title(part):
        return "".join(word.title() for word in part.split("_"))

    names = {node: "Client" if not node else "".join(title(part) for part in node) + "Resource" for node in nodes}
    lines = [
        "// @generated from the protocol descriptor table; do not edit.",
        "using Grpc.Core;",
        "using Google.Protobuf;",
        f"namespace {'.'.join(part.title() for part in namespace.split('.'))}.Sdk;",
        "",
    ]
    for node in sorted(nodes):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        builtin = not node and namespace == "harmos" and target is None
        inheritance = " : global::System.IDisposable" if builtin else ""
        lines += [f"public sealed class {names[node]}{inheritance}", "{"]
        if builtin:
            lines += ["  private global::Grpc.Net.Client.GrpcChannel? ownedChannel;"]
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
                "  public static Client Connect(string address, global::Harmos.Storage.V1.StorageTransferSettings? transfer = null, int? messageBytes = null)",
                "  {",
                "    var maximum = Transfer.MessageBytes(transfer, messageBytes);",
                "    var channel = global::Grpc.Net.Client.GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });",
                "    return new Client(channel.CreateCallInvoker()) { ownedChannel = channel };",
                "  }",
                "  public void Dispose() => ownedChannel?.Dispose();",
            ]
        for method in operations:
            request = "global::" + method["request_csharp"]
            response = "global::" + method["response_csharp"]
            stream = method["server_streaming"]
            result = f"AsyncServerStreamingCall<{response}>" if stream else f"AsyncUnaryCall<{response}>"
            kind = "ServerStreaming" if stream else "Unary"
            lines += [
                f"  public {result} {method['method']}({request} request, CallOptions options = default)",
                "  {",
                f'    var method = new Method<{request}, {response}>(MethodType.{kind}, "{method["package"]}.{method["service"]}", "{method["method"]}",',
                f"        Marshallers.Create<{request}>(value => value.ToByteArray(), bytes => {request}.Parser.ParseFrom(bytes)),",
                f"        Marshallers.Create<{response}>(value => value.ToByteArray(), bytes => {response}.Parser.ParseFrom(bytes)));",
                f"    return invoker.Async{kind}Call(method, null, options, request);",
                "  }",
            ]
        lines += ["}"]
    (target or root / "sdk/csharp/Harmos/Client.cs").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def cpp(root: Path, methods: list[dict], *, namespace: str = "harmos", target: Path | None = None) -> None:
    keywords = set(
        "alignas alignof and and_eq asm auto bitand bitor bool break case catch char char8_t char16_t char32_t class compl concept const "
        "consteval constexpr constinit const_cast continue co_await co_return co_yield decltype default delete do double dynamic_cast else "
        "enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept not not_eq nullptr "
        "operator or or_eq private protected public register reinterpret_cast requires return short signed sizeof static static_assert "
        "static_cast struct switch template this thread_local throw true try typedef typeid typename union unsigned using virtual void "
        "volatile wchar_t while xor xor_eq".split()
    )

    def identifier(value):
        return value + "_" if value in keywords else value

    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    def title(part):
        return "".join(word.title() for word in part.split("_"))

    names = {node: "Client" if not node else "".join(title(part) for part in node) + "Resource" for node in nodes}
    includes = sorted({method["file"].removesuffix(".proto") + ".grpc.pb.h" for method in methods})
    lines = [
        "// @generated from the protocol descriptor table; do not edit.",
        "#pragma once",
        "#include <grpcpp/grpcpp.h>",
        "#include <memory>",
        "#include <stdexcept>",
    ]
    builtin = namespace == "harmos" and target is None
    if builtin:
        lines += ['#include "harmos/transfer.hpp"', "#include <string>"]
    # Preserve Platform Macros While Declaring and Calling Wire Method Names.
    macro_names = sorted({method["method"] for method in methods})
    lines += [line for name in macro_names for line in (f'#pragma push_macro("{name}")', f"#undef {name}")]
    lines += [f'#include "{name}"' for name in includes]
    lines += [f"namespace {namespace.replace('.', '::')} {{"]
    for node in sorted(nodes, key=lambda node: (-len(node), node)):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        lines += [f"struct {names[node]} {{"]
        if operations:
            lines += ["  std::shared_ptr<grpc::Channel> channel;"]
        lines += [f"  {names[child]} {identifier(child[-1])};" for child in children]
        initializers = (["channel(channel)"] if operations else []) + [f"{identifier(child[-1])}(channel)" for child in children]
        lines += [f"  explicit {names[node]}(std::shared_ptr<grpc::Channel> channel) : {', '.join(initializers)} {{}}"]
        if not node and builtin:
            lines += [
                "  static Client connect(std::string address, const ::harmos::storage::v1::StorageTransferSettings& transfer = {}, std::optional<int> message_bytes = {}) {",
                "    const auto maximum = ::harmos::transfer_message_bytes(transfer, message_bytes);",
                "    grpc::ChannelArguments arguments; arguments.SetMaxSendMessageSize(maximum); arguments.SetMaxReceiveMessageSize(maximum);",
                '    if (address.starts_with("http://")) address.erase(0, 7);',
                "    return Client(grpc::CreateCustomChannel(address, grpc::InsecureChannelCredentials(), arguments));",
                "  }",
            ]
        for method in operations:
            request = "::" + method["request"].replace(".", "::")
            response = "::" + method["response"].replace(".", "::")
            service = "::" + method["package"].replace(".", "::") + "::" + method["service"]
            if method["server_streaming"]:
                lines += [
                    f"  std::unique_ptr<grpc::ClientReader<{response}>> {identifier(method['path'][-1])}(grpc::ClientContext& context, const {request}& request) {{",
                    f"    return {service}::NewStub(channel)->{method['method']}(&context, request);",
                    "  }",
                ]
            else:
                lines += [
                    f"  {response} {identifier(method['path'][-1])}(grpc::ClientContext& context, const {request}& request) {{",
                    f"    {response} response;",
                    f"    const auto status = {service}::NewStub(channel)->{method['method']}(&context, request, &response);",
                    f'    if (!status.ok()) throw std::runtime_error(std::string("{method["method"]}: ") + status.error_message());',
                    "    return response;",
                    "  }",
                ]
        lines += ["};"]
    lines += ["}"]
    lines += [f'#pragma pop_macro("{name}")' for name in reversed(macro_names)]
    destination = target or root / "sdk/cpp/include/harmos/generated/client.hpp"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def compose(repository: Path, descriptors: list[Path], output: Path, namespace: str) -> None:
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
    generate(output, merged, namespace=namespace)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--descriptors", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--namespace", default="harmos")
    parser.add_argument("--cpp-fixture", type=Path)
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[5]
    if options.cpp_fixture:
        fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
        cpp(root, table(FileDescriptorSet.FromString(fixture.read_bytes())), namespace="fixture", target=options.cpp_fixture)
    elif options.descriptors:
        if options.output is None:
            parser.error("composed client descriptors require --output")
        compose(root, options.descriptors, options.output, options.namespace)
    else:
        generate(root)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
