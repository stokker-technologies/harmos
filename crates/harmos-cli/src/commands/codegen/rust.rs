/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Rust SDK Bindings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use clap::ValueEnum;

// Local Imports
use crate::commands::codegen::{Generation, Inputs, Output, identifier_character, replace_word};
use crate::descriptor::{Face, Field, Method, camel, is_version, snake, title};
use crate::error::{Error, Result};

/* === Constants === */

const HEADER: &str = "// @generated from protocol descriptors; do not edit.";

const KEYWORDS: &[&str] = &[
    "gen", "type", "while", "const", "crate", "await", "true", "final", "struct", "impl", "mut", "override", "match", "return", "use",
    "as", "box", "if", "priv", "abstract", "virtual", "become", "try", "continue", "unsafe", "mod", "yield", "pub", "static", "loop",
    "where", "do", "in", "async", "fn", "typeof", "macro", "let", "move", "trait", "dyn", "unsized", "false", "for", "else", "ref",
    "super", "break", "enum", "extern",
];

const SETTINGS_LIST: &str = "harmos.host.v1.plugins.SettingsListRequest";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(flatten)]
    inputs: Inputs,

    /// Keep only these request fields positional and move the rest into call options, as `package.Request=field,field` (repeatable).
    #[arg(long = "selector", value_name = "REQUEST=FIELDS")]
    selectors: Vec<String>,

    /// Visibility of the generated host entry point and its exports.
    #[arg(long, value_enum, default_value_t = Visibility::Public)]
    host_visibility: Visibility,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Visibility {
    /// `pub`: the SDK exposes the host resources.
    Public,

    /// `pub(crate)`: product contexts wrap the host resources.
    Crate,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn run(args: Args) -> Result<()> {
    let generation = args.inputs.load()?;
    let selectors = selectors(&args.selectors)?;
    let clients = call_fields(&generation.clients, &selectors);
    let contexts = call_fields(&generation.contexts, &selectors);
    let mut output = Output::default();
    if generation.application {
        let (resources, types) = client(&clients, true)?;
        output.add("resources.rs", &resources);
        output.add("client_types.rs", &types);
        output.add_source("plugin.rs", application_host(&contexts));
        return output.write(args.inputs.out());
    }
    for (path, lines) in exports(&generation)? {
        output.add(path, &lines);
    }
    if !clients.is_empty() {
        output.add("client/resources.rs", &client(&clients, false)?.0);
    }
    if generation.namespace == "harmos" {
        output.add("context/contracts.rs", &contracts(&contexts));
    }
    let (host, exports) = host(&contexts, args.host_visibility);
    output.add("plugin/host.rs", &host);
    output.add_source("plugin/host_exports.rs", exports);
    if generation.namespace != "harmos" {
        output.add("host.rs", &bindings(&contexts));
    }
    output.write(args.inputs.out())
}

/// The Rust path of a protocol type.
pub(crate) fn message(full: &str) -> String {
    for (package, module) in [("harmos.host.v1.", "host"), ("harmos.plugin.v1.", "plugin"), ("harmos.model.v1.", "model")] {
        let Some(rest) = full.strip_prefix(package) else {
            continue;
        };
        let parts: Vec<&str> = rest.split('.').collect();
        let mut path: Vec<String> = parts[..parts.len() - 1].iter().map(|part| snake(part)).collect();
        path.push(last(full).to_owned());
        return format!("harmos_protocol::{module}::{}", path.join("::"));
    }
    if full.starts_with("google.protobuf.") {
        return format!("prost_types::{}", last(full));
    }
    let parts: Vec<&str> = full.split('.').collect();
    let version = parts.iter().position(|part| is_version(part)).expect("product protocol packages carry a version");
    let mut path = vec![parts[version - 1].to_owned()];
    path.extend(parts[version + 1..parts.len() - 1].iter().map(|part| identifier(&snake(part))));
    path.push(parts[parts.len() - 1].to_owned());
    format!("{}_protocol::{}", snake(parts[0]), path.join("::"))
}

/* === Private Functions === */

fn exports(generation: &Generation) -> Result<Vec<(&'static str, Vec<String>)>> {
    let schema = &generation.schema;
    let namespace = generation.namespace.as_str();
    let methods = schema.table()?;
    let mut resource_names = BTreeSet::new();
    for method in methods.iter().filter(|method| method.face == Face::Client) {
        for length in 1..method.path.len() {
            resource_names.insert(method.path[..length].iter().map(|part| camel(part)).collect::<String>());
        }
    }
    let public_paths = schema.public_paths(namespace, &BTreeSet::new());
    let client_paths = schema.public_paths(namespace, &resource_names);
    let requests = schema.request_names()?;
    let audiences = schema.payload_audiences(Some(&generation.library))?;

    // Group Payload Re-Exports by Boundary
    let mut groups: BTreeMap<&str, Vec<(Vec<String>, String)>> =
        BTreeMap::from([("shared", Vec::new()), ("client", Vec::new()), ("plugin", Vec::new())]);
    for file in schema.files() {
        let package = file.package();
        let owned = package.starts_with(&format!("{namespace}.")) || package.starts_with("harmos.");
        if !owned || file.name().ends_with("guest.proto") || package.starts_with("harmos.plugin.") {
            continue;
        }
        let messages = file.message_type.iter().map(|message| (message.name(), Some(message)));
        let enumerations = file.enum_type.iter().map(|enumeration| (enumeration.name(), None));
        for (name, message_type) in messages.chain(enumerations) {
            let full = format!("{package}.{name}");
            if requests.contains(&full) || ["Request", "Response"].contains(&name) || name.starts_with("Child") {
                continue;
            }
            let source = if namespace == "harmos" {
                message(&full)
                    .replace("harmos_protocol::model", "crate::wire::shared")
                    .replace("harmos_protocol::host", "crate::wire::host")
            } else {
                message(&full)
                    .replace(&format!("{namespace}_protocol::model"), "crate::wire::model")
                    .replace(&format!("{namespace}_protocol::host"), "crate::wire::host")
                    .replace(&format!("{namespace}_protocol::plugin"), "crate::wire::plugin")
            };
            let reached = audiences.get(&full).cloned().unwrap_or_default();
            let boundaries: BTreeSet<&str> = if package.contains(".plugin.") {
                BTreeSet::from(["plugin"])
            } else if package.contains(".model.") || reached.len() > 1 {
                BTreeSet::from(["shared"])
            } else if reached.is_empty() {
                BTreeSet::from(["client"])
            } else {
                reached
            };
            let nested = message_type.is_some_and(|message| {
                let modules = message.nested_type.iter().any(|nested| !nested.options.as_ref().is_some_and(|options| options.map_entry()));
                let oneofs = message.field.iter().any(|field| field.oneof_index.is_some() && !field.proto3_optional());
                modules || !message.enum_type.is_empty() || oneofs
            });
            for boundary in boundaries {
                let paths = if boundary == "client" { &client_paths } else { &public_paths };
                let mut prefix: Vec<String> = paths[&full].split('.').map(str::to_owned).collect();
                prefix.pop();
                if boundary == "plugin" && prefix.first().is_some_and(|first| first == "host") {
                    prefix.insert(0, namespace.to_owned());
                }
                let entries = groups.get_mut(boundary).expect("payload boundaries are shared, client or plugin");
                entries.push((prefix.clone(), format!("pub use {source};")));
                if nested {
                    let parent = source.rsplit_once("::").map_or(source.as_str(), |(parent, _)| parent);
                    entries.push((prefix, format!("pub use {parent}::{};", snake(name))));
                }
            }
        }
    }

    // Render Each Boundary as Nested Modules
    let mut rendered = Vec::new();
    for (boundary, mut entries) in groups {
        entries.sort();
        let mut lines = vec![HEADER.to_owned()];
        let mut opened: Vec<String> = Vec::new();
        for (prefix, declaration) in entries {
            let shared = opened.iter().zip(&prefix).take_while(|(first, second)| first == second).count();
            lines.extend(opened[shared..].iter().map(|_| "}".to_owned()));
            lines.extend(prefix[shared..].iter().map(|part| format!("pub mod {part} {{")));
            lines.push(declaration);
            opened = prefix;
        }
        lines.extend(opened.iter().map(|_| "}".to_owned()));
        let path = match boundary {
            "shared" => "exports.rs",
            "client" => "client/types.rs",
            _ => "plugin/types.rs",
        };
        rendered.push((path, lines));
    }
    Ok(rendered)
}

fn client(methods: &[Method], application: bool) -> Result<(Vec<String>, Vec<String>)> {
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
    for method in &methods {
        for length in 1..method.path.len() {
            nodes.insert(method.path[..length].to_vec());
        }
    }
    let names: BTreeMap<Vec<String>, String> = nodes
        .iter()
        .map(|node| (node.clone(), if node.is_empty() { "Root".to_owned() } else { node.iter().map(|part| camel(part)).collect() }))
        .collect();
    let used = used_types(&methods, |_| true);
    let mut reserved: BTreeSet<String> = names.values().cloned().collect();
    reserved.extend(["Call", "Client", "CallOptions", "ClientTransport", "Watching", "ReflectMessage", "PathAndQuery"].map(str::to_owned));
    reserved.extend(methods.iter().map(|method| options_name(method)));
    let message_names = message_names(&used, &reserved);
    let field_type = |field: &Field| field_type(field, &message_names);

    // Render Imports and Call Options
    let mut lines = vec![
        HEADER.to_owned(),
        "use harmos::client::{CallOptions, ClientTransport, Watching, action_error, request_id};".to_owned(),
        "use prost_reflect::ReflectMessage;".to_owned(),
        "use tonic::codegen::http::uri::PathAndQuery;".to_owned(),
    ];
    lines.extend(used.iter().filter(|full| !message_names[*full].contains("::")).map(|full| format!("use {};", message(full))));
    for method in &methods {
        let name = options_name(method);
        lines.extend(["#[derive(Default)]".to_owned(), format!("pub struct {name} {{")]);
        for field in method.fields.iter().filter(|field| field.optional) {
            lines.push(format!("    pub {}: Option<{}>,", identifier(&field.name), field_type(field)));
        }
        lines.extend(["    pub context: CallOptions,", "}"].map(str::to_owned));
        lines.extend([format!("impl Clone for {name} {{"), "    fn clone(&self) -> Self {".to_owned(), "        Self {".to_owned()]);
        for field in method.fields.iter().filter(|field| field.optional) {
            let name = identifier(&field.name);
            lines.push(format!("            {name}: self.{name}{},", if field.copy { "" } else { ".clone()" }));
        }
        lines.extend(
            [
                "            context: CallOptions { metadata: self.context.metadata.clone(), timeout: self.context.timeout },",
                "        }",
                "    }",
                "}",
            ]
            .map(str::to_owned),
        );
    }

    // Render Each Resource and Its Calls
    for node in &nodes {
        let children: Vec<&Vec<String>> =
            nodes.iter().filter(|child| child.len() == node.len() + 1 && child[..node.len()] == node[..]).collect();
        let operations: Vec<&Method> = methods.iter().filter(|method| method.path[..method.path.len() - 1] == node[..]).collect();
        lines.extend(["#[derive(Clone)]".to_owned(), format!("pub struct {} {{", names[node])]);
        if !operations.is_empty() {
            lines.push("    client: ClientTransport,".to_owned());
        }
        lines.extend(children.iter().map(|child| format!("    pub {}: {},", identifier(child.last().expect("child")), names[*child])));
        lines.extend([
            "}".to_owned(),
            format!("impl {} {{", names[node]),
            "    pub fn new(client: ClientTransport) -> Self {".to_owned(),
            "        Self {".to_owned(),
        ]);
        lines.extend(
            children
                .iter()
                .map(|child| format!("            {}: {}::new(client.clone()),", identifier(child.last().expect("child")), names[*child])),
        );
        if !operations.is_empty() {
            lines.push("            client,".to_owned());
        }
        lines.extend(["        }", "    }"].map(str::to_owned));
        for method in operations {
            if method.client_streaming {
                return Err(Error::Contract(format!(
                    "Generate Rust Client {}: Declare a Managed Contract for Client Streaming Before Regenerating",
                    method.method
                )));
            }
            let request = &message_names[&method.request];
            let response = &message_names[&method.response];
            let required: Vec<&Field> = method.fields.iter().filter(|field| !field.optional).collect();
            let parameters: Vec<String> = required
                .iter()
                .map(|field| {
                    let value = field_type(field);
                    format!("{}: {}", identifier(&field.name), if value == "String" { "impl Into<String>".to_owned() } else { value })
                })
                .collect();
            let mut args: Vec<String> = required.iter().map(|field| identifier(&field.name)).collect();
            let options = options_name(method);
            let mut result = method.payload_field.as_ref().map_or_else(|| response.clone(), field_type);
            if method.server_streaming {
                result = format!("Watching<{result}>");
            }
            let name = identifier(method.path.last().expect("call path"));
            args.push(format!("{options}::default()"));
            let separator = if parameters.is_empty() { "" } else { ", " };
            lines
                .push(format!("    pub async fn {name}(&self{separator}{}) -> Result<{result} , tonic::Status> {{", parameters.join(", ")));
            lines.push(format!("        self.{name}_with({}).await", args.join(", ")));
            lines.push("    }".to_owned());
            let mut with = parameters.clone();
            with.push(format!("options: {options}"));
            lines.push(format!("    pub async fn {name}_with(&self, {}) -> Result<{result} , tonic::Status> {{", with.join(", ")));
            lines.push(format!("        let value = {request} {{"));
            for group in oneofs(method) {
                let members: Vec<&Field> = method.fields.iter().filter(|field| field.oneof == group).collect();
                let variant = format!("{}::{}::{}", parent(&message(&method.request)), snake(last(&method.request)), camel(&group));
                let variants = members.iter().map(|field| format!("options.{}", field.name)).collect::<Vec<_>>().join(", ") + ",";
                lines.push(format!("            {group}: match ({variants}) {{"));
                for (index, member) in members.iter().enumerate() {
                    let pattern = patterns(members.len(), index) + ",";
                    lines.push(format!("                ({pattern}) => Some({variant}::{}(value)),", title(&member.name)));
                }
                let empty = vec!["None"; members.len()].join(", ") + ",";
                lines.push(format!("                ({empty}) => None,"));
                lines.push(format!(
                    "                _ => return Err(tonic::Status::invalid_argument(\"Choose At Most One {group} Variant\")),"
                ));
                lines.push("            },".to_owned());
            }
            for field in method.fields.iter().filter(|field| field.oneof.is_empty()) {
                let name = identifier(&field.name);
                let settings_name = method.request == SETTINGS_LIST && name == "name";
                let mut expression = if name == "request_id" {
                    "match options.request_id { Some(value) => value, None => request_id()?, }".to_owned()
                } else if settings_name {
                    "Some(name.into())".to_owned()
                } else if field.optional {
                    let unwrap = !field.wire_optional && (field.kind != 11 || field.repeated || field.map);
                    format!("options.{name}{}", if unwrap { ".unwrap_or_default()" } else { "" })
                } else if field.kind == 11 && !field.repeated && !field.map {
                    format!("Some({name})")
                } else if field_type(field) == "String" {
                    format!("{name}.into()")
                } else {
                    name.clone()
                };
                if field.kind == 14 {
                    expression = enumeration(&expression, field);
                }
                if field.wire_optional && !field.optional && field.kind != 11 && !settings_name {
                    expression = format!("Some({expression})");
                }
                lines.push(if expression == name { format!("            {name},") } else { format!("            {name}: {expression},") });
            }
            lines.extend(["        };", "        let request = options.context.request(value);"].map(str::to_owned));
            let path = format!("/{}.{}/{}", method.package, method.service, method.method);
            let call = if method.server_streaming { "watch_known" } else { "unary" };
            let action = method.path.join(".");
            lines.push(format!(
                "        let response = self.client.{call}::<{request}, {response}>(PathAndQuery::from_static(\"{path}\"), request, {response}::default().descriptor()).await.map_err(|error| action_error(\"{action}\", error))?;"
            ));
            let missing = format!("tonic::Status::data_loss(\"Read {}.{}: Missing Payload\")", method.service, method.method);
            if method.server_streaming {
                lines.push(format!("        let response = response.with_action(\"{action}\");"));
                match &method.payload_field {
                    Some(payload) => {
                        let value = if payload.kind == 11 && !payload.repeated {
                            format!("response.{}.ok_or_else(|| {missing})", payload.name)
                        } else {
                            format!("Ok(response.{})", payload.name)
                        };
                        lines.push(format!("        Ok(response.map(|response| {value}))"));
                    }
                    None => lines.push("        Ok(response)".to_owned()),
                }
            } else {
                lines.push("        let response = response.into_inner();".to_owned());
                match &method.payload_field {
                    Some(payload) if payload.kind == 11 && !payload.repeated => {
                        lines.push(format!("        response.{}.ok_or_else(|| {missing})", payload.name));
                    }
                    Some(payload) => lines.push(format!("        Ok(response.{})", payload.name)),
                    None => lines.push("        Ok(response)".to_owned()),
                }
            }
            lines.push("    }".to_owned());
        }
        lines.push("}".to_owned());
    }

    // Re-Export Application Responses Next to the Client
    let mut types = vec![HEADER.to_owned()];
    if application {
        let responses: BTreeSet<&String> = methods
            .iter()
            .map(|method| &method.response)
            .filter(|response| !response.starts_with("harmos.") && !response.starts_with("google."))
            .collect();
        types.extend(responses.into_iter().map(|response| format!("pub use {};", message(response))));
    }
    Ok((lines, types))
}

fn contracts(methods: &[Method]) -> Vec<String> {
    let mut messages: BTreeSet<&str> = methods.iter().flat_map(|method| [method.request.as_str(), method.response.as_str()]).collect();
    messages.extend(["harmos.model.v1.SettingDeclaration", "harmos.model.v1.SettingsDeclarations"]);
    let mut lines = vec![
        "// @generated from the protocol descriptor table; do not edit.".to_owned(),
        "use crate::{Message, OperationRequest};".to_owned(),
        "use prost_types::FileDescriptorSet;".to_owned(),
    ];
    lines.extend(messages.iter().map(|full| format!("use {};", message(full))));
    for full in &messages {
        lines.extend([
            format!("impl Message for {} {{", last(full)),
            format!("    fn name() -> String {{ \"{full}\".into() }}"),
            "    fn types() -> FileDescriptorSet {".to_owned(),
            format!(
                "        <FileDescriptorSet as prost::Message>::decode(&include_bytes!(\"types/{full}.bin\")[..]).expect(\"generated context wrapper\")"
            ),
            "    }".to_owned(),
            "}".to_owned(),
        ]);
    }
    for method in methods {
        lines.extend([
            format!("impl OperationRequest for {} {{", last(&method.request)),
            format!("    type Response = {};", last(&method.response)),
            format!("    const ID: &'static str = \"{}\";", method.identity()),
            format!("    const WATCH: bool = {};", method.server_streaming),
            "}".to_owned(),
        ]);
    }
    lines
}

/// The SDK-internal host resources and their export list.
fn host(methods: &[Method], visibility: Visibility) -> (Vec<String>, String) {
    let rendered = host_resources(methods, false);
    let mut lines = rendered.lines;
    if visibility == Visibility::Crate {
        for line in lines.iter_mut().filter(|line| line.as_str() == "    pub fn host(&self) -> HostRoot {") {
            *line = "    pub(crate) fn host(&self) -> HostRoot {".to_owned();
        }
    }
    let mut exports = vec!["HostOperation".to_owned(), "HostRoot".to_owned()];
    exports.extend(rendered.names);
    for method in methods.iter().filter(|method| method.surface) {
        let name = method.path.iter().map(|part| camel(part)).collect::<String>();
        exports.push(format!("Host{name}Options"));
        if method.server_streaming {
            exports.push(format!("Host{name}"));
        }
    }
    exports.sort();
    let declaration = match visibility {
        Visibility::Public => "pub use",
        Visibility::Crate => {
            "#[allow(unused_imports, reason = \"The Generator Emits the Complete Internal Host Inventory\")]\npub(crate) use"
        }
    };
    (lines, format!("{HEADER}\n{declaration} self::host::{{{}}};\n", exports.join(", ")))
}

/// Host resources over the published SDK, wrapped in an application plugin context.
fn application_host(methods: &[Method]) -> String {
    let rendered = host_resources(methods, true);
    let source = rendered.lines.join("\n");
    let start = source.find("impl Registration for HostOperation {").expect("host resources register their operations");
    let end = start + source[start..].find("#[derive(Clone)]").expect("host resources declare their root");
    let mut source = format!("{}{}", &source[..start], &source[end..]);
    source = source.replace("use crate::{Context, Error, Result, OperationPolicy};", "use harmos::{Error, Result, OperationPolicy};");
    source = source.replace("use crate::plugin::{Dependency, Registrar, Registration};", "");
    source = replace_word(&source, "Context", "harmos::plugin::Context", |character| character == ':' || identifier_character(character));
    source = source.replace("crate::context::Watching", "harmos::plugin::Watching");
    source.push_str("\n#[derive(Clone)]\npub struct PluginContext { pub host: HostRoot }\nimpl PluginContext {\n");
    source.push_str("pub fn new(context: harmos::plugin::Context) -> Self { Self { host: HostRoot::new(context) } }\n}\n");
    source.push('\n');
    source
}

struct HostResources {
    lines: Vec<String>,
    names: Vec<String>,
}

fn host_resources(methods: &[Method], application: bool) -> HostResources {
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
    let mut used: BTreeSet<String> = BTreeSet::new();
    for method in methods {
        used.insert(method.request.clone());
        if method.surface {
            used.insert(method.response.clone());
        }
    }
    used.extend(used_types(methods, |_| false));
    let mut reserved: BTreeSet<String> =
        ["Context", "Error", "Result", "Plugin", "OperationPolicy", "NonZeroUsize", "HostOperation", "HostRoot"].map(str::to_owned).into();
    reserved.extend(names.values().cloned());
    for method in methods {
        let name = method.path.iter().map(|part| camel(part)).collect::<String>();
        reserved.extend([format!("Host{name}"), format!("Host{name}Options")]);
    }
    let message_names = message_names(&used, &reserved);
    let field_type = |field: &Field| field_type(field, &message_names);

    // Render Imports and the Operation Inventory
    let mut lines = vec![
        HEADER.to_owned(),
        "use crate::{Context, Error, Result, OperationPolicy};".to_owned(),
        "use crate::plugin::{Dependency, Registrar, Registration};".to_owned(),
        "use std::num::NonZeroUsize;".to_owned(),
    ];
    let mut surface_types: BTreeSet<String> = BTreeSet::new();
    for method in methods.iter().filter(|method| method.surface) {
        surface_types.extend([method.request.clone(), method.response.clone()]);
        surface_types.extend(method.typed_fields().filter(|field| !field.type_name.is_empty()).map(|field| field.type_name.clone()));
    }
    for full in &used {
        if !message_names[full].contains("::") && (!application || surface_types.contains(full)) {
            lines.push(format!("use {};", message(full)));
        }
    }
    let operation_names: Vec<String> = methods.iter().map(|method| method.path.iter().map(|part| camel(part)).collect()).collect();
    lines.push("pub enum HostOperation {".to_owned());
    lines.extend(operation_names.iter().map(|name| format!("    {name},")));
    lines.extend(["}".to_owned(), "impl HostOperation {".to_owned(), format!("    pub const ALL: [Self; {}] = [", methods.len())]);
    lines.extend(operation_names.iter().map(|name| format!("        Self::{name},")));
    lines.extend(
        [
            "    ];",
            "}",
            "impl Registration for HostOperation {",
            "    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {",
            "        match self {",
        ]
        .map(str::to_owned),
    );
    for (method, name) in methods.iter().zip(&operation_names) {
        lines.push(format!("            HostOperation::{name} => ctx.register(Dependency::<{}>::new()),", message_names[&method.request]));
    }
    lines.extend(
        [
            "        }",
            "    }",
            "}",
            "impl Context {",
            "    pub fn host(&self) -> HostRoot {",
            "        HostRoot::new(self.clone())",
            "    }",
            "}",
            "#[derive(Clone)]",
            "pub struct HostRoot {",
        ]
        .map(str::to_owned),
    );
    let roots: Vec<&Vec<String>> = nodes.iter().filter(|node| node.len() == 1).collect();
    lines.extend(roots.iter().map(|node| format!("    pub {}: {},", node[0], names[*node])));
    lines.extend(["}", "impl HostRoot {", "    fn new(context: Context) -> Self {", "        Self {"].map(str::to_owned));
    lines.extend(roots.iter().map(|node| format!("            {}: {}::new(context.clone()),", node[0], names[*node])));
    lines.extend(["        }", "    }", "}"].map(str::to_owned));

    // Render Call Options and Watch Handles
    for (method, name) in methods.iter().zip(&operation_names).filter(|(method, _)| method.surface) {
        lines.extend(["#[derive(Default)]".to_owned(), format!("pub struct Host{name}Options {{")]);
        for field in method.fields.iter().filter(|field| field.optional) {
            lines.push(format!("    pub {}: Option<{}>,", identifier(&field.name), field_type(field)));
        }
        if !method.server_streaming {
            lines.push("    pub policy: Option<OperationPolicy>,".to_owned());
        }
        lines.push("}".to_owned());
        if !method.server_streaming {
            continue;
        }
        let response = &message_names[&method.response];
        let result = method.payload_field.as_ref().map_or_else(|| response.clone(), field_type);
        lines.extend([
            format!("pub struct Host{name} {{"),
            format!("    incoming: crate::context::Watching<{response}>,"),
            "}".to_owned(),
            format!("impl Host{name} {{"),
            format!("    pub async fn next(&mut self) -> Result<Option<{result}>> {{"),
            "        let Some(response) = self.incoming.next().await? else {".to_owned(),
            "            return Ok(None);".to_owned(),
            "        };".to_owned(),
        ]);
        match &method.payload_field {
            Some(payload) if payload.kind == 11 && !payload.repeated => lines.push(format!(
                "        response.{}.map(Some).ok_or_else(|| Error::Invalid(\"Read {name}: Missing Payload\".into()))",
                payload.name
            )),
            Some(payload) => lines.push(format!("        Ok(Some(response.{}))", payload.name)),
            None => lines.push("        Ok(Some(response))".to_owned()),
        }
        lines.extend(["    }", "}"].map(str::to_owned));
    }

    // Render Each Resource and Its Calls
    for node in &nodes {
        let operations: Vec<(&Method, &String)> =
            methods.iter().zip(&operation_names).filter(|(method, _)| method.path[..method.path.len() - 1] == node[..]).collect();
        let children: Vec<&Vec<String>> = nodes.iter().filter(|child| child[..child.len() - 1] == node[..]).collect();
        lines.extend(["#[derive(Clone)]".to_owned(), format!("pub struct {} {{", names[node])]);
        if !operations.is_empty() {
            lines.push("    context: Context,".to_owned());
        }
        lines.extend(children.iter().map(|child| format!("    pub {}: {},", identifier(child.last().expect("child")), names[*child])));
        lines.extend(["}".to_owned(), format!("impl {} {{", names[node]), "    fn new(context: Context) -> Self { Self {".to_owned()]);
        lines.extend(
            children
                .iter()
                .map(|child| format!("        {}: {}::new(context.clone()),", identifier(child.last().expect("child")), names[*child])),
        );
        if !operations.is_empty() {
            lines.push("        context,".to_owned());
        }
        lines.push("    } }".to_owned());
        for (method, name) in operations {
            let request = &message_names[&method.request];
            let response = &message_names[&method.response];
            let required: Vec<&Field> = method.fields.iter().filter(|field| !field.optional).collect();
            let mut parameters: Vec<String> =
                required.iter().map(|field| format!("{}: {}", identifier(&field.name), field_type(field))).collect();
            let mut args: Vec<String> = required.iter().map(|field| identifier(&field.name)).collect();
            if method.server_streaming {
                parameters.push("max_item_bytes: NonZeroUsize".to_owned());
                args.push("max_item_bytes".to_owned());
            }
            let result = if method.server_streaming {
                format!("Host{name}")
            } else {
                method.payload_field.as_ref().map_or_else(|| response.clone(), field_type)
            };
            let action = identifier(method.path.last().expect("call path"));
            let separator = if parameters.is_empty() { "" } else { ", " };
            args.push(format!("Host{name}Options::default()"));
            lines.push(format!("    pub async fn {action}(&self{separator}{}) -> Result<{result}> {{", parameters.join(", ")));
            lines.push(format!("        self.{action}_with({}).await", args.join(", ")));
            lines.push("    }".to_owned());
            parameters.push(format!("options: Host{name}Options"));
            lines.push(format!("    pub async fn {action}_with(&self, {}) -> Result<{result}> {{", parameters.join(", ")));
            lines.push(format!("        let request = {request} {{"));
            for group in oneofs(method) {
                let members: Vec<&Field> = method.fields.iter().filter(|field| field.oneof == group).collect();
                let variant = format!("{}::{}::{}", parent(&message(&method.request)), snake(last(&method.request)), camel(&group));
                let selected = members.iter().map(|field| format!("options.{}", field.name)).collect::<Vec<_>>().join(", ");
                lines.push(format!("            {group}: match ({selected}) {{"));
                for (index, member) in members.iter().enumerate() {
                    let pattern = patterns(members.len(), index) + ",";
                    lines.push(format!("                ({pattern}) => Some({variant}::{}(value)),", camel(&member.name)));
                }
                lines.push(format!("                _ => return Err(Error::Invalid(\"Choose Exactly One {group} Variant\".into())),"));
                lines.push("            },".to_owned());
            }
            for field in method.fields.iter().filter(|field| field.oneof.is_empty()) {
                let mut value = if field.optional {
                    format!("options.{}", identifier(&field.name))
                } else if field.kind == 11 && !field.map && !field.repeated {
                    format!("Some({})", identifier(&field.name))
                } else {
                    field.name.clone()
                };
                if field.name == "request_id" {
                    value = "match options.request_id { Some(value) => value, None => harmos::client::request_id()?, }".to_owned();
                } else if field.optional && !field.wire_optional && (field.kind != 11 || field.repeated || field.map) {
                    value.push_str(".unwrap_or_default()");
                }
                if field.kind == 14 {
                    value = enumeration(&value, field);
                }
                let name = identifier(&field.name);
                lines.push(if value == field.name { format!("            {name},") } else { format!("            {name}: {value},") });
            }
            lines.push("        };".to_owned());
            if method.server_streaming {
                if !method.fields.iter().any(|field| field.optional) {
                    lines.push(format!("        let Host{name}Options {{}} = options;"));
                }
                lines.push(format!("        Ok(Host{name} {{ incoming: self.context.watch(request, max_item_bytes).await? }})"));
            } else {
                lines.push(format!("        let response: {response} = self.context.operation_with(request, options.policy).await?;"));
                match &method.payload_field {
                    Some(payload) if payload.kind == 11 && !payload.repeated => lines.push(format!(
                        "        response.{}.ok_or_else(|| Error::Invalid(\"Read {name}: Missing Payload\".into()))",
                        payload.name
                    )),
                    Some(payload) => lines.push(format!("        Ok(response.{})", payload.name)),
                    None => lines.push("        Ok(response)".to_owned()),
                }
            }
            lines.push("    }".to_owned());
        }
        lines.push("}".to_owned());
    }
    HostResources {
        lines,
        names: names.into_values().collect(),
    }
}

/// Product request bindings: each context call's request names its response, identity and kind.
fn bindings(methods: &[Method]) -> Vec<String> {
    let mut lines = [
        HEADER,
        "pub use crate::message::Wire;",
        "use prost_reflect::ReflectMessage;",
        "pub trait Request: ReflectMessage + Default + 'static {\n    type Response: ReflectMessage + Default + 'static;\n    const ID: &'static str;\n    const WATCH: bool;\n}",
        "impl<T: Request> harmos::OperationRequest for Wire<T> { type Response = Wire<T::Response>; const ID: &'static str = T::ID; const WATCH: bool = T::WATCH; }",
    ]
    .map(str::to_owned)
    .to_vec();
    for method in methods {
        lines.push(format!(
            "impl Request for {} {{ type Response = {}; const ID: &'static str = \"{}\"; const WATCH: bool = {}; }}",
            message(&method.request),
            message(&method.response),
            method.identity(),
            method.server_streaming
        ));
    }
    lines
}

fn selectors(values: &[String]) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut selectors = BTreeMap::new();
    for value in values {
        let Some((request, fields)) = value.split_once('=') else {
            return Err(Error::Input(format!("Generate Rust SDK: Write --selector {value} as package.Request=field,field")));
        };
        selectors.insert(request.to_owned(), fields.split(',').map(str::to_owned).collect());
    }
    Ok(selectors)
}

