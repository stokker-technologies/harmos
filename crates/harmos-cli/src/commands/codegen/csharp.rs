/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate C# SDK Bindings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

// Third Party Imports
use prost_types::FileDescriptorProto;

// Local Imports
use crate::commands::codegen::{Generation, Inputs, Output, failure, flag, identifier_character, replace_word};
use crate::descriptor::{Field, Method, camel, title};
use crate::error::{Error, Result};

/* === Constants === */

const HEADER: &str = "// @generated from protocol descriptors; do not edit.";

const KEYWORDS: &[&str] = &[
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
];

/// Carrier messages that stay internal next to the call requests.
const CARRIERS: &[&str] =
    &["Request", "Response", "ChildRequest", "ChildResponse", "ChildCancel", "ChildWatch", "ChildWatchResponse", "ChildWatchCancel"];

/// Top-level wire declarations the carrier projection makes internal.
const CARRIER_DECLARATIONS: &[&str] = &["sealed partial class ", "static partial class ", "enum "];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(flatten)]
    inputs: Inputs,

    /// C# namespace of the client; the host lives in `<NAMESPACE>.Plugins` [default: the title-cased --namespace].
    #[arg(long, value_name = "NAMESPACE")]
    csharp_namespace: Option<String>,

    /// Compile the wire messages below `Wire/` with this protoc, replacing the folder.
    #[arg(long, value_name = "EXE", requires = "grpc_plugin")]
    protoc: Option<PathBuf>,

    /// The `grpc_csharp_plugin` that compiles wire services.
    #[arg(long, value_name = "EXE", requires = "protoc")]
    grpc_plugin: Option<PathBuf>,

    /// Rename a type as a whole word in every generated file, as `Old=New` (repeatable).
    #[arg(long = "rename-type", value_name = "OLD=NEW")]
    renames: Vec<String>,

    /// Emit the client classes as internal in this namespace.
    #[arg(long, value_name = "NAMESPACE")]
    internal_client: Option<String>,

    /// Emit the host resources as internal.
    #[arg(long)]
    internal_host: bool,

    /// Emit the top-level wire types of `<NAMESPACE>.Clients` namespaces as internal.
    #[arg(long, requires = "protoc")]
    internal_carriers: bool,

    /// Move this wire enum from `<NAMESPACE>.Clients` into `<NAMESPACE>`, declared in `Values.g.cs`.
    #[arg(long, value_name = "ENUM", requires = "protoc")]
    lift_enum: Option<String>,
}

