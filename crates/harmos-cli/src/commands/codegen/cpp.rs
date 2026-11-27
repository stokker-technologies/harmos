/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate C++ SDK Bindings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports

// Local Imports
use crate::commands::codegen::{Generation, Inputs, Output, identifier_character, replace_word};
use crate::descriptor::{Field, Method, camel, is_version, snake};
use crate::error::{Error, Result};

/* === Constants === */

const HEADER: &str = "// @generated from protocol descriptors; do not edit.";

/// C++ keywords, separated by spaces; identifiers that match one take a trailing underscore.
const KEYWORDS: &str = concat!(
    "alignas alignof and and_eq asm atomic_cancel atomic_commit atomic_noexcept auto bitand bitor bool break case catch char char16_t ",
    "char32_t char8_t class co_await co_return co_yield compl concept const const_cast consteval constexpr constinit continue ",
    "decltype default delete do double dynamic_cast else enum explicit export extern false float for friend goto if import inline int ",
    "long module mutable namespace new noexcept not not_eq nullptr operator or or_eq private protected public reflexpr register ",
    "reinterpret_cast requires return short signed sizeof static static_assert static_cast struct switch synchronized template this ",
    "thread_local throw true try typedef typeid typename union unsigned using virtual void volatile wchar_t while xor xor_eq",
);

/// Host call names that take a trailing underscore.
const RESERVED_CALLS: &[&str] = &["delete", "export", "import", "new", "class", "default"];

/// Locals a generated call body declares; parameters avoid them.
const LOCALS: &[&str] =
    &["options", "max_item_bytes", "request", "response", "context", "operation", "incoming", "reader", "status", "key", "item"];

/// Envelope messages that never become public payloads.
const CARRIERS: &[&str] =
    &["Request", "Response", "ChildRequest", "ChildResponse", "ChildCancel", "ChildWatch", "ChildWatchResponse", "ChildWatchCancel"];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(flatten)]
    inputs: Inputs,

    /// C++ namespace of the application's client and plugin context; defaults to --namespace.
    #[arg(long, value_name = "NAMESPACE", requires = "application")]
    cpp_namespace: Option<String>,

    /// Name the client root `detail::Transport` and include `<namespace>/detail/transport.hpp`, for SDKs that wrap the transport.
    #[arg(long)]
    client_transport: bool,

    /// Raise failed client call statuses through this function, called as `function("action", status, *context)`.
    #[arg(long, value_name = "FUNCTION")]
    rpc_error: Option<String>,

    /// Leave this payload out of the public `messages.hpp`, named as that barrel re-exports it, such as `Suite` or `client::Name` (repeatable).
    #[arg(long = "omit-export", value_name = "NAME", conflicts_with = "application")]
    omitted_exports: Vec<String>,

    /// Place host resources in `<namespace>::<SEGMENT>` instead of `<namespace>::plugin` and define `Registrar::add` in `<namespace>`.
    #[arg(long, value_name = "SEGMENT", conflicts_with = "application")]
    host_namespace: Option<String>,
}

/// One payload or enumeration constant re-exported from the private payload namespace.
struct Export {
    boundary: &'static str,
    name: String,
    qualified: String,
    constant: bool,
}

/// The public and private payload barrels and the C++ name of every exported payload.
struct Exports {
    public: Vec<String>,
    private: Vec<String>,
    type_names: BTreeMap<String, String>,
}

/// The client resources, their private wire header and, for applications, the response re-exports.
struct ClientSources {
    lines: Vec<String>,
    wire: Vec<String>,
    messages: Option<Vec<String>>,
}

/// The rendered host header and source with the number of root resources.
struct HostSources {
    header: Vec<String>,
    source: Vec<String>,
    roots: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    let generation = args.inputs.load()?;
    let namespace = generation.namespace.as_str();
    let mut output = Output::default();
    if generation.application {
        let code = args.cpp_namespace.as_deref().unwrap_or(namespace);
        let ClientSources { lines, wire, messages } = client(&generation, &BTreeMap::new(), code, &args)?;
        output.add("client.hpp", &lines);
        output.add("detail/client.hpp", &wire);
        if let Some(messages) = messages {
            output.add("client_messages.hpp", &messages);
        }
        let (header, source) = application_host(host(&generation.contexts, &BTreeMap::new(), namespace, None), namespace, code);
        output.add_source("plugin.hpp", header);
        output.add_source("plugin.cpp", source);
        return output.write(args.inputs.out());
    }
    let Exports { public, private, type_names } = exports(&generation, &args.omitted_exports)?;
    output.add(format!("include/{namespace}/messages.hpp"), &public);
    output.add(format!("include/{namespace}/detail/messages.hpp"), &private);
    if !generation.clients.is_empty() {
        let ClientSources { lines, wire, .. } = client(&generation, &type_names, namespace, &args)?;
        output.add(format!("include/{namespace}/_generated/client.hpp"), &lines);
        output.add(format!("include/{namespace}/_generated/detail/client.hpp"), &wire);
    }
    let rendered = host(&generation.contexts, &type_names, namespace, args.host_namespace.as_deref());
    output.add(format!("include/{namespace}/_generated/host.hpp"), &rendered.header);
    output.add("src/generated/host.cpp", &rendered.source);
    output.write(args.inputs.out())
}

