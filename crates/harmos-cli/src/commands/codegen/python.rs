/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Python SDK Bindings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

// Third Party Imports
use prost_types::{DescriptorProto, FileDescriptorProto};

// Local Imports
use crate::commands::codegen::{Generation, Inputs, Output, failure, flag};
use crate::descriptor::{Field, Method, camel};
use crate::error::{Error, Result};

/* === Constants === */

const HEADER: &str = "# @generated from protocol descriptors; do not edit.";

const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except",
    "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
    "while", "with", "yield",
];

/// Names generated modules bind themselves; payloads with these names stay module-qualified.
const BOUND: &[&str] = &[
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
];

const SUFFIXES: [&str; 3] = ["_pb2.py", "_pb2.pyi", "_pb2_grpc.py"];

const SETTINGS_LIST: &str = "harmos.host.v1.plugins.SettingsListRequest";

/* === Macros === */

/* === Type Aliases === */

/// Protocol types mapped to their defining module and dotted name inside it.
type Modules = BTreeMap<String, (String, String)>;

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(flatten)]
    inputs: Inputs,

    /// Import the protocol modules of files below `ROOT/` from `MODULE`, as `ROOT=MODULE` (repeatable) [default: harmos=harmos._generated.proto].
    #[arg(long = "module", value_name = "ROOT=MODULE")]
    modules: Vec<String>,

    /// Compile the owned protocol files into `proto/` with this protoc program.
    #[arg(long, value_name = "PROGRAM")]
    protoc: Option<PathBuf>,

    /// Pass this argument to the protoc program before the generated ones (repeatable).
    #[arg(long = "protoc-arg", value_name = "ARGUMENT", allow_hyphen_values = true, requires = "protoc")]
    protoc_args: Vec<String>,

    /// Import root holding the `.proto` sources (repeatable).
    #[arg(long = "proto-path", value_name = "DIRECTORY", requires = "protoc")]
    proto_paths: Vec<PathBuf>,

    /// Declare the generated message stubs `@final` over the protobuf metaclass.
    #[arg(long, requires = "protoc")]
    final_stubs: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    let generation = args.inputs.load()?;
    let mut roots = vec![("harmos".to_owned(), "harmos._generated.proto".to_owned())];
    for value in &args.modules {
        let Some((root, module)) = value.split_once('=') else {
            return Err(Error::Input(format!("Generate Python SDK: Write --module {value} as ROOT=MODULE")));
        };
        roots.retain(|(existing, _)| existing != root);
        roots.push((root.to_owned(), module.to_owned()));
    }
    roots.sort_by_key(|(root, _)| std::cmp::Reverse(root.len()));
    let modules = message_types(generation.schema.files(), &roots);

    // Render Typed Calls
    let mut output = Output::default();
    if !generation.clients.is_empty() {
        output.add("client.py", &client(&generation.clients, &modules, &generation.namespace)?);
    }
    let host = host(&generation.contexts, &modules, &roots, generation.application);
    output.add(if generation.application { "plugin.py" } else { "host.py" }, &host);

    // Compile Owned Wire Modules
    if let Some(protoc) = &args.protoc {
        let scratch = env::temp_dir().join(format!("harmos-python-{}", process::id()));
        let compiled = wire(&scratch, protoc, &args, &generation, &roots);
        fs::remove_dir_all(&scratch).ok();
        for (path, source) in compiled? {
            output.add_source(path, source);
        }
    }
    output.write(args.inputs.out())
}

/* === Private Functions === */