/// Keep selector fields positional; every other field of a selected request becomes a call option.
fn call_fields(methods: &[Method], selectors: &BTreeMap<String, BTreeSet<String>>) -> Vec<Method> {
    let mut selected = methods.to_vec();
    for method in &mut selected {
        let Some(fields) = selectors.get(&method.request) else {
            continue;
        };
        for field in &mut method.fields {
            field.optional = field.optional || !fields.contains(&field.name);
        }
    }
    selected
}

fn used_types(methods: &[Method], messages: impl Fn(&Method) -> bool) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for method in methods {
        if messages(method) {
            used.insert(method.request.clone());
            used.insert(method.response.clone());
        }
        used.extend(method.typed_fields().filter(|field| !field.type_name.is_empty()).map(|field| field.type_name.clone()));
    }
    used
}

/// Bare names unless they clash with each other or with generated names; clashes keep their full path.
fn message_names(used: &BTreeSet<String>, reserved: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for full in used {
        *counts.entry(last(full)).or_default() += 1;
    }
    used.iter()
        .map(|full| {
            let name = last(full);
            let clashing = reserved.contains(name) || counts[name] > 1;
            (full.clone(), if clashing { message(full) } else { name.to_owned() })
        })
        .collect()
}

fn field_type(field: &Field, message_names: &BTreeMap<String, String>) -> String {
    let value = if field.type_name.is_empty() { scalar(field.kind).to_owned() } else { message_names[&field.type_name].clone() };
    if field.map {
        format!("std::collections::HashMap<String, {value}>")
    } else if field.repeated {
        format!("Vec<{value}>")
    } else {
        value
    }
}