/* === Private Functions === */

/// Render the payload barrels and name every exported payload.
fn exports(generation: &Generation, omitted: &[String]) -> Result<Exports> {
    let schema = &generation.schema;
    let namespace = generation.namespace.as_str();
    let public_paths = schema.public_paths(namespace, &BTreeSet::new());
    let audiences = schema.payload_audiences(Some(&generation.library))?;
    let requests = schema.request_names()?;
    let client_only = BTreeSet::from(["client"]);
    let mut type_names = BTreeMap::new();
    let mut private = vec![HEADER.to_owned(), "#pragma once".to_owned()];
    let mut public = vec![HEADER.to_owned(), "#pragma once".to_owned(), "#include \"detail/messages.hpp\"".to_owned()];

    // Collect Payloads and Enumeration Constants per Boundary
    let mut entries = Vec::new();
    for file in schema.files() {
        let package = file.package();
        let guest = file.name().ends_with("guest.proto");
        let owned = package.starts_with(&format!("{namespace}."));
        if guest || !owned || package.contains(".plugin.") {
            continue;
        }
        private.push(format!("#include \"{}.pb.h\"", file.name().strip_suffix(".proto").unwrap_or(file.name())));
        let wire = format!("::{}", package.replace('.', "::"));
        let boundary_of = |full: &str| {
            let reached = audiences.get(full).unwrap_or(&client_only);
            if package.contains(".model.") || reached.len() > 1 { "" } else { *reached.first().expect("payload audiences name a face") }
        };
        let messages = file.message_type.iter().map(|message| (message.name(), Some(message)));
        let enumerations = file.enum_type.iter().map(|enumeration| (enumeration.name(), None));
        for (definition, message_type) in messages.chain(enumerations) {
            let full = format!("{package}.{definition}");
            let boundary = boundary_of(&full);
            if requests.contains(&full) || CARRIERS.contains(&definition) {
                continue;
            }
            let name = public_paths[&full].replace('.', "::");
            let scope = if boundary.is_empty() { String::new() } else { format!("{boundary}::") };
            type_names.insert(full.clone(), format!("::{namespace}::{scope}{name}"));
            entries.push(Export {
                boundary,
                name,
                qualified: format!("{wire}::{definition}"),
                constant: false,
            });
            let Some(message_type) = message_type else {
                continue;
            };
            let mut pending = vec![(full.clone(), type_names[&full].clone(), message_type)];
            while let Some((full, parent, message)) = pending.pop() {
                for enumeration in &message.enum_type {
                    type_names.insert(format!("{full}.{}", enumeration.name()), format!("{parent}::{}", enumeration.name()));
                }
                for nested in &message.nested_type {
                    type_names.insert(format!("{full}.{}", nested.name()), format!("{parent}::{}", nested.name()));
                    pending.push((format!("{full}.{}", nested.name()), format!("{parent}::{}", nested.name()), nested));
                }
            }
        }
        for enumeration in &file.enum_type {
            let full = format!("{package}.{}", enumeration.name());
            let boundary = boundary_of(&full);
            let scope = public_paths[&full].rsplit_once('.').map_or(String::new(), |(scope, _)| scope.replace('.', "::"));
            for value in &enumeration.value {
                let prefix = snake(enumeration.name()).to_uppercase() + "_";
                let name = if value.name().starts_with(&prefix) { value.name().to_owned() } else { format!("{prefix}{}", value.name()) };
                entries.push(Export {
                    boundary,
                    name: if scope.is_empty() { name } else { format!("{scope}::{name}") },
                    qualified: format!("static_cast<{wire}::{}>({})", enumeration.name(), value.number()),
                    constant: true,
                });
            }
        }
    }

    // Declare Payloads Privately and Re-Export Them per Boundary
    private.extend([
        format!("namespace {namespace}::detail {{"),
        "inline constexpr char stream_fingerprint_prefix[] = \"harmos.protobuf.stream.v1\\0\";".to_owned(),
        "namespace payloads {".to_owned(),
    ]);
    for boundary in ["", "client", "plugin"] {
        if !boundary.is_empty() {
            private.push(format!("namespace {boundary} {{"));
        }
        for entry in entries.iter().filter(|entry| entry.boundary == boundary) {
            let (prefix, name) = entry.name.rsplit_once("::").unwrap_or(("", entry.name.as_str()));
            let declaration = if entry.constant {
                format!("inline constexpr auto {name} = {};", entry.qualified)
            } else {
                format!("using {};", entry.qualified)
            };
            private.push(if prefix.is_empty() { declaration } else { format!("namespace {prefix} {{ {declaration} }}") });
        }
        if !boundary.is_empty() {
            private.push("}".to_owned());
        }
    }
    private.extend(["}", "}"].map(str::to_owned));
    for boundary in ["", "client", "plugin"] {
        let scope = if boundary.is_empty() { String::new() } else { format!("{boundary}::") };
        public.push(format!("namespace {namespace}{} {{", if boundary.is_empty() { String::new() } else { format!("::{boundary}") }));
        for entry in entries.iter().filter(|entry| entry.boundary == boundary) {
            let qualified = format!("{scope}{}", entry.name);
            if omitted.contains(&qualified) {
                continue;
            }
            let prefix = entry.name.rsplit_once("::").map_or("", |(prefix, _)| prefix);
            let declaration = format!("using ::{namespace}::detail::payloads::{qualified};");
            public.push(if prefix.is_empty() { declaration } else { format!("namespace {prefix} {{ {declaration} }}") });
        }
        public.push("}".to_owned());
    }
    Ok(Exports { public, private, type_names })
}