/// Text projections applied to every generated file, in order.
struct Projection<'a> {
    namespace: &'a str,
    renames: Vec<(&'a str, &'a str)>,
    lifted: Option<&'a str>,
    carriers: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Projection<'_> {
    /// Rename types, lift the enum declaration into `values` and make carrier namespaces internal.
    fn apply(&self, mut source: String, values: &mut String) -> String {
        let namespace = self.namespace;
        for (old, new) in &self.renames {
            source = replace_word(&source, old, new, identifier_character);
        }
        if let Some(name) = self.lifted {
            source = source.replace(&format!("global::{namespace}.Clients.{name}"), &format!("global::{namespace}.{name}"));
            let declaration = format!("  public enum {name} {{");
            if let Some(start) = source.find(&declaration)
                && let Some(length) = source[start + declaration.len()..].find('}')
            {
                let end = start + declaration.len() + length + 1;
                *values = source[start..end].replace("pbr::OriginalName", "OriginalName");
                source.replace_range(start..end, "");
            }
        }
        let carrier = format!("namespace {namespace}.Clients");
        let carried =
            source.match_indices(&carrier).any(|(index, _)| matches!(source[index + carrier.len()..].chars().next(), Some('.' | ';')));
        if self.carriers && carried {
            let lines = source.split('\n').map(|line| match line.strip_prefix("  public ") {
                Some(rest) if CARRIER_DECLARATIONS.iter().any(|declaration| rest.starts_with(declaration)) => format!("  internal {rest}"),
                _ => line.to_owned(),
            });
            source = lines.collect::<Vec<_>>().join("\n");
        }
        source
    }
}

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    let generation = args.inputs.load()?;
    let namespace = args.csharp_namespace.clone().unwrap_or_else(|| title(&generation.namespace));
    let product = !generation.application && generation.namespace != "harmos";
    let delegated = generation.application || product;
    let mut renames = Vec::new();
    for value in &args.renames {
        let Some(pair) = value.split_once('=') else {
            return Err(Error::Input(format!("Generate C# SDK: Write --rename-type {value} as Old=New")));
        };
        renames.push(pair);
    }
    let projection = Projection {
        namespace: &namespace,
        renames,
        lifted: args.lift_enum.as_deref(),
        carriers: args.internal_carriers,
    };
    let mut values = String::new();
    let mut output = Output::default();
    if delegated || !generation.clients.is_empty() {
        let (client_namespace, visibility) = match &args.internal_client {
            Some(internal) => (internal.as_str(), "internal"),
            None => (namespace.as_str(), "public"),
        };
        output.add_source("Client.g.cs", projection.apply(client(&generation.clients, client_namespace, visibility, product), &mut values));
    }
    let visibility = if args.internal_host { "internal" } else { "public" };
    let source = host(&generation.contexts, &format!("{namespace}.Plugins"), visibility, product, delegated);
    output.add_source(if generation.application { "Plugin.g.cs" } else { "Host.g.cs" }, projection.apply(source, &mut values));
    if let (Some(protoc), Some(plugin)) = (&args.protoc, &args.grpc_plugin) {
        let scratch = env::temp_dir().join(format!("harmos-csharp-{}", process::id()));
        let sources = wire(&scratch, &args.inputs.descriptors, protoc, plugin, &generation);
        fs::remove_dir_all(&scratch).ok();
        for (path, source) in sources? {
            output.add_source(path, projection.apply(source, &mut values));
        }
    }
    if let Some(name) = &args.lift_enum {
        if values.is_empty() {
            return Err(Error::Input(format!(
                "Generate C# SDK: Wire Enum {name} Not Found; Supply Descriptors That Declare It in {namespace}.Clients"
            )));
        }
        output.add_source("Values.g.cs", format!("{HEADER}\nusing Google.Protobuf.Reflection;\nnamespace {namespace};\n{values}\n"));
    }
    let folder = args.inputs.out().join("Wire");
    if args.protoc.is_some() && folder.is_dir() {
        fs::remove_dir_all(&folder).map_err(failure("Clear Wire Folder", &folder))?;
    }
    output.write(args.inputs.out())
}

/* === Private Functions === */