/// Typed client resources: content arguments and exactly-one-field response projection.
fn client(methods: &[Method], modules: &Modules, namespace: &str) -> Result<Vec<String>> {
    let methods: Vec<Method> = methods
        .iter()
        .cloned()
        .map(|mut method| {
            if method.request == SETTINGS_LIST {
                for field in method.fields.iter_mut().filter(|field| field.name == "name") {
                    field.optional = false;
                }
            }
            method
        })
        .collect();
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::from([Vec::new()]);
    let mut leaves = BTreeSet::new();
    for method in &methods {
        if !leaves.insert(method.path.clone()) {
            return Err(Error::Contract(format!(
                "Generate Python Client {}: Invalid or Duplicate Resource; Correct the source protocol declarations or explicit public exports before regenerating",
                method.path.join(".")
            )));
        }
        if method.client_streaming {
            return Err(Error::Contract(format!(
                "Generate Python Client {}: Client Streaming Requires a Managed Contract; Correct the source protocol declarations or explicit public exports before regenerating",
                method.method
            )));
        }
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    if leaves.iter().any(|leaf| nodes.contains(leaf)) {
        return Err(Error::Contract(
            "Generate Python Client: Resource Collides With a Method; Correct the source protocol declarations or explicit public exports before regenerating".into(),
        ));
    }
    let names: BTreeMap<Vec<String>, String> = nodes
        .iter()
        .map(|node| {
            (
                node.clone(),
                if node.is_empty() {
                    "Client".to_owned()
                } else {
                    format!("{}Resource", node.iter().map(|part| camel(part)).collect::<String>())
                },
            )
        })
        .collect();
    let mut used = BTreeSet::new();
    for method in &methods {
        used.extend([method.request.clone(), method.response.clone()]);
        used.extend(method.typed_fields().filter(|field| !field.type_name.is_empty()).map(|field| field.type_name.clone()));
    }
    let (imports, message_names) = message_imports(modules, &used, &names.values().cloned().collect());
    let field_type = |field: &Field| {
        let value = value_type(field, &message_names);
        if field.map {
            format!("dict[str, {value}]")
        } else if field.repeated {
            format!("Sequence[{value}]")
        } else {
            value
        }
    };

    let mut lines = [
        HEADER,
        "\"\"\"Typed Client Resources.\"\"\"",
        "from collections.abc import AsyncGenerator, Sequence",
        "from uuid import uuid4",
        "from grpc import CallCredentials",
        "from grpc.aio import AioRpcError, Channel",
        "from harmos._client_transport import Metadata, call_error, stream, unary",
        "from harmos import _connection",
    ]
    .map(str::to_owned)
    .to_vec();
    lines.extend(imports);
    let mut ordered: Vec<&Vec<String>> = nodes.iter().collect();
    ordered.sort_by(|first, second| second.len().cmp(&first.len()).then_with(|| first.cmp(second)));
    for node in ordered {
        let children: Vec<&Vec<String>> =
            nodes.iter().filter(|child| child.len() == node.len() + 1 && child[..node.len()] == node[..]).collect();
        let parent = if node.is_empty() { "(_connection.Connection)" } else { "" };
        lines.extend([
            String::new(),
            format!("class {}{parent}:", names[node]),
            "    def __init__(self, channel: Channel) -> None:".to_owned(),
        ]);
        if node.is_empty() {
            lines.push("        super().__init__(channel)".to_owned());
        }
        lines.push("        self._channel = channel".to_owned());
        for child in &children {
            lines.push(format!("        self._resource_{} = {}(channel)", identifier(child.last().expect("child")), names[*child]));
        }
        for child in &children {
            let name = identifier(child.last().expect("child"));
            lines.extend([
                String::new(),
                "    @property".to_owned(),
                format!("    def {name}(self) -> {}:", names[*child]),
                format!("        return self._resource_{name}"),
            ]);
        }
        for method in methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]) {
            let request = &message_names[&method.request];
            let response = &message_names[&method.response];
            let mut parameters: Vec<String> = method
                .fields
                .iter()
                .filter(|field| !field.optional)
                .map(|field| format!("{}: {}", identifier(&field.name), field_type(field)))
                .collect();
            parameters.push("*".to_owned());
            parameters.extend(
                method
                    .fields
                    .iter()
                    .filter(|field| field.optional)
                    .map(|field| format!("{}: {} | None = None", identifier(&field.name), field_type(field))),
            );
            let mut occupied: BTreeSet<String> = method.fields.iter().map(|field| identifier(&field.name)).collect();
            occupied.insert("self".to_owned());
            let mut transport = BTreeMap::new();
            for name in ["metadata", "timeout", "credentials"] {
                let mut parameter = if namespace != "harmos" && name == "metadata" { format!("call_{name}") } else { name.to_owned() };
                while occupied.contains(&parameter) {
                    parameter = format!("call_{parameter}");
                }
                occupied.insert(parameter.clone());
                transport.insert(name, parameter);
            }
            parameters.push(format!("{}: Metadata | None = None", transport["metadata"]));
            parameters.push(format!("{}: float | None = None", transport["timeout"]));
            parameters.push(format!("{}: CallCredentials | None = None", transport["credentials"]));
            let result = method.payload_field.as_ref().map_or_else(|| response.clone(), field_type).replace("Sequence[", "list[");
            let returns = if method.server_streaming { format!("AsyncGenerator[{result}]") } else { result };
            lines.push(String::new());
            lines.push(format!(
                "    async def {}(self, {}) -> {returns}:",
                identifier(method.path.last().expect("call path")),
                parameters.join(", ")
            ));
            let values: Vec<String> = method
                .fields
                .iter()
                .map(|field| {
                    let value = if field.name == "request_id" {
                        "request_id if request_id is not None else str(uuid4())".to_owned()
                    } else {
                        identifier(&field.name)
                    };
                    if KEYWORDS.contains(&field.name.as_str()) {
                        format!("**{{'{}': {value}}}", field.name)
                    } else {
                        format!("{}={value}", field.name)
                    }
                })
                .collect();
            let groups: BTreeSet<&str> =
                method.fields.iter().filter(|field| !field.oneof.is_empty()).map(|field| field.oneof.as_str()).collect();
            for group in groups {
                let variants = method
                    .fields
                    .iter()
                    .filter(|field| field.oneof == group)
                    .map(|field| identifier(&field.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("        if sum(value is not None for value in ({variants},)) > 1:"));
                lines.push(format!("            raise ValueError(\"Choose At Most One {group} Variant\")"));
            }
            lines.push(format!("        request = {request}({})", values.join(", ")));
            let path = format!("/{}.{}/{}", method.package, method.service, method.method);
            let action = method.path.join(".");
            let call = format!(
                "{}(self._channel, '{path}', request, {response}, metadata={}, timeout={}, credentials={}",
                if method.server_streaming { "stream" } else { "unary" },
                transport["metadata"],
                transport["timeout"],
                transport["credentials"]
            );
            let payload = method.payload_field.as_ref();
            let value = payload.map_or_else(
                || "response".to_owned(),
                |payload| if payload.repeated { format!("list(response.{})", payload.name) } else { format!("response.{}", payload.name) },
            );
            let checked = payload.filter(|payload| payload.kind == 11 && !payload.repeated);
            if method.server_streaming {
                lines.extend([
                    format!("        call = {call})"),
                    "        try:".to_owned(),
                    "            async for response in call:".to_owned(),
                ]);
                if let Some(payload) = checked {
                    lines.push(format!("                if not response.HasField(\"{}\"):", payload.name));
                    lines.push(format!(
                        "                    raise ValueError(\"Read {}.{}: Missing Payload\")",
                        method.service, method.method
                    ));
                }
                lines.extend([
                    format!("                yield {value}"),
                    "        except AioRpcError as error:".to_owned(),
                    format!("            raise call_error('{action}', error) from error"),
                    "        finally:".to_owned(),
                    "            call.cancel()".to_owned(),
                ]);
            } else {
                lines.push(format!("        response = await {call}, action='{action}')"));
                if let Some(payload) = checked {
                    lines.push(format!("        if not response.HasField(\"{}\"):", payload.name));
                    lines.push(format!("            raise ValueError(\"Read {}.{}: Missing Payload\")", method.service, method.method));
                }
                lines.push(format!("        return {value}"));
            }
        }
    }
    Ok(lines)
}