/// The host resources over the plugin context and the operation registrar.
fn host(methods: &[Method], type_names: &BTreeMap<String, String>, namespace: &str, segment: Option<&str>) -> HostSources {
    let scope = format!("{namespace}::{}", segment.unwrap_or("plugin"));
    let field_type = |field: &Field| field_type(field, type_names);
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::new();
    for method in methods.iter().filter(|method| method.surface) {
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    let names: BTreeMap<Vec<String>, String> = nodes
        .iter()
        .map(|node| (node.clone(), format!("Host{}Resource", node.iter().map(|part| camel(part)).collect::<String>())))
        .collect();
    let operation_names: Vec<String> = methods.iter().map(|method| method.path.iter().map(|part| camel(part)).collect()).collect();
    let mut header =
        vec![HEADER.to_owned(), "#pragma once".to_owned(), format!("namespace {scope} {{"), "enum class HostOperation {".to_owned()];
    header.extend(operation_names.iter().map(|name| format!("  {name},")));
    header.push("};".to_owned());
    let mut source = vec![HEADER.to_owned(), format!("#include \"{namespace}/{namespace}.hpp\"")];
    let files: BTreeSet<&str> = methods.iter().map(|method| method.file.as_str()).collect();
    source.extend(files.iter().map(|file| format!("#include \"{}.pb.h\"", file.strip_suffix(".proto").unwrap_or(file))));

    // Import Unambiguous Wire Types; Clashes Keep Their Qualified Path
    let used: BTreeSet<&str> = methods.iter().flat_map(|method| [method.request.as_str(), method.response.as_str()]).collect();
    let mut imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reserved: BTreeSet<String> =
        ["Context", "Plugin", "Operation", "Watching", "HostRoot", "HostOperation"].map(str::to_owned).into();
    reserved.extend(names.values().cloned());
    for full in &used {
        let (_, container) = type_import(&format!("::{}", full.replace('.', "::")));
        imports.entry(last_segment(&container).to_owned()).or_default().insert(container);
    }
    let clashes = |imports: &BTreeMap<String, BTreeSet<String>>| {
        let mut clashes = reserved.clone();
        clashes.extend(imports.iter().filter(|(_, containers)| containers.len() > 1).map(|(name, _)| name.clone()));
        clashes
    };
    let clashing = clashes(&imports);
    let wire_names: BTreeMap<&str, String> = used
        .iter()
        .map(|full| {
            let qualified = format!("::{}", full.replace('.', "::"));
            let (name, container) = type_import(&qualified);
            (*full, if clashing.contains(last_segment(&container)) { qualified } else { name })
        })
        .collect();
    let declared: BTreeSet<String> = imports
        .iter()
        .filter(|(name, _)| !clashing.contains(*name))
        .flat_map(|(_, containers)| containers.iter().map(|container| format!("using {container};")))
        .collect();
    source.extend(declared);
    let boundary = source.len();
    source.push(format!("namespace {scope} {{"));
    if namespace != "harmos" {
        source.extend(
            ["using ::harmos::plugin::Operation;", "using ::harmos::plugin::Watching;", "using ::harmos::VISIBILITY_PUBLIC;"]
                .map(str::to_owned),
        );
    }

    // Render Call Options and Watch Handles
    let policy = if namespace == "harmos" { "OperationPolicy" } else { "::harmos::OperationPolicy" };
    for (method, name) in methods.iter().zip(&operation_names).filter(|(method, _)| method.surface) {
        header.push(format!("struct Host{name}Options {{"));
        for field in method.fields.iter().filter(|field| field.optional) {
            header.push(format!("  std::optional<{}> {};", field_type(field), field.name));
        }
        if !method.server_streaming {
            header.push(format!("  std::optional<{policy}> policy;"));
        }
        header.push("};".to_owned());
        if method.server_streaming {
            let value = method.payload_field.as_ref().map_or_else(|| message(&method.response, type_names), field_type);
            header.extend([
                format!("class Host{name} {{"),
                "public:".to_owned(),
                format!("  explicit Host{name}(std::function<std::optional<{value}>()> next_value) : next_(std::move(next_value)) {{}}"),
                format!("  std::optional<{value}> next() {{ return next_(); }}"),
                "private:".to_owned(),
                format!("  std::function<std::optional<{value}>()> next_;"),
                "};".to_owned(),
            ]);
        }
    }

    // Render Each Resource and Its Calls
    for node in nodes.iter().rev() {
        let children: Vec<&Vec<String>> = nodes.iter().filter(|child| child[..child.len() - 1] == node[..]).collect();
        let operations: Vec<(&Method, &String)> =
            methods.iter().zip(&operation_names).filter(|(method, _)| method.path[..method.path.len() - 1] == node[..]).collect();
        header.extend([format!("struct {} {{", names[node]), "  Context context;".to_owned()]);
        header.extend(children.iter().map(|child| format!("  {} {};", names[*child], identifier(child.last().expect("child")))));
        let mut members: BTreeSet<String> = BTreeSet::from(["context".to_owned()]);
        members.extend(children.iter().map(|child| identifier(child.last().expect("child"))));
        members.extend(operations.iter().map(|(method, _)| identifier(method.path.last().expect("call path"))));
        let context = parameter("context", &members);
        let mut initializers = vec![format!("context({context})")];
        initializers.extend(children.iter().map(|child| format!("{}({context})", identifier(child.last().expect("child")))));
        header.push(format!("  {}(Context {context}) : {} {{}}", names[node], initializers.join(", ")));
        for (method, name) in operations {
            let request = &wire_names[method.request.as_str()];
            let response = &wire_names[method.response.as_str()];
            let payload = method.payload_field.as_ref();
            let value_type = payload.map_or_else(|| message(&method.response, type_names), field_type);
            let result = if method.server_streaming { format!("Host{name}") } else { value_type.clone() };
            let options = parameter("options", &members);
            let maximum = parameter("max_item_bytes", &members);
            let mut occupied = members.clone();
            occupied.extend([options.clone(), maximum.clone()]);
            occupied.extend(LOCALS.iter().map(|local| (*local).to_owned()));
            let mut arguments = BTreeMap::new();
            for field in method.fields.iter().filter(|field| !field.optional) {
                let argument = parameter(&field.name, &occupied);
                occupied.insert(argument.clone());
                arguments.insert(field.name.as_str(), argument);
            }
            let mut parameters: Vec<String> = method
                .fields
                .iter()
                .filter(|field| !field.optional)
                .map(|field| format!("const {}& {}", field_type(field), arguments[field.name.as_str()]))
                .collect();
            if method.server_streaming {
                parameters.push(format!("std::size_t {maximum}"));
            }
            let call = method.path.last().expect("call path");
            let call = if RESERVED_CALLS.contains(&call.as_str()) { format!("{call}_") } else { call.clone() };
            let declaration = [parameters.clone(), vec![format!("Host{name}Options {options} = {{}}")]].concat();
            header.push(format!("  {result} {call}({});", declaration.join(", ")));
            let definition = [parameters, vec![format!("Host{name}Options {options}")]].concat();
            source.extend([format!("{result} {}::{call}({}) {{", names[node], definition.join(", ")), format!("  {request} request;")]);
            for field in &method.fields {
                if field.name == "request_id" {
                    source.push(format!(
                        "  request.set_request_id({options}.request_id ? *{options}.request_id : ::harmos::detail::request_id());"
                    ));
                    continue;
                }
                let member = identifier(&field.name);
                let value = if field.optional { format!("*{options}.{member}") } else { arguments[field.name.as_str()].clone() };
                if field.optional {
                    source.push(format!("  if ({options}.{member}) {{"));
                }
                let assignment = if field.map {
                    format!("for (const auto& [key, item] : {value}) {{ (*request.mutable_{member}())[key] = item; }}")
                } else if field.repeated {
                    format!("request.mutable_{member}()->Assign({value}.begin(), {value}.end());")
                } else if field.kind == 11 {
                    format!("*request.mutable_{member}() = {value};")
                } else {
                    format!("request.set_{member}({value});")
                };
                source.push(format!("    {assignment}"));
                if field.optional {
                    source.push("  }".to_owned());
                }
            }
            source.push(format!("  auto operation = Operation<{request}, {response}>(\"{}\", VISIBILITY_PUBLIC);", method.identity()));
            if method.server_streaming {
                source.extend([
                    format!("  (void){options};"),
                    "  operation.watch();".to_owned(),
                    format!("  auto incoming = std::make_shared<Watching<{response}>>(context.watch(operation, request, {maximum}));"),
                    format!("  return Host{name}([incoming]() -> std::optional<{value_type}> {{"),
                    "    auto response = incoming->next();".to_owned(),
                    "    if (!response) { return {}; }".to_owned(),
                ]);
                let value = match payload {
                    Some(payload) if payload.kind == 11 => format!("std::move(*response->mutable_{}())", payload.name),
                    Some(payload) => format!("response->{}()", payload.name),
                    None => "*response".to_owned(),
                };
                if let Some(payload) = payload.filter(|payload| payload.kind == 11 && !payload.repeated) {
                    source.push(format!(
                        "    if (!response->has_{}()) {{ throw std::runtime_error(\"Read {name}: Missing Payload\"); }}",
                        payload.name
                    ));
                }
                source.extend([format!("    return {value};"), "  });".to_owned()]);
            } else {
                source.push(format!("  auto response = context.operation(operation, request, {options}.policy).get();"));
                let value = match payload {
                    Some(payload) if payload.repeated => {
                        format!("{}(response.{}().begin(), response.{}().end())", field_type(payload), payload.name, payload.name)
                    }
                    Some(payload) if payload.kind == 11 => {
                        source.push(format!(
                            "  if (!response.has_{}()) {{ throw std::runtime_error(\"Read {name}: Missing Payload\"); }}",
                            payload.name
                        ));
                        format!("std::move(*response.mutable_{}())", payload.name)
                    }
                    Some(payload) => format!("response.{}()", payload.name),
                    None => "response".to_owned(),
                };
                source.push(format!("  return {value};"));
            }
            source.push("}".to_owned());
        }
        header.push("};".to_owned());
    }

    // Render the Root and the Operation Registrar
    let roots: Vec<&Vec<String>> = nodes.iter().filter(|node| node.len() == 1).collect();
    header.push("struct HostRoot {".to_owned());
    header.extend(roots.iter().map(|node| format!("  {} {};", names[*node], node[0])));
    header.extend(["};", "}"].map(str::to_owned));
    source.extend([
        "}".to_owned(),
        format!("namespace {scope} {{"),
        "HostRoot Context::host() {".to_owned(),
        format!("  return {{{}}};", vec!["{*this}"; roots.len()].join(", ")),
        "}".to_owned(),
    ]);
    match segment {
        Some(segment) => source.extend([
            "}".to_owned(),
            format!("namespace {namespace} {{"),
            format!("Registrar& Registrar::add({segment}::HostOperation operation) {{"),
            format!("  using {segment}::HostOperation;"),
            "  using ::harmos::VISIBILITY_PUBLIC;".to_owned(),
            "  using ::harmos::plugin::Operation;".to_owned(),
            "  using ::harmos::plugin::Dependency;".to_owned(),
        ]),
        None => source.push("Registrar& Registrar::add(HostOperation operation) {".to_owned()),
    }
    source.push("  switch (operation) {".to_owned());
    for (method, name) in methods.iter().zip(&operation_names) {
        let watch = if method.server_streaming { ".watch()" } else { "" };
        source.push(format!(
            "    case HostOperation::{name}: return add(Dependency{{Operation<{}, {}>(\"{}\", VISIBILITY_PUBLIC){watch}}});",
            wire_names[method.request.as_str()],
            wire_names[method.response.as_str()],
            method.identity()
        ));
    }
    source.extend(
        ["    default: throw std::invalid_argument(\"Declare Host Requirement: Unknown Operation\");", "  }", "}", "}"].map(str::to_owned),
    );

    // Shorten Named Types in the Body Behind Using Declarations
    let mut named: BTreeSet<String> =
        methods.iter().filter(|method| method.payload_field.is_none()).map(|method| message(&method.response, type_names)).collect();
    for method in methods {
        named.extend(method.typed_fields().filter(|field| !field.type_name.is_empty()).map(|field| message(&field.type_name, type_names)));
    }
    named.extend(
        ["string", "vector", "optional", "size_t", "uint64_t", "uint32_t", "int64_t", "int32_t", "runtime_error", "invalid_argument"]
            .map(|name| format!("std::{name}")),
    );
    for qualified in &named {
        let (_, container) = type_import(qualified);
        imports.entry(last_segment(&container).to_owned()).or_default().insert(container);
    }
    let clashing = clashes(&imports);
    let mut named: Vec<String> = named.into_iter().collect();
    named.sort_by_key(|qualified| Reverse(qualified.len()));
    let mut body = source[boundary..].join("\n");
    let mut declarations = BTreeSet::new();
    for qualified in &named {
        if !body.contains(qualified.as_str()) {
            continue;
        }
        let (name, imported) = type_import(qualified);
        if clashing.contains(last_segment(&imported)) {
            continue;
        }
        body = replace_word(&body, qualified, &name, |_| false);
        let declaration = format!("using {imported};");
        if !source.contains(&declaration) {
            declarations.insert(declaration);
        }
    }
    source.truncate(boundary);
    source.extend(declarations);
    source.push(body);
    HostSources { header, source, roots: roots.len() }
}

/// Host resources over the published SDK, wrapped in an application plugin context.
fn application_host(rendered: HostSources, namespace: &str, code: &str) -> (String, String) {
    let scope = format!("namespace {namespace}::plugin {{");
    let target = format!("namespace {}::plugin {{", code.replace('.', "::"));
    let mut header = rendered.header.join("\n").replace(&scope, &target);
    header = replace_word(&header, "Context", "::harmos::plugin::Context", |character| character == ':' || identifier_character(character));
    header = header.replace("  ::harmos::plugin::Context context;", "private:\n  ::harmos::plugin::Context context;\npublic:");
    header.truncate(header.rfind('}').expect("host headers close their namespace"));
    header.push_str("struct PluginContext {\n  HostRoot host;\n  explicit PluginContext(::harmos::plugin::Context context) : host{");
    header.push_str(&vec!["{context}"; rendered.roots].join(", "));
    header.push_str("} {}\n};\n}\n");
    header = format!("#include \"harmos/harmos.hpp\"\n{header}")
        .replace("std::optional<OperationPolicy>", "std::optional<::harmos::OperationPolicy>");
    let mut source = rendered.source.join("\n");
    source.truncate(source.find("HostRoot Context::host()").expect("host sources define the root accessor"));
    source.push_str("}\n");
    source = source.replace(&scope, &format!("{target}\nusing namespace ::harmos::plugin;\nusing namespace ::harmos;"));
    (header, format!("#include \"plugin.hpp\"\n{source}"))
}

/// Render the client resources and their private wire header.
fn client(generation: &Generation, type_names: &BTreeMap<String, String>, code: &str, args: &Args) -> Result<ClientSources> {
    let namespace = generation.namespace.as_str();
    let methods = &generation.clients;
    let code = code.replace('.', "::");

    // Name Library Payloads by Their Public Barrel Paths
    let mut type_names = type_names.clone();
    let public_paths = generation.library.public_paths("harmos", &BTreeSet::new());
    for (full, audience) in generation.library.payload_audiences(None)? {
        let Some(path) = public_paths.get(&full) else {
            continue;
        };
        let boundary = if full.contains(".model.") || audience.len() > 1 {
            String::new()
        } else {
            format!("{}::", audience.first().expect("payload audiences name a face"))
        };
        type_names.insert(full, format!("::harmos::{boundary}{}", path.replace('.', "::")));
    }
    let field_type = |field: &Field| field_type(field, &type_names);
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::from([Vec::new()]);
    for method in methods {
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    let root = if args.client_transport { "Transport" } else { "Client" };
    let resource = |node: &Vec<String>| format!("{}Resource", node.iter().map(|part| camel(part)).collect::<String>());
    let names: BTreeMap<&Vec<String>, String> =
        nodes.iter().map(|node| (node, if node.is_empty() { root.to_owned() } else { resource(node) })).collect();

    // Keep Wire Stubs and Request Types in a Private Header
    let mut lines = vec![
        HEADER.to_owned(),
        "#pragma once".to_owned(),
        "#include \"harmos/client_transport.hpp\"".to_owned(),
        "#include \"harmos/messages.hpp\"".to_owned(),
    ];
    if namespace != "harmos" {
        lines.push(format!("#include \"{namespace}/messages.hpp\""));
    }
    let mut wire = vec![HEADER.to_owned(), "#pragma once".to_owned()];
    let includes: BTreeSet<String> =
        methods.iter().map(|method| format!("{}.grpc.pb.h", method.file.strip_suffix(".proto").unwrap_or(&method.file))).collect();
    wire.extend(includes.iter().map(|include| format!("#include \"{include}\"")));
    wire.push(format!("namespace {code}::detail::packages {{"));
    for method in methods {
        for full in [method.request.clone(), method.response.clone(), format!("{}.{}", method.package, method.service)] {
            let parts: Vec<&str> = full.split('.').collect();
            let boundary = parts.iter().position(|part| part.starts_with(char::is_uppercase)).expect("wire types carry a type name");
            wire.push(format!("namespace {} {{ using ::{}; }}", private_name(&parts[..boundary].join(".")), parts[..=boundary].join("::")));
        }
    }
    wire.push("}".to_owned());
    lines.push("#include \"detail/client.hpp\"".to_owned());
    if args.client_transport {
        lines.push(format!("#include \"{namespace}/detail/transport.hpp\""));
    }
    let messages = (code != namespace).then(|| {
        let mut messages = vec![HEADER.to_owned(), "#pragma once".to_owned(), "#include \"detail/client.hpp\"".to_owned()];
        for method in methods.iter().filter(|method| method.owner != "harmos") {
            let (_, container) = type_import(&format!("::{}", method.response.replace('.', "::")));
            let imported = private_name(&container.strip_prefix("::").unwrap_or(&container).replace("::", "."));
            messages
                .push(format!("namespace {}::client {{ using ::{code}::detail::packages::{imported}; }}", method.owner.replace('.', "::")));
        }
        lines.push("#include \"client_messages.hpp\"".to_owned());
        messages
    });

    // Render Call Options
    lines.extend([format!("namespace {code} {{"), "namespace client {".to_owned()]);
    for method in methods {
        lines.push(format!("struct {}Options {{", method.path.iter().map(|part| camel(part)).collect::<String>()));
        for field in method.fields.iter().filter(|field| field.optional) {
            lines.push(format!("  std::optional<{}> {};", field_type(field), identifier(&field.name)));
        }
        lines.extend(["  std::shared_ptr<grpc::ClientContext> context;", "};"].map(str::to_owned));
    }
    lines.push("}".to_owned());

    // Render Each Resource and Its Calls
    let mut ordered: Vec<&Vec<String>> = nodes.iter().collect();
    ordered.sort_by_key(|node| Reverse(node.len()));
    for node in ordered {
        let children: Vec<&Vec<String>> =
            nodes.iter().filter(|child| child.len() == node.len() + 1 && child[..node.len()] == node[..]).collect();
        let operations: Vec<&Method> = methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]).collect();
        if node.is_empty() && args.client_transport {
            lines.push("namespace detail {".to_owned());
        }
        lines.push(format!("struct {} {{", names[node]));
        if !operations.is_empty() {
            lines.push("  std::shared_ptr<grpc::Channel> channel;".to_owned());
        }
        lines.extend(children.iter().map(|child| format!("  {} {};", names[*child], identifier(child.last().expect("child")))));
        let mut members: BTreeSet<String> = BTreeSet::from(["channel".to_owned()]);
        members.extend(children.iter().map(|child| identifier(child.last().expect("child"))));
        members.extend(operations.iter().map(|method| identifier(method.path.last().expect("call path"))));
        let connection = parameter("connection", &members);
        let mut initializers = if operations.is_empty() { Vec::new() } else { vec![format!("channel({connection})")] };
        initializers.extend(children.iter().map(|child| format!("{}({connection})", identifier(child.last().expect("child")))));
        lines.push(format!("  explicit {}(std::shared_ptr<grpc::Channel> {connection}) : {} {{}}", names[node], initializers.join(", ")));
        if node.is_empty() {
            let address = parameter("address", &members);
            let transfer = parameter("transfer", &members);
            let message_bytes = parameter("message_bytes", &members);
            lines.extend([
                format!(
                    "  static {root} connect(std::string {address}, const ::harmos::StorageTransferSettings& {transfer} = {{}}, std::optional<int> {message_bytes} = {{}}) {{"
                ),
                format!("    const auto maximum = ::harmos::transfer_message_bytes({transfer}, {message_bytes});"),
                "    grpc::ChannelArguments arguments;".to_owned(),
                "    arguments.SetMaxSendMessageSize(maximum);".to_owned(),
                "    arguments.SetMaxReceiveMessageSize(maximum);".to_owned(),
                format!("    if ({address}.starts_with(\"http://\")) {{"),
                format!("      {address}.erase(0, 7);"),
                "    }".to_owned(),
                format!("    return {root}(grpc::CreateCustomChannel({address}, grpc::InsecureChannelCredentials(), arguments));"),
                "  }".to_owned(),
            ]);
        }
        for method in operations {
            if method.client_streaming {
                return Err(Error::Contract(format!(
                    "Generate C++ Client {}: Declare a Managed Contract for Client Streaming Before Regenerating",
                    method.method
                )));
            }
            let prefix = method.path.iter().map(|part| camel(part)).collect::<String>();
            let request = format!("detail::packages::{}", private_name(&method.request));
            let response = format!("detail::packages::{}", private_name(&method.response));
            let service = format!("detail::packages::{}::{}", private_name(&method.package), method.service);
            let options = parameter("options", &members);
            let maximum = parameter("max_item_bytes", &members);
            let mut occupied = members.clone();
            occupied.extend([options.clone(), maximum]);
            occupied.extend(LOCALS.iter().map(|local| (*local).to_owned()));
            let mut arguments = BTreeMap::new();
            for field in method.fields.iter().filter(|field| !field.optional) {
                let argument = parameter(&field.name, &occupied);
                occupied.insert(argument.clone());
                arguments.insert(field.name.as_str(), argument);
            }
            let mut parameters: Vec<String> = method
                .fields
                .iter()
                .filter(|field| !field.optional)
                .map(|field| format!("const {}& {}", field_type(field), arguments[field.name.as_str()]))
                .collect();
            parameters.push(format!("client::{prefix}Options {options} = {{}}"));
            let payload = method.payload_field.as_ref();
            let result = payload.map_or_else(|| message(&method.response, &type_names), field_type);
            let stream = method.server_streaming;
            let returns = if stream { format!("::harmos::ClientStream<{result}>") } else { result.clone() };
            let name = identifier(method.path.last().expect("call path"));
            lines.extend([format!("  {returns} {name}({}) {{", parameters.join(", ")), format!("    {request} request;")]);
            for field in &method.fields {
                let name = identifier(&field.name);
                if name == "request_id" {
                    lines.push(format!(
                        "    request.set_request_id({options}.request_id ? *{options}.request_id : ::harmos::detail::request_id());"
                    ));
                    continue;
                }
                if field.optional {
                    lines.push(format!("    if ({options}.{name}) {{"));
                }
                let value = if field.optional && (field.repeated || field.map) {
                    format!("(*{options}.{name})")
                } else if field.optional {
                    format!("*{options}.{name}")
                } else {
                    arguments[field.name.as_str()].clone()
                };
                let indent = if field.optional { "      " } else { "    " };
                if field.map {
                    lines.extend([
                        format!("{indent}for (const auto& [key, item] : {value}) {{"),
                        format!("{indent}  (*request.mutable_{name}())[key] = item;"),
                        format!("{indent}}}"),
                    ]);
                } else if field.repeated {
                    lines.push(format!("{indent}request.mutable_{name}()->Assign({value}.begin(), {value}.end());"));
                } else if field.kind == 11 {
                    lines.push(format!("{indent}*request.mutable_{name}() = {value};"));
                } else {
                    lines.push(format!("{indent}request.set_{name}({value});"));
                }
                if field.optional {
                    lines.push("    }".to_owned());
                }
            }
            lines.push(format!("    auto context = {options}.context ? {options}.context : std::make_shared<grpc::ClientContext>();"));
            let action = method.path.join(".");
            let failure = |verb: &str| match &args.rpc_error {
                Some(function) => format!("throw {function}(\"{action}\", status, *context);"),
                None => format!("throw std::runtime_error(\"{verb} {action}: \" + status.error_message());"),
            };
            let indent = if stream {
                let capture = if args.rpc_error.is_some() { "[reader, context]" } else { "[reader]" };
                lines.extend([
                    format!(
                        "    auto reader = std::shared_ptr<grpc::ClientReader<{response}>>({service}::NewStub(channel)->{}(context.get(), request));",
                        method.method
                    ),
                    format!("    return {{context, {capture}() -> std::optional<{result}> {{"),
                    format!("      {response} response;"),
                    "      if (!reader->Read(&response)) {".to_owned(),
                    "        const auto status = reader->Finish();".to_owned(),
                    format!("        if (!status.ok()) {{ {} }}", failure("Read")),
                    "        return {};".to_owned(),
                    "      }".to_owned(),
                ]);
                "      "
            } else {
                lines.extend([
                    format!("    {response} response;"),
                    format!("    const auto status = {service}::NewStub(channel)->{}(context.get(), request, &response);", method.method),
                    "    if (!status.ok()) {".to_owned(),
                    format!("      {}", failure("Call")),
                    "    }".to_owned(),
                ]);
                "    "
            };
            let value = match payload {
                Some(payload) if payload.repeated => {
                    format!("{result}(response.{}().begin(), response.{}().end())", payload.name, payload.name)
                }
                Some(payload) if payload.kind == 11 => {
                    lines.extend([
                        format!("{indent}if (!response.has_{}()) {{", payload.name),
                        format!("{indent}  throw std::runtime_error(\"Read {action}: Missing Payload\");"),
                        format!("{indent}}}"),
                    ]);
                    format!("std::move(*response.mutable_{}())", payload.name)
                }
                Some(payload) => format!("response.{}()", payload.name),
                None => "response".to_owned(),
            };
            lines.push(format!("{indent}return {value};"));
            if stream {
                lines.push("    }};".to_owned());
            }
            lines.push("  }".to_owned());
        }
        lines.push("};".to_owned());
    }
    if args.client_transport {
        lines.push("}".to_owned());
    }
    lines.push("}".to_owned());
    Ok(ClientSources { lines, wire, messages })
}