/// Typed client resources over a call invoker.
fn client(methods: &[Method], namespace: &str, visibility: &str, product: bool) -> String {
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::from([Vec::new()]);
    for method in methods {
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    let name = |node: &[String]| if node.is_empty() { "Client".to_owned() } else { format!("{}Resource", joined(node)) };
    let usings = BTreeSet::from([
        "System",
        "System.Collections.Generic",
        "System.Threading",
        "System.Threading.Tasks",
        "System.Runtime.CompilerServices",
        "Grpc.Core",
        "Grpc.Net.Client",
        "Google.Protobuf",
        "Google.Protobuf.WellKnownTypes",
    ]);
    let mut lines = vec![HEADER.to_owned(), "#nullable enable".to_owned()];
    lines.extend(usings.iter().map(|using| format!("using {using};")));
    lines.extend([format!("namespace {namespace};"), String::new()]);
    for node in &nodes {
        let children: Vec<&Vec<String>> =
            nodes.iter().filter(|child| child.len() == node.len() + 1 && child[..node.len()] == node[..]).collect();
        let operations: Vec<&Method> = methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]).collect();
        let builtin = node.is_empty();

        // Render the Resource and Its Children
        lines.extend([format!("{visibility} sealed class {}{}", name(node), if builtin { " : IDisposable" } else { "" }), "{".to_owned()]);
        if builtin {
            lines.push("  private GrpcChannel? ownedChannel;".to_owned());
        }
        if !operations.is_empty() {
            lines.push("  private readonly CallInvoker invoker;".to_owned());
        }
        lines.extend(children.iter().map(|child| format!("  public {} {} {{ get; }}", name(child), camel(&child[node.len()]))));
        lines.extend([format!("  public {}(CallInvoker invoker)", name(node)), "  {".to_owned()]);
        if !operations.is_empty() {
            lines.push("    this.invoker = invoker;".to_owned());
        }
        lines.extend(children.iter().map(|child| format!("    {} = new {}(invoker);", camel(&child[node.len()]), name(child))));
        lines.push("  }".to_owned());
        if builtin {
            lines.extend(
                [
                    "  public static Client Connect(string address, global::Harmos.StorageTransferSettings? transfer = null, int? messageBytes = null)",
                    "  {",
                    "    var maximum = global::Harmos.Transfer.MessageBytes(transfer, messageBytes);",
                    "    var channel = GrpcChannel.ForAddress(address, new() { MaxSendMessageSize = maximum, MaxReceiveMessageSize = maximum });",
                    "    return new Client(channel.CreateCallInvoker()) { ownedChannel = channel };",
                    "  }",
                    "  public void Dispose() => ownedChannel?.Dispose();",
                ]
                .map(str::to_owned),
            );
        }

        // Render Each Call
        for method in operations {
            let request = qualified(&method.request_csharp);
            let response = qualified(&method.response_csharp);
            let required: Vec<&Field> = method.fields.iter().filter(|field| !field.optional).collect();
            let optional: Vec<&Field> = method.fields.iter().filter(|field| field.optional).collect();
            let mut parameters: Vec<String> =
                required.iter().map(|field| format!("{} {}", field_type(field), parameter(&field.name))).collect();
            parameters.extend(optional.iter().map(|field| format!("{}? {} = null", field_type(field), parameter(&field.name))));
            parameters.push("CallOptions context = default".to_owned());
            let payload = method.payload_field.as_ref();
            let result = payload.map_or_else(|| response.clone(), field_type);
            let stream = method.server_streaming;
            if stream {
                parameters.push("[EnumeratorCancellation] CancellationToken cancellationToken = default".to_owned());
            }
            let returns = if stream { format!("IAsyncEnumerable<{result}>") } else { format!("Task<{result}>") };
            let kind = if stream { "ServerStreaming" } else { "Unary" };
            let signature = format!("  public async {returns} {}({})", method.method, parameters.join(", "));
            if product && method.owner == "harmos" {
                let path = method.path[..method.path.len() - 1].iter().map(|part| camel(part)).collect::<Vec<_>>().join(".");
                let mut args: Vec<String> = required.iter().chain(&optional).map(|field| parameter(&field.name)).collect();
                args.push("context".to_owned());
                if stream {
                    args.push("cancellationToken".to_owned());
                }
                let call = format!("new Harmos.Client(invoker).{path}.{}({})", method.method, args.join(", "));
                lines.extend([signature, "  {".to_owned()]);
                lines.push(if stream {
                    format!("    await foreach (var response in {call}) {{ yield return response; }}")
                } else {
                    format!("    return await {call};")
                });
                lines.push("  }".to_owned());
                continue;
            }
            lines.extend([signature, "  {".to_owned(), format!("    var request = new {request}"), "    {".to_owned()]);
            for field in &method.fields {
                let name = parameter(&field.name);
                if field.repeated || field.map || (field.optional && name != "requestId") {
                    continue;
                }
                let value = if name == "requestId" { "requestId ?? Guid.NewGuid().ToString(\"N\")".to_owned() } else { name };
                lines.push(format!("      {} = {value},", camel(&field.name)));
            }
            lines.push("    };".to_owned());
            for field in &method.fields {
                let name = parameter(&field.name);
                if field.map {
                    lines.extend([
                        format!("    foreach (var entry in {name})"),
                        "    {".to_owned(),
                        format!("      request.{}.Add(entry.Key, entry.Value);", camel(&field.name)),
                        "    }".to_owned(),
                    ]);
                } else if field.repeated {
                    lines.push(format!("    request.{}.Add({name});", camel(&field.name)));
                }
            }
            for field in optional.iter().filter(|field| field.name != "request_id") {
                let name = parameter(&field.name);
                let value = if [9, 11, 12].contains(&field.kind) { name.clone() } else { format!("{name}.Value") };
                lines.extend([
                    format!("    if ({name} is not null)"),
                    "    {".to_owned(),
                    format!("      request.{} = {value};", camel(&field.name)),
                    "    }".to_owned(),
                ]);
            }
            lines.extend([
                format!(
                    "    var method = new Method<{request}, {response}>(MethodType.{kind}, \"{}.{}\", \"{}\",",
                    method.package, method.service, method.method
                ),
                format!("        Marshallers.Create<{request}>(value => value.ToByteArray(), bytes => {request}.Parser.ParseFrom(bytes)),"),
                format!(
                    "        Marshallers.Create<{response}>(value => value.ToByteArray(), bytes => {response}.Parser.ParseFrom(bytes)));"
                ),
            ]);
            let failure = format!(
                "throw new RpcException(new Status(error.StatusCode, \"Call {}: \" + error.Status.Detail), error.Trailers);",
                method.path.join(".")
            );
            let mut value = payload.map_or_else(|| "response".to_owned(), |payload| format!("response.{}", camel(&payload.name)));
            if payload.is_some_and(|payload| payload.kind == 11 && !payload.repeated) {
                value = format!(
                    "{value} ?? throw new InvalidOperationException(\"Read {}.{}: Missing Payload\")",
                    method.service, method.method
                );
            }
            if stream {
                lines.extend(
                    [
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
                    ]
                    .map(str::to_owned),
                );
                lines.push(format!("        {failure}"));
                lines.extend(
                    [
                        "      }",
                        "      if (!available)",
                        "      {",
                        "        yield break;",
                        "      }",
                        "      var response = call.ResponseStream.Current;",
                    ]
                    .map(str::to_owned),
                );
                lines.extend([format!("      yield return {value};"), "    }".to_owned()]);
            } else {
                lines.extend([
                    "    using var call = invoker.AsyncUnaryCall(method, null, context, request);".to_owned(),
                    format!("    {response} response;"),
                ]);
                lines.extend(
                    ["    try", "    {", "      response = await call.ResponseAsync;", "    }", "    catch (RpcException error)", "    {"]
                        .map(str::to_owned),
                );
                lines.extend([format!("      {failure}"), "    }".to_owned(), format!("    return {value};")]);
            }
            lines.push("  }".to_owned());
        }
        lines.push("}".to_owned());
    }
    format!("{}\n", lines.join("\n"))
}