/// Typed plugin-to-host calls, their operation inventory and, for applications, a plugin context.
fn host(methods: &[Method], modules: &Modules, roots: &[(String, String)], application: bool) -> Vec<String> {
    let mut used = BTreeSet::new();
    let mut reserved = BTreeSet::new();
    for method in methods {
        used.extend([method.request.clone(), method.response.clone()]);
        used.extend(method.typed_fields().filter(|field| !field.type_name.is_empty()).map(|field| field.type_name.clone()));
        for length in 1..method.path.len() {
            reserved.insert(format!("Host{}Resource", method.path[..length].iter().map(|part| camel(part)).collect::<String>()));
        }
    }
    let (imports, message_names) = message_imports(modules, &used, &reserved);
    let field_type = |field: &Field| {
        let value = value_type(field, &message_names);
        if field.map {
            format!("dict[str, {value}]")
        } else if field.repeated {
            format!("list[{value}]")
        } else {
            value
        }
    };
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::new();
    for method in methods.iter().filter(|method| method.surface) {
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    let names: BTreeMap<&Vec<String>, String> =
        nodes.iter().map(|node| (node, format!("Host{}Resource", node.iter().map(|part| camel(part)).collect::<String>()))).collect();
    let constant = |method: &Method| method.path.join("_").to_uppercase();

    // Render the Operation Inventory
    let mut lines = vec![
        HEADER.to_owned(),
        "from collections.abc import AsyncGenerator".to_owned(),
        "from enum import Enum".to_owned(),
        "from uuid import uuid4".to_owned(),
        "from typing import TYPE_CHECKING, cast".to_owned(),
        "from google.protobuf.message import Message".to_owned(),
        "from harmos._contracts import Operation".to_owned(),
        format!("from {} import VISIBILITY_PUBLIC", module("harmos/model/v1/operation.proto", roots)),
        format!("from {} import OperationPolicy", module("harmos/model/v1/policy.proto", roots)),
    ];
    lines.extend(imports);
    lines.extend(["if TYPE_CHECKING:", "    from harmos._invocation import Context", "class HostOperation(Enum):"].map(str::to_owned));
    if methods.is_empty() {
        lines.push("    pass".to_owned());
    }
    for method in methods {
        lines.push(format!("    {} = \"{}\"", constant(method), method.identity()));
    }
    for method in methods {
        let name = constant(method);
        let watch = if method.server_streaming { "True" } else { "False" };
        lines.push(format!(
            "_{name} = Operation(HostOperation.{name}.value, {}, {}, visibility=VISIBILITY_PUBLIC, watch={watch})",
            message_names[&method.request], message_names[&method.response]
        ));
    }
    lines.push("def host_requirement(operation: HostOperation) -> Operation[Message, Message]:".to_owned());
    if !methods.is_empty() {
        lines.push("    match operation:".to_owned());
    }
    for method in methods {
        let name = constant(method);
        lines.push(format!("        case HostOperation.{name}:"));
        lines.push(format!("            return cast(Operation[Message, Message], _{name})"));
    }
    lines.extend(
        [
            "    raise ValueError(\"Declare Host Requirement: Unknown Operation\")",
            "class HostRoot:",
            "    def __init__(self, context: 'Context') -> None:",
        ]
        .map(str::to_owned),
    );
    for node in nodes.iter().filter(|node| node.len() == 1) {
        lines.push(format!("        self.{} = {}(context)", identifier(&node[0]), names[node]));
    }
    if nodes.is_empty() {
        lines.push("        pass".to_owned());
    }

    // Render Each Resource and Its Calls
    for node in &nodes {
        lines.extend([
            format!("class {}:", names[node]),
            "    def __init__(self, context: 'Context') -> None:".to_owned(),
            "        self._context = context".to_owned(),
        ]);
        for child in nodes.iter().filter(|child| child[..child.len() - 1] == node[..]) {
            lines.push(format!("        self.{} = {}(context)", identifier(child.last().expect("child")), names[child]));
        }
        for method in methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]) {
            let name = constant(method);
            let request = &message_names[&method.request];
            let response = &message_names[&method.response];
            let mut parameters: Vec<String> = method
                .fields
                .iter()
                .filter(|field| !field.optional)
                .map(|field| format!("{}: {}", identifier(&field.name), field_type(field)))
                .collect();
            parameters.push("*".to_owned());
            parameters.extend(
                method
                    .fields
                    .iter()
                    .filter(|field| field.optional)
                    .map(|field| format!("{}: {} | None = None", identifier(&field.name), field_type(field))),
            );
            parameters
                .push(if method.server_streaming { "max_item_bytes: int" } else { "policy: OperationPolicy | None = None" }.to_owned());
            let payload = method.payload_field.as_ref();
            let mut result = payload.map_or_else(|| response.clone(), field_type);
            if method.server_streaming {
                result = format!("AsyncGenerator[{result}]");
            }
            let values: Vec<String> = method
                .fields
                .iter()
                .map(|field| {
                    let value = if field.name == "request_id" {
                        "request_id if request_id is not None else str(uuid4())".to_owned()
                    } else {
                        identifier(&field.name)
                    };
                    if KEYWORDS.contains(&field.name.as_str()) {
                        format!("**{{'{}': {value}}}", field.name)
                    } else {
                        format!("{}={value}", field.name)
                    }
                })
                .collect();
            lines.push(format!(
                "    async def {}(self, {}) -> {result}:",
                identifier(method.path.last().expect("call path")),
                parameters.join(", ")
            ));
            lines.push(format!("        request = {request}({})", values.join(", ")));
            let indent = if method.server_streaming { "                " } else { "        " };
            if method.server_streaming {
                lines.extend([
                    format!("        call = self._context.watch(_{name}, request, max_item_bytes=max_item_bytes)"),
                    "        try:".to_owned(),
                    "            async for response in call:".to_owned(),
                ]);
            } else {
                lines.push(format!("        response = await self._context._call(_{name}, request, policy=policy)"));
            }
            if let Some(payload) = payload.filter(|payload| payload.kind == 11 && !payload.repeated) {
                lines.push(format!("{indent}if not response.HasField(\"{}\"):", payload.name));
                lines.push(format!("{indent}    raise ValueError(\"Read {name}: Missing Payload\")"));
            }
            let value = payload.map_or_else(
                || "response".to_owned(),
                |payload| if payload.repeated { format!("list(response.{})", payload.name) } else { format!("response.{}", payload.name) },
            );
            lines.push(format!("{indent}{} {value}", if method.server_streaming { "yield" } else { "return" }));
            if method.server_streaming {
                lines.extend(["        finally:", "            await call.aclose()"].map(str::to_owned));
            }
        }
    }
    if application {
        lines.extend(
            ["class PluginContext:", "    def __init__(self, context: \"Context\") -> None:", "        self.host = HostRoot(context)"]
                .map(str::to_owned),
        );
    }
    lines
}