/// The C++ name of a protocol type; exported payloads keep their barrel names.
fn message(full: &str, type_names: &BTreeMap<String, String>) -> String {
    let parts: Vec<&str> = full.split('.').collect();
    let name = parts[parts.len() - 1];
    if full.starts_with("google.protobuf.") {
        return format!("::google::protobuf::{name}");
    }
    if let Some(exported) = type_names.get(full) {
        return exported.clone();
    }
    if full.contains(".host.") {
        format!("::{}", full.replace('.', "::"))
    } else if full.contains(".model.") {
        format!("::{}::{name}", parts[0])
    } else {
        format!("::{}::plugin::{name}", parts[0])
    }
}

fn field_type(field: &Field, type_names: &BTreeMap<String, String>) -> String {
    let value = if field.type_name.is_empty() { scalar(field.kind).to_owned() } else { message(&field.type_name, type_names) };
    if field.map {
        format!("std::map<std::string, {value}>")
    } else if field.repeated {
        format!("std::vector<{value}>")
    } else {
        value
    }
}

fn scalar(kind: i32) -> &'static str {
    match kind {
        1 => "double",
        2 => "float",
        4 | 6 => "std::uint64_t",
        5 | 15 | 17 => "std::int32_t",
        7 | 13 => "std::uint32_t",
        8 => "bool",
        9 | 12 => "std::string",
        _ => "std::int64_t",
    }
}