/// Host resources over a plugin context, with the operation inventory they register.
fn host(methods: &[Method], namespace: &str, visibility: &str, product: bool, delegated: bool) -> String {
    let nodes: BTreeSet<Vec<String>> = methods
        .iter()
        .filter(|method| method.surface)
        .flat_map(|method| (1..method.path.len()).map(|length| method.path[..length].to_vec()))
        .collect();
    let name = |node: &[String]| format!("Host{}Resource", joined(node));
    let mut usings = BTreeSet::from([
        "System",
        "System.Collections.Generic",
        "System.Threading",
        "System.Threading.Tasks",
        "System.Runtime.CompilerServices",
        "Google.Protobuf",
        "Google.Protobuf.WellKnownTypes",
    ]);
    if delegated {
        usings.extend(["Harmos", "Harmos.Plugins", "Harmos.Clients"]);
    }

    // Render the Operation Inventory and Its Contracts
    let mut lines = vec![HEADER.to_owned(), "#nullable enable".to_owned()];
    lines.extend(usings.iter().map(|using| format!("using {using};")));
    lines.push(format!("namespace {namespace};"));
    if product {
        lines.extend([format!("{visibility} sealed partial class Plugin"), "{".to_owned()]);
        lines.extend(
            [
                "  internal Plugin RequiresLibrary(Harmos.Plugins.HostOperation operation)",
                "  {",
                "    transport.Register(ctx => ctx.Register(operation));",
                "    return this;",
                "  }",
                "}",
            ]
            .map(str::to_owned),
        );
    }
    lines.extend([format!("{visibility} enum HostOperation"), "{".to_owned()]);
    lines.extend(methods.iter().map(|method| format!("  {},", joined(&method.path))));
    lines.extend(["}", "internal static class HostContracts", "{"].map(str::to_owned));
    for method in methods.iter().filter(|method| !(delegated && method.owner == "harmos")) {
        lines.push(format!(
            "  internal static readonly Operation<{}, {}> {} = new(\"{}\", Visibility.Public, watch: {});",
            qualified(&method.request_csharp),
            qualified(&method.response_csharp),
            joined(&method.path),
            method.identity(),
            method.server_streaming
        ));
    }
    let requirement = if product {
        "  public static Plugin Requires(this Plugin plugin, HostOperation operation) => operation switch"
    } else {
        "  public static Registrar Register(this Registrar ctx, HostOperation operation) => operation switch"
    };
    lines.extend([
        "}".to_owned(),
        format!("{visibility} static class HostRequirements"),
        "{".to_owned(),
        requirement.to_owned(),
        "  {".to_owned(),
    ]);
    for method in methods {
        let name = joined(&method.path);
        let contract = if delegated && method.owner == "harmos" {
            format!("Harmos.Plugins.HostOperation.{name}")
        } else {
            format!("HostContracts.{name}")
        };
        lines.push(if product {
            format!(
                "    HostOperation.{name} => plugin.{}({contract}),",
                if method.owner == "harmos" { "RequiresLibrary" } else { "Requires" }
            )
        } else {
            format!("    HostOperation.{name} => ctx.Register({contract}),")
        });
    }
    lines.extend(
        ["    _ => throw new ArgumentOutOfRangeException(nameof(operation), \"Declare Host Requirement: Unknown Operation\")", "  };", "}"]
            .map(str::to_owned),
    );
    lines.extend([format!("{visibility} sealed class HostRoot(Context context)"), "{".to_owned()]);
    lines.extend(
        nodes
            .iter()
            .filter(|node| node.len() == 1)
            .map(|node| format!("  public {} {} {{ get; }} = new(context);", name(node), camel(&node[0]))),
    );
    lines.push("}".to_owned());

    // Render Each Resource and Its Calls
    for node in &nodes {
        lines.extend([format!("{visibility} sealed class {}(Context context)", name(node)), "{".to_owned()]);
        let children = nodes.iter().filter(|child| child[..child.len() - 1] == node[..]);
        lines.extend(children.map(|child| format!("  public {} {} {{ get; }} = new(context);", name(child), camel(&child[node.len()]))));
        for method in methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]) {
            let request = qualified(&method.request_csharp);
            let response = qualified(&method.response_csharp);
            let stream = method.server_streaming;
            let required: Vec<&Field> = method.fields.iter().filter(|field| !field.optional).collect();
            let optional: Vec<&Field> = method.fields.iter().filter(|field| field.optional).collect();
            let mut parameters: Vec<String> =
                required.iter().map(|field| format!("{} {}", field_type(field), identifier(&field.name))).collect();
            if stream {
                parameters.push("ulong maxItemBytes".to_owned());
            }
            parameters.extend(optional.iter().map(|field| format!("{}? {} = null", field_type(field), identifier(&field.name))));
            if !stream {
                parameters.push("OperationPolicy? policy = null".to_owned());
            }
            parameters
                .push(format!("{}CancellationToken cancellationToken = default", if stream { "[EnumeratorCancellation] " } else { "" }));
            let payload = method.payload_field.as_ref();
            let result = payload.map_or_else(|| response.clone(), field_type);
            let name = joined(&method.path);
            let result = if stream { format!("IAsyncEnumerable<{result}>") } else { format!("Task<{result}>") };
            lines.extend([format!("  public async {result} {}({})", method.method, parameters.join(", ")), "  {".to_owned()]);
            if delegated && method.owner == "harmos" {
                let path = method.path[..method.path.len() - 1].iter().map(|part| camel(part)).collect::<Vec<_>>().join(".");
                let mut args: Vec<String> = required.iter().map(|field| identifier(&field.name)).collect();
                if stream {
                    args.push("maxItemBytes".to_owned());
                }
                args.extend(optional.iter().map(|field| identifier(&field.name)));
                if !stream {
                    args.push("policy".to_owned());
                }
                args.push("cancellationToken".to_owned());
                let call = format!("context.{}Host.{path}.{}({})", if product { "Transport." } else { "" }, method.method, args.join(", "));
                if stream {
                    lines.extend([
                        format!("    await foreach (var response in {call})"),
                        "    {".to_owned(),
                        "      yield return response;".to_owned(),
                        "    }".to_owned(),
                    ]);
                } else {
                    lines.push(format!("    return await {call};"));
                }
                lines.push("  }".to_owned());
                continue;
            }
            lines.extend([format!("    var request = new {request}"), "    {".to_owned()]);
            for field in &method.fields {
                if field.name == "request_id" {
                    lines.push("      RequestId = request_id ?? Guid.NewGuid().ToString(\"N\"),".to_owned());
                    continue;
                }
                if field.optional || field.repeated || field.map {
                    continue;
                }
                lines.push(format!("      {} = {},", camel(&field.name), identifier(&field.name)));
            }
            lines.push("    };".to_owned());
            for field in &method.fields {
                let field_name = identifier(&field.name);
                if field.map {
                    lines.extend([
                        format!("    foreach (var entry in {field_name})"),
                        "    {".to_owned(),
                        format!("      request.{}.Add(entry.Key, entry.Value);", camel(&field.name)),
                        "    }".to_owned(),
                    ]);
                } else if field.repeated {
                    lines.push(format!("    request.{}.Add({field_name});", camel(&field.name)));
                }
            }
            for field in method.fields.iter().filter(|field| field.optional && field.name != "request_id") {
                let field_name = identifier(&field.name);
                let value = if [9, 11, 12].contains(&field.kind) { "" } else { ".Value" };
                lines.extend([
                    format!("    if ({field_name} is not null)"),
                    "    {".to_owned(),
                    format!("      request.{} = {field_name}{value};", camel(&field.name)),
                    "    }".to_owned(),
                ]);
            }
            let mut value = payload.map_or_else(|| "response".to_owned(), |payload| format!("response.{}", camel(&payload.name)));
            if payload.is_some_and(|payload| payload.kind == 11 && !payload.repeated) {
                value = format!("{value} ?? throw new InvalidOperationException(\"Read {name}: Missing Payload\")");
            }
            if stream {
                lines.extend([
                    format!("    await foreach (var response in context.WatchAsync(HostContracts.{name}, request, maxItemBytes, cancellationToken))"),
                    "    {".to_owned(),
                    format!("      yield return {value};"),
                    "    }".to_owned(),
                ]);
            } else {
                lines.extend([
                    format!("    var response = await context.Operation(HostContracts.{name}, request, policy, cancellationToken);"),
                    format!("    return {value};"),
                ]);
            }
            lines.push("  }".to_owned());
        }
        lines.push("}".to_owned());
    }
    if delegated && !product {
        lines.extend([format!("{visibility} sealed class PluginContext(Harmos.Plugins.Context context)"), "{".to_owned()]);
        lines.extend(["  public HostRoot Host { get; } = new(context);", "}"].map(str::to_owned));
    }
    format!("{}\n", lines.join("\n"))
}

