# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate C# Client and Plugin Resource Classes."""

# === Imports === #

# Standard Library Imports
from pathlib import Path

# Local Imports
from harmos_tools.build.resources.descriptor import Field, Method

from .configuration import Configuration

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SCALARS = {
    1: "double",
    2: "float",
    3: "long",
    4: "ulong",
    5: "int",
    6: "ulong",
    7: "uint",
    8: "bool",
    9: "string",
    12: "ByteString",
    13: "uint",
    15: "int",
    16: "long",
    17: "int",
    18: "long",
}


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
    methods: list[Method],
    *,
    namespace: str = "harmos",
    configuration: Configuration | None = None,
    target: Path | None = None,
    check: bool = False,
) -> None:
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))

    configuration = configuration or Configuration()

    def title(part: str) -> str:
        return "".join(word.title() for word in part.split("_"))

    def parameter(name: str) -> str:
        value = title(name)
        return _identifier(value[0].lower() + value[1:])

    names = {node: "Client" if not node else "".join(title(part) for part in node) + "Resource" for node in nodes}
    used = {method[key + "_csharp"] for method in methods for key in ("request", "response")}
    used.update(
        field["csharp"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["csharp"]
    )
    message_names = _csharp_names(used)

    def field_type(field: Field) -> str:
        value = message_names[field["csharp"]] if field["csharp"] else SCALARS.get(field["kind"], "long")
        if field["type"].startswith("google.protobuf."):
            value = message_names[field["csharp"]]
        return f"IReadOnlyDictionary<string, {value}>" if field["map"] else f"IReadOnlyList<{value}>" if field["repeated"] else value

    namespaces = {
        "System",
        "System.Collections.Generic",
        "System.Threading",
        "System.Threading.Tasks",
        "System.Runtime.CompilerServices",
        "Grpc.Core",
        "Grpc.Net.Client",
        "Google.Protobuf",
        "Google.Protobuf.WellKnownTypes",
    }
    namespaces.update(message(full).rsplit(".", 1)[0] for full in used if ".Types." not in full)
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#nullable enable",
        *[f"using {name};" for name in sorted(namespaces)],
        f"namespace {'.'.join(part.title() for part in namespace.split('.'))};",
        "",
    ]
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
                "  public static Client Connect(string address, global::Harmos.StorageTransferSettings? transfer = null, int? messageBytes = null)",
                "  {",
                "    var maximum = global::Harmos.Transfer.MessageBytes(transfer, messageBytes);",
                "    var channel = GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });",
                "    return new Client(channel.CreateCallInvoker()) { ownedChannel = channel };",
                "  }",
                "  public void Dispose() => ownedChannel?.Dispose();",
            ]
        for method in operations:
            request, response = (message_names[method[key + "_csharp"]] for key in ("request", "response"))
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
            if configuration.namespace != "harmos" and method["owner"] == "harmos":
                path = ".".join(title(part) for part in method["path"][:-1])
                args = [parameter(field["name"]) for field in [*required, *optional]] + ["context"]
                args += ["cancellationToken"] if stream else []
                call = f"new Harmos.Client(invoker).{path}.{method['method']}({', '.join(args)})"
                lines += [f"  public async {returns} {method['method']}({', '.join(parameters)})", "  {"]
                lines += (
                    [f"    await foreach (var response in {call}) {{ yield return response; }}"]
                    if stream
                    else [f"    return await {call};"]
                )
                lines += ["  }"]
                continue
            lines += [
                f"  public async {returns} {method['method']}({', '.join(parameters)})",
                "  {",
                f"    var request = new {request}",
                "    {",
            ]
            for field in method["fields"]:
                name = parameter(field["name"])
                if field["repeated"] or field["map"]:
                    continue
                if field["optional"] and name != "requestId":
                    continue
                value = 'requestId ?? Guid.NewGuid().ToString("N")' if name == "requestId" else name
                lines += [f"      {title(field['name'])} = {value},"]
            lines += ["    };"]
            for field in method["fields"]:
                name = parameter(field["name"])
                if field["map"]:
                    lines += [
                        f"    foreach (var entry in {name})",
                        "    {",
                        f"      request.{title(field['name'])}.Add(entry.Key, entry.Value);",
                        "    }",
                    ]
                elif field["repeated"]:
                    lines += [f"    request.{title(field['name'])}.Add({name});"]
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
    methods: list[Method],
    *,
    check: bool = False,
    target: Path | None = None,
    namespace: str = "Harmos.Plugins",
    configuration: Configuration | None = None,
) -> None:
    configuration = configuration or Configuration()

    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    used = {method[key + "_csharp"] for method in methods for key in ("request", "response")}
    used.update(field["csharp"] for method in methods for field in method["fields"] if field["csharp"])
    used.update(method["payload_field"]["csharp"] for method in methods if method["payload_field"] and method["payload_field"]["csharp"])
    message_names = _csharp_names(used)

    def field_type(field: Field) -> str:
        if field["type"]:
            value = message_names[field["csharp"]]
        else:
            value = SCALARS.get(field["kind"], "long")
        return f"IReadOnlyDictionary<string, {value}>" if field["map"] else f"IReadOnlyList<{value}>" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
    namespaces = {
        "System",
        "System.Collections.Generic",
        "System.Threading",
        "System.Threading.Tasks",
        "System.Runtime.CompilerServices",
        "Google.Protobuf",
        "Google.Protobuf.WellKnownTypes",
    }
    namespaces.update(message(full).rsplit(".", 1)[0] for full in used if ".Types." not in full)
    if target is not None:
        namespaces.update({"Harmos", "Harmos.Plugins", "Harmos.Clients"})
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "#nullable enable",
        *[f"using {name};" for name in sorted(namespaces)],
        f"namespace {namespace};",
        "public enum HostOperation",
        "{",
    ]
    if configuration.namespace != "harmos":
        lines[-2:-2] = [
            "public sealed partial class Plugin",
            "{",
            "  internal Plugin RequiresLibrary(Harmos.Plugins.HostOperation operation)",
            "  {",
            "    transport.Requires(operation);",
            "    return this;",
            "  }",
            "}",
        ]
    lines += [f"  {''.join(title(part) for part in method['path'])}," for method in methods]
    lines += ["}", "internal static class HostContracts", "{"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        if target is not None and method["owner"] == "harmos":
            continue
        request_type, response_type = (message_names[method[key + "_csharp"]] for key in ("request", "response"))
        lines += [
            f'  internal static readonly Operation<{request_type}, {response_type}> {name} = new("{method["package"]}.{method["service"]}/{method["method"]}", Visibility.Public, watch: {str(method["server_streaming"]).lower()});'
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
        lines += [
            f"    HostOperation.{name} => plugin.{'RequiresLibrary' if configuration.namespace != 'harmos' and method['owner'] == 'harmos' else 'Requires'}({contract}),"
        ]
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
            request_type, response_type = (message_names[method[key + "_csharp"]] for key in ("request", "response"))
            parameters = [f"{field_type(field)} {_identifier(field['name'])}" for field in method["fields"] if not field["optional"]]
            stream = method["server_streaming"]
            if stream:
                parameters += ["ulong maxItemBytes"]
            parameters += [f"{field_type(field)}? {_identifier(field['name'])} = null" for field in method["fields"] if field["optional"]]
            if not stream:
                parameters += ["OperationPolicy? policy = null"]
            parameters += [("[EnumeratorCancellation] " if stream else "") + "CancellationToken cancellationToken = default"]
            payload = method["payload_field"]
            result = field_type(payload) if payload else response_type
            name = "".join(title(part) for part in method["path"])
            result = f"IAsyncEnumerable<{result}>" if stream else f"Task<{result}>"
            lines += [f"  public async {result} {method['method']}({', '.join(parameters)})", "  {"]
            if target is not None and method["owner"] == "harmos":
                path = ".".join(title(part) for part in method["path"][:-1])
                args = [_identifier(field["name"]) for field in method["fields"] if not field["optional"]]
                if stream:
                    args += ["maxItemBytes"]
                args += [_identifier(field["name"]) for field in method["fields"] if field["optional"]]
                args += ([] if stream else ["policy"]) + ["cancellationToken"]
                call = f"context.{'Transport.' if configuration.namespace != 'harmos' else ''}Host.{path}.{method['method']}({', '.join(args)})"
                if stream:
                    lines += [
                        f"    await foreach (var response in {call})",
                        "    {",
                        "      yield return response;",
                        "    }",
                    ]
                else:
                    lines += [f"    return await {call};"]
                lines += ["  }"]
                continue
            lines += [f"    var request = new {request_type}", "    {"]
            for field in method["fields"]:
                if field["optional"] or field["repeated"] or field["map"]:
                    continue
                lines += [f"      {title(field['name'])} = {_identifier(field['name'])},"]
            lines += ["    };"]
            for field in method["fields"]:
                field_name = _identifier(field["name"])
                if field["map"]:
                    lines += [
                        f"    foreach (var entry in {field_name})",
                        "    {",
                        f"      request.{title(field['name'])}.Add(entry.Key, entry.Value);",
                        "    }",
                    ]
                elif field["repeated"]:
                    lines += [f"    request.{title(field['name'])}.Add({field_name});"]
            for field in method["fields"]:
                if not field["optional"]:
                    continue
                lines += [
                    f"    if ({_identifier(field['name'])} is not null)",
                    "    {",
                    f"      request.{title(field['name'])} = {_identifier(field['name'])}"
                    + ("" if field["kind"] in {9, 11, 12} else ".Value")
                    + ";",
                    "    }",
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
    if target is not None and configuration.namespace == "harmos":
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


def _identifier(name: str) -> str:
    return (
        "@" + name
        if name
        in {
            "enum",
            "object",
            "fixed",
            "is",
            "continue",
            "foreach",
            "class",
            "protected",
            "in",
            "internal",
            "virtual",
            "using",
            "do",
            "public",
            "case",
            "break",
            "string",
            "lock",
            "try",
            "int",
            "if",
            "unchecked",
            "return",
            "finally",
            "this",
            "extern",
            "true",
            "volatile",
            "void",
            "namespace",
            "stackalloc",
            "unsafe",
            "default",
            "base",
            "goto",
            "implicit",
            "else",
            "new",
            "false",
            "readonly",
            "switch",
            "params",
            "override",
            "ref",
            "sizeof",
            "typeof",
            "as",
            "throw",
            "while",
            "explicit",
            "for",
            "struct",
            "delegate",
            "private",
            "const",
            "sealed",
            "checked",
            "null",
            "event",
            "interface",
            "static",
            "operator",
            "out",
        }
        else name
    )


def _csharp_names(used: set[str]) -> dict[str, str]:
    names = {full: full.rsplit(".", 1)[-1] for full in used}
    duplicates = {name for name in names.values() if list(names.values()).count(name) > 1}
    for full in used:
        if names[full] in duplicates or ".Types." in full:
            names[full] = "global::" + message(full)
    return names


# === Entry Point === #