fn scalar(kind: i32) -> &'static str {
    match kind {
        1 => "f64",
        2 => "f32",
        4 | 6 => "u64",
        5 | 15 | 17 => "i32",
        7 | 13 => "u32",
        8 => "bool",
        9 => "String",
        12 => "Vec<u8>",
        _ => "i64",
    }
}

fn enumeration(expression: &str, field: &Field) -> String {
    if field.repeated {
        format!("{expression}.into_iter().map(|value| value as i32).collect()")
    } else if field.wire_optional && field.optional {
        format!("{expression}.map(|value| value as i32)")
    } else {
        format!("{expression} as i32")
    }
}

fn options_name(method: &Method) -> String {
    format!("{}Options", method.path.iter().map(|part| camel(part)).collect::<String>())
}

fn oneofs(method: &Method) -> BTreeSet<String> {
    method.fields.iter().filter(|field| !field.oneof.is_empty()).map(|field| field.oneof.clone()).collect()
}

fn patterns(count: usize, selected: usize) -> String {
    (0..count).map(|index| if index == selected { "Some(value)" } else { "None" }).collect::<Vec<_>>().join(", ")
}

fn identifier(name: &str) -> String {
    if ["self", "Self", "super", "crate"].contains(&name) {
        return format!("{name}_");
    }
    if KEYWORDS.contains(&name) { format!("r#{name}") } else { name.to_owned() }
}

fn last(full: &str) -> &str {
    full.rsplit_once('.').map_or(full, |(_, name)| name)
}

fn parent(path: &str) -> &str {
    path.rsplit_once("::").map_or(path, |(parent, _)| parent)
}

/* === Tests === */