/// Run the caller's protoc once per selected file below `scratch` and project its output onto the public payload contract.
fn wire(scratch: &Path, descriptors: &[PathBuf], protoc: &Path, plugin: &Path, generation: &Generation) -> Result<Vec<(PathBuf, String)>> {
    let set = env::join_paths(descriptors)
        .map_err(|error| Error::Input(format!("Compile C# Wire: {error}; Supply Descriptor Paths Without the Path List Separator")))?;
    let mut requests: BTreeSet<String> = generation
        .schema
        .request_names()?
        .iter()
        .map(|name| name.rsplit_once('.').map_or(name.as_str(), |(_, name)| name).to_owned())
        .collect();
    requests.extend(CARRIERS.iter().map(|name| (*name).to_owned()));
    let mut sources = Vec::new();
    for file in generation.schema.files() {
        let package = file.package();
        let owned = if generation.application {
            !package.starts_with("harmos.") && !package.starts_with("google.")
        } else {
            package.starts_with(&format!("{}.", generation.namespace))
        };
        if !owned || file.name().starts_with("google/") || file.name().ends_with("guest.proto") {
            continue;
        }
        let stem = file.name().strip_suffix(".proto").unwrap_or(file.name());
        let folder = scratch.join(stem);
        fs::create_dir_all(&folder).map_err(failure("Create Wire Folder", &folder))?;
        let mut command = Command::new(protoc);
        command.arg(flag("--descriptor_set_in=", &set)).arg(flag("--csharp_out=", &folder));
        if !file.service.is_empty() {
            command.arg(flag("--plugin=protoc-gen-grpc=", plugin)).arg(flag("--grpc_out=", &folder)).arg("--grpc_opt=internal_access");
        }
        let status = command.arg(file.name()).status().map_err(failure("Run Protoc", protoc))?;
        if !status.success() {
            return Err(Error::Input(format!(
                "Compile C# Wire {}: protoc {status}; Correct the Descriptors or Supply the Pinned Compiler",
                file.name()
            )));
        }
        let entries = fs::read_dir(&folder).map_err(failure("Read Wire Folder", &folder))?;
        let mut paths: Vec<PathBuf> = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<_>>()
            .map_err(failure("Read Wire Folder", &folder))?;
        paths.retain(|path| path.extension().is_some_and(|extension| extension == "cs"));
        paths.sort();
        for path in paths {
            let source = fs::read_to_string(&path).map_err(failure("Read Wire Source", &path))?;
            let filename = path.file_name().and_then(OsStr::to_str).expect("protoc names its outputs after the protocol file");
            sources.push((Path::new("Wire").join(stem).join(filename), wire_source(&source, filename, file, &requests)));
        }
    }
    Ok(sources)
}