/// Split a qualified C++ name at its first type segment into the imported name and its container.
fn type_import(qualified: &str) -> (String, String) {
    let rooted = qualified.strip_prefix("::");
    let parts: Vec<&str> = rooted.unwrap_or(qualified).split("::").collect();
    let boundary = parts.iter().position(|part| part.starts_with(char::is_uppercase)).unwrap_or(parts.len() - 1);
    (parts[boundary..].join("::"), format!("{}{}", if rooted.is_some() { "::" } else { "" }, parts[..=boundary].join("::")))
}

/// Keep package versions inside the private transport header.
fn private_name(full: &str) -> String {
    full.split('.').filter(|part| !is_version(part)).collect::<Vec<_>>().join("::")
}

fn parameter(name: &str, members: &BTreeSet<String>) -> String {
    let mut name = identifier(name);
    while members.contains(&name) {
        name.push_str(if name.ends_with('_') { "value" } else { "_value" });
    }
    name
}

fn identifier(name: &str) -> String {
    if KEYWORDS.split(' ').any(|keyword| keyword == name) { format!("{name}_") } else { name.to_owned() }
}

fn last_segment(qualified: &str) -> &str {
    qualified.rsplit_once("::").map_or(qualified, |(_, name)| name)
}

/* === Tests === */