/// Run the caller's protoc over the owned protocol files and map their imports onto the generated modules.
fn wire(scratch: &Path, protoc: &Path, args: &Args, generation: &Generation, roots: &[(String, String)]) -> Result<Vec<(PathBuf, String)>> {
    let owned = |file: &&FileDescriptorProto| {
        let name = file.name();
        if generation.application {
            !name.starts_with("harmos/") && !name.starts_with("google/")
        } else {
            name.starts_with(&format!("{}/", generation.namespace))
        }
    };
    let files: Vec<&str> = generation.schema.files().iter().filter(owned).map(FileDescriptorProto::name).collect();
    fs::create_dir_all(scratch).map_err(failure("Create Wire Folder", scratch))?;
    let mut command = Command::new(protoc);
    command.args(&args.protoc_args);
    for path in &args.proto_paths {
        command.arg(flag("-I", path));
    }
    let status = command
        .arg(flag("--python_out=", scratch))
        .arg(flag("--pyi_out=", scratch))
        .arg(flag("--grpc_python_out=", scratch))
        .args(&files)
        .status()
        .map_err(failure("Run Protoc", protoc))?;
    if !status.success() {
        return Err(Error::Input(format!(
            "Compile Python Wire: protoc {status}; Correct the Protocol Sources or Supply the Pinned Compiler"
        )));
    }
    let mut compiled = Vec::new();
    for name in files {
        let stem = name.strip_suffix(".proto").unwrap_or(name);
        let relative = stem.split_once('/').map_or(stem, |(_, relative)| relative);
        for suffix in SUFFIXES {
            let path = scratch.join(format!("{stem}{suffix}"));
            let mut source = fs::read_to_string(&path).map_err(failure("Read Wire Module", &path))?;
            for (root, module) in roots {
                source = source.replace(&format!("from {root}."), &format!("from {module}."));
                source = source.replace(&format!("import {root}."), &format!("import {module}."));
                source = source.replace(&format!("from {root} import "), &format!("from {module} import "));
                source = source.replace(
                    &format!("BuildTopDescriptorsAndMessages(DESCRIPTOR, '{root}."),
                    &format!("BuildTopDescriptorsAndMessages(DESCRIPTOR, '{module}."),
                );
            }
            if suffix == "_pb2_grpc.py" && source.contains("grpc.experimental.") {
                source = source.replacen("import grpc\n", "import grpc\nimport grpc.experimental\n", 1);
            }
            if suffix == "_pb2.pyi" && args.final_stubs {
                source = final_stubs(&source);
            }
            compiled.push((Path::new("proto").join(format!("{relative}{suffix}")), source));
        }
    }
    Ok(compiled)
}