/// Apply the public payload and internal carrier namespace contract to one protoc output.
fn wire_source(source: &str, filename: &str, file: &FileDescriptorProto, requests: &BTreeSet<String>) -> String {
    let mut source = message(source);
    if !file.package().starts_with("harmos.host.") {
        for name in requests {
            source = source.replace(&format!("public sealed partial class {name} :"), &format!("internal sealed partial class {name} :"));
        }
    }
    if filename.ends_with("Grpc.cs") {
        source = source
            .replace("namespace Harmos.Plugins {", "namespace Harmos.Internal.Host {")
            .replace("namespace Harmos.Clients {", "namespace Harmos.Internal.Client {");
        let namespace = message(file.options.as_ref().map_or("", |options| options.csharp_namespace()));
        if let Some(root) = namespace.strip_suffix(".Clients") {
            source = source.replace(&format!("namespace {namespace} {{"), &format!("namespace {root}.Internal.Client {{"));
        }
    }
    if file.package() == "harmos.plugin.v1" {
        source = source.replace("public sealed partial class ", "internal sealed partial class ");
    }

    // Generated Namespaces Follow the Same File-Scoped Rule as Authored C#
    let declaration = source.match_indices("namespace ").find_map(|(index, keyword)| {
        let start = index + keyword.len();
        let length = source[start..]
            .find(|character: char| !(character.is_alphanumeric() || character == '_' || character == '.'))
            .unwrap_or(source.len() - start);
        (length > 0 && source[start + length..].starts_with(" {"))
            .then(|| (index, source[start..start + length].to_owned(), start + length + 2))
    });
    if let Some((start, name, end)) = declaration {
        source.replace_range(start..end, &format!("namespace {name};"));
        if let Some(ending) = source.rfind('}') {
            source.remove(ending);
        }
    }
    source
}

