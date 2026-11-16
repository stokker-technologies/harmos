# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Csharp Resource Contracts."""

# === Imports === #

# Standard Library Imports
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.build.resources.descriptor

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def message(name: str) -> str:
    return (
        name.replace("Harmos.Plugin.V1", "Harmos.Internal.Plugin")
        .replace("Harmos.Host.V1.StatusResources", "Harmos.Clients")
        .replace("Harmos.Host.V1.PluginsResources", "Harmos.Clients")
        .replace("Harmos.Host.V1.AccessResources", "Harmos.Clients")
        .replace("Harmos.Host.V1", "Harmos.Clients")
        .replace("Harmos.Model.V1", "Harmos")
    )


def client(
    root: Path,
    methods: list[harmos_tools.build.resources.descriptor.Method],
    *,
    namespace: str = "harmos",
    target: Path | None = None,
    check: bool = False,
) -> None:
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

    def field_type(field: harmos_tools.build.resources.descriptor.Field) -> str:
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
    lines += [f"using {aliases[full]} = global::{message(full)};" for full in sorted(used)]
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


def host(
    root: Path,
    methods: list[harmos_tools.build.resources.descriptor.Method],
    *,
    check: bool = False,
    target: Path | None = None,
    namespace: str = "Harmos.Plugins",
) -> None:
    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: harmos_tools.build.resources.descriptor.Field) -> str:
        if field["type"]:
            value = field["type"].rsplit(".", 1)[-1]
        else:
            value = {1: "double", 2: "float", 8: "bool", 9: "string", 12: "ByteString", 4: "ulong", 13: "uint"}.get(field["kind"], "long")
        return f"IReadOnlyList<{value}>" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
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
        *(["using Harmos;", "using Harmos.Plugins;", "using Harmos.Clients;"] if target else []),
        *[
            f"using {method[key].rsplit('.', 1)[-1]} = global::{message(method[key + '_csharp'])};"
            for method in methods
            if target is None or method["owner"] != "harmos"
            for key in ("request", "response")
        ],
        f"namespace {namespace};",
        "public enum HostOperation",
        "{",
    ]
    lines += [f"  {''.join(title(part) for part in method['path'])}," for method in methods]
    lines += ["}", "internal static class HostContracts", "{"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        if target is not None and method["owner"] == "harmos":
            continue
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
        contract = f"Harmos.Plugins.HostOperation.{name}" if target is not None and method["owner"] == "harmos" else f"HostContracts.{name}"
        lines += [f"    HostOperation.{name} => plugin.Requires({contract}),"]
    lines += [
        '    _ => throw new ArgumentOutOfRangeException(nameof(operation), "Declare Host Requirement: Unknown Operation")',
        "  };",
        "}",
        "public sealed class HostRoot(Context context)",
        "{",
    ]
    lines += [f"  public {names[node]} {title(node[-1])} {{ get; }} = new(context);" for node in nodes if len(node) == 1]
    lines += ["}"]
    for node in nodes:
        lines += [f"public sealed class {names[node]}(Context context)", "{"]
        lines += [f"  public {names[child]} {title(child[-1])} {{ get; }} = new(context);" for child in nodes if child[:-1] == node]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
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
            lines += [f"  public async {result} {method['method']}({', '.join(parameters)})", "  {"]
            if target is not None and method["owner"] == "harmos":
                path = ".".join(title(part) for part in method["path"][:-1])
                args = [field["name"] for field in method["fields"] if not field["optional"]]
                if stream:
                    args += ["maxItemBytes"]
                args += [field["name"] for field in method["fields"] if field["optional"]]
                args += ([] if stream else ["policy"]) + ["cancellationToken"]
                call = f"context.Host.{path}.{method['method']}({', '.join(args)})"
                if stream:
                    lines += [f"    await foreach (var response in {call}) {{ yield return response; }}"]
                else:
                    lines += [f"    return await {call};"]
                lines += ["  }"]
                continue
            lines += [f"    var request = new {q}", "    {"]
            lines += [f"      {title(field['name'])} = {field['name']}," for field in method["fields"] if not field["optional"]]
            lines += ["    };"]
            lines += [
                f"    if ({field['name']} is not null) request.{title(field['name'])} = {field['name']};"
                for field in method["fields"]
                if field["optional"]
            ]
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
    if target is not None:
        lines += [
            "public sealed class PluginContext(Harmos.Plugins.Context context)",
            "{",
            "  public HostRoot Host { get; } = new(context);",
            "}",
        ]
    destination = target or root / "sdk/csharp/src/Harmos/Generated/Host.g.cs"
    source = "\n".join(lines) + "\n"
    if check:
        if not destination.is_file() or destination.read_text(encoding="utf-8") != source:
            raise ValueError("Check C# Host Resources: Generated Drift; Run mise run generate -- --sdk csharp")
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(source, encoding="utf-8", newline="\n")


# === Private Functions === #

# === Entry Point === #