/// Fields are managed by the protobuf metaclass, not Python slots.
fn final_stubs(source: &str) -> String {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut result = String::from("from google.protobuf.reflection import GeneratedProtocolMessageType\nfrom typing import final\n\n");
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let indent = &line[..line.len() - line.trim_start_matches(' ').len()];
        let declaration = line[indent.len()..].strip_prefix("class ").and_then(|rest| rest.strip_suffix("(_message.Message):\n"));
        let named =
            declaration.filter(|name| !name.is_empty() && name.chars().all(|character| character.is_alphanumeric() || character == '_'));
        let slots = lines
            .get(index + 1)
            .is_some_and(|next| next.starts_with(' ') && next.trim_start_matches(' ').starts_with("__slots__ = ") && next.ends_with('\n'));
        if let Some(name) = named.filter(|_| slots) {
            result.push_str(&format!("{indent}@final\n{indent}class {name}(_message.Message, metaclass=GeneratedProtocolMessageType):\n{indent}    __slots__: tuple[str, ...]\n"));
            index += 2;
            continue;
        }
        result.push_str(line);
        index += 1;
    }
    result
}

/// Each protocol type with its defining module after the root mapping.
fn message_types(files: &[FileDescriptorProto], roots: &[(String, String)]) -> Modules {
    let mut types = Modules::new();
    for file in files {
        let module = module(file.name(), roots);
        record(&file.message_type, file.package(), &module, "", &mut types);
        for enumeration in &file.enum_type {
            types.insert(format!("{}.{}", file.package(), enumeration.name()), (module.clone(), enumeration.name().to_owned()));
        }
    }
    types
}