/// The public C# name of a protocol namespace or type.
fn message(name: &str) -> String {
    name.replace("Harmos.Plugin.V1", "Harmos.Internal.Plugin")
        .replace("Harmos.Host.V1.StatusResources", "Harmos.Clients")
        .replace("Harmos.Host.V1.PluginsResources", "Harmos.Clients")
        .replace("Harmos.Host.V1.AccessResources", "Harmos.Clients")
        .replace("Harmos.Host.V1", "Harmos.Clients")
        .replace("Harmos.Model.V1", "Harmos")
}

/// Generated code names every protocol type from the global namespace, so imported namespaces never collide.
fn qualified(csharp: &str) -> String {
    format!("global::{}", message(csharp))
}

fn field_type(field: &Field) -> String {
    let value = if field.csharp.is_empty() { scalar(field.kind).to_owned() } else { qualified(&field.csharp) };
    if field.map {
        format!("IReadOnlyDictionary<string, {value}>")
    } else if field.repeated {
        format!("IReadOnlyList<{value}>")
    } else {
        value
    }
}

fn scalar(kind: i32) -> &'static str {
    match kind {
        1 => "double",
        2 => "float",
        4 | 6 => "ulong",
        5 | 15 | 17 => "int",
        7 | 13 => "uint",
        8 => "bool",
        9 => "string",
        12 => "ByteString",
        _ => "long",
    }
}

fn joined(path: &[String]) -> String {
    path.iter().map(|part| camel(part)).collect()
}

/// A client parameter: the camel-cased field name with a lower-case first letter.
fn parameter(name: &str) -> String {
    let value = camel(name);
    let mut characters = value.chars();
    let first = characters.next().expect("protocol field names are not empty");
    identifier(&first.to_lowercase().chain(characters).collect::<String>())
}

fn identifier(name: &str) -> String {
    if KEYWORDS.contains(&name) { format!("@{name}") } else { name.to_owned() }
}

/* === Tests === */