fn record(messages: &[DescriptorProto], package: &str, module: &str, parent: &str, types: &mut Modules) {
    for message in messages {
        let name = format!("{parent}{}", message.name());
        types.insert(format!("{package}.{name}"), (module.to_owned(), name.clone()));
        for enumeration in &message.enum_type {
            types.insert(format!("{package}.{name}.{}", enumeration.name()), (module.to_owned(), format!("{name}.{}", enumeration.name())));
        }
        record(&message.nested_type, package, module, &format!("{name}."), types);
    }
}

/// The Python module of a protocol file, `root/path/name.proto` to `MODULE.path.name_pb2`.
fn module(file: &str, roots: &[(String, String)]) -> String {
    let dotted = format!("{}_pb2", file.strip_suffix(".proto").unwrap_or(file).replace('/', "."));
    for (root, module) in roots {
        if let Some(rest) = dotted.strip_prefix(root.as_str()).filter(|rest| rest.starts_with('.')) {
            return format!("{module}{rest}");
        }
    }
    dotted
}

/// Import each used type by name unless its name clashes; clashes import the module and stay qualified.
fn message_imports(types: &Modules, used: &BTreeSet<String>, reserved: &BTreeSet<String>) -> (Vec<String>, BTreeMap<String, String>) {
    let head = |name: &str| name.split_once('.').map_or(name, |(head, _)| head).to_owned();
    let mut modules: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for full in used {
        let (module, name) = &types[full];
        modules.entry(head(name)).or_default().insert(module);
    }
    let mut duplicates: BTreeSet<String> = modules.iter().filter(|(_, values)| values.len() > 1).map(|(name, _)| name.clone()).collect();
    duplicates.extend(reserved.iter().cloned());
    duplicates.extend(BOUND.iter().map(|name| (*name).to_owned()));
    let mut imports = BTreeSet::new();
    let mut names = BTreeMap::new();
    for full in used {
        let (module, name) = &types[full];
        if duplicates.contains(&head(name)) {
            names.insert(full.clone(), format!("{module}.{name}"));
            imports.insert(format!("import {module}"));
        } else {
            names.insert(full.clone(), name.clone());
            imports.insert(format!("from {module} import {}", head(name)));
        }
    }
    (imports.into_iter().collect(), names)
}

fn value_type(field: &Field, message_names: &BTreeMap<String, String>) -> String {
    if !field.type_name.is_empty() {
        return message_names[&field.type_name].clone();
    }
    match field.kind {
        1 | 2 => "float",
        8 => "bool",
        9 => "str",
        12 => "bytes",
        _ => "int",
    }
    .to_owned()
}

fn identifier(name: &str) -> String {
    if KEYWORDS.contains(&name) { format!("{name}_") } else { name.to_owned() }
}

/* === Tests === */
