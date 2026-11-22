# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Rust Resource Surfaces."""

# === Imports === #

# Standard Library Imports
import re
from pathlib import Path

# isort: split

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FileDescriptorSet

# isort: split

# Local Imports
from . import descriptor
from .configuration import Configuration

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SCALARS = {
    1: "f64",
    2: "f32",
    3: "i64",
    4: "u64",
    5: "i32",
    6: "u64",
    7: "u32",
    8: "bool",
    9: "String",
    12: "Vec<u8>",
    13: "u32",
    15: "i32",
    16: "i64",
    17: "i32",
    18: "i64",
}


# === Types === #

# === Classes === #

# === Public Functions === #


def exports(root: Path, descriptors: FileDescriptorSet, *, configuration: Configuration | None = None) -> None:
    configuration = configuration or Configuration()
    methods = descriptor.table(descriptors)
    resource_names = {
        "".join(part.title().replace("_", "") for part in method["path"][:length])
        for method in methods
        if method["face"] == "client"
        for length in range(1, len(method["path"]))
    }
    public_paths = descriptor.public_paths(descriptors, namespace=configuration.namespace)
    client_paths = descriptor.public_paths(descriptors, namespace=configuration.namespace, reserved_names=resource_names)
    requests = descriptor.request_names(descriptors)

    audiences = descriptor.payload_audiences(descriptors, library=configuration.library)
    groups: dict[str, list[tuple[tuple[str, ...], str]]] = {"shared": [], "client": [], "plugin": []}
    for file in descriptors.file:
        if not file.package.startswith((f"{configuration.namespace}.", "harmos.")):
            continue
        if file.name.endswith("guest.proto"):
            continue
        if file.package.startswith("harmos.plugin."):
            continue
        for definition in [*file.message_type, *file.enum_type]:
            full = f"{file.package}.{definition.name}"
            if full in requests:
                continue
            if definition.name in {"Request", "Response"}:
                continue
            if definition.name.startswith("Child"):
                continue
            source = (
                message(full)
                .replace(f"{configuration.namespace}_protocol::model", "crate::wire::model")
                .replace(f"{configuration.namespace}_protocol::host", "crate::wire::host")
                .replace(f"{configuration.namespace}_protocol::plugin", "crate::wire::plugin")
            )
            if configuration.namespace == "harmos":
                source = (
                    message(full)
                    .replace("harmos_protocol::model", "crate::wire::shared")
                    .replace("harmos_protocol::host", "crate::wire::host")
                )
            groups_for_type = (
                {"plugin"}
                if ".plugin." in file.package
                else {"shared"}
                if ".model." in file.package or len(audiences.get(full, set())) > 1
                else audiences.get(full, {"client"})
            )
            for group in groups_for_type:
                paths = client_paths if group == "client" else public_paths
                prefix = tuple(paths[full].split(".")[:-1])
                if group == "plugin" and prefix[:1] == ("host",):
                    prefix = (configuration.namespace, *prefix)
                groups[group].append((prefix, f"pub use {source};"))
            if isinstance(definition, DescriptorProto) and (
                any(not nested.options.map_entry for nested in definition.nested_type)
                or definition.enum_type
                or any(field.HasField("oneof_index") and not field.proto3_optional for field in definition.field)
            ):
                parent = source.rsplit("::", 1)[0]
                for group in groups_for_type:
                    paths = client_paths if group == "client" else public_paths
                    prefix = tuple(paths[full].split(".")[:-1])
                    if group == "plugin" and prefix[:1] == ("host",):
                        prefix = (configuration.namespace, *prefix)
                    module = parent + "::" + descriptor.snake(definition.name)
                    groups[group].append((prefix, f"pub use {module};"))
    for group, entries in groups.items():
        lines = []
        opened: tuple[str, ...] = ()
        for prefix, declaration in sorted(entries):
            shared = 0
            while shared < min(len(opened), len(prefix)) and opened[shared] == prefix[shared]:
                shared += 1
            lines += ["}" for _ in opened[shared:]]
            lines += [f"pub mod {part} {{" for part in prefix[shared:]]
            lines.append(declaration)
            opened = prefix
        lines += ["}" for _ in opened]
        target = root / "sdk/rust/src" / ("exports.rs" if group == "shared" else f"{group}/types.rs")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("\n".join(["// @generated from protocol descriptors; do not edit.", *lines, ""]), encoding="utf-8", newline="\n")


def message(full: str) -> str:
    for package, module in [("harmos.host.v1.", "host"), ("harmos.plugin.v1.", "plugin"), ("harmos.model.v1.", "model")]:
        if full.startswith(package):
            return f"harmos_protocol::{module}::" + "::".join(
                [*(descriptor.snake(part) for part in full.removeprefix(package).split(".")[:-1]), full.rsplit(".", 1)[-1]]
            )
    if full.startswith("google.protobuf."):
        return "prost_types::" + full.rsplit(".", 1)[-1]
    parts = full.split(".")
    version = next(index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part))
    parents = [_identifier(descriptor.snake(part)) for part in parts[version + 1 : -1]]
    return descriptor.snake(parts[0]) + "_protocol::" + "::".join([parts[version - 1], *parents, parts[-1]])


def call_fields(methods: list[descriptor.Method], *, configuration: Configuration | None = None) -> list[descriptor.Method]:
    """Keep Selectors Positional and Defaultable Read Filters in Call Options."""
    configuration = configuration or Configuration()
    required = configuration.rust_selectors
    selected: list[descriptor.Method] = []
    for method in methods:
        selectors = required.get(method["request"])
        if selectors is not None:
            method = method.copy()
            method["fields"] = [
                descriptor.Field(**{**field, "optional": field["optional"] or field["name"] not in selectors}) for field in method["fields"]
            ]
        selected.append(method)
    return selected


def client(
    root: Path,
    methods: list[descriptor.Method],
    *,
    target: Path | None = None,
    types_target: Path | None = None,
    configuration: Configuration | None = None,
) -> None:
    if target is None and not methods:
        (root / "sdk/rust/src/client/resources.rs").unlink(missing_ok=True)
        (root / "sdk/rust/src/connection/resources.rs").unlink(missing_ok=True)
        return
    configuration = configuration or Configuration()
    methods = call_fields(methods, configuration=configuration)
    selected: list[descriptor.Method] = []
    for method in methods:
        method = method.copy()
        if method["face"] == "client" and method["request"] == "harmos.host.v1.plugins.SettingsListRequest":
            method["fields"] = [
                descriptor.Field(**{**field, "optional": False}) if field["name"] == "name" else field for field in method["fields"]
            ]
        selected.append(method)
    methods = selected
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    names = {}
    for names_node in nodes:
        names[names_node] = "Root" if not names_node else "".join(part.title().replace("_", "") for part in names_node)

    used = set()
    for used_method in methods:
        for used_key in ("request", "response"):
            used.add(used_method[used_key])

    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    reserved = set(names.values()) | {"Call", "Client", "CallOptions", "ClientTransport", "Watching", "ReflectMessage", "PathAndQuery"}
    reserved.update("".join(part.title().replace("_", "") for part in method["path"]) + "Options" for method in methods)
    message_names = _message_names(used, reserved)

    def field_type(field: descriptor.Field) -> str:
        value = message_names[field["type"]] if field["type"] else SCALARS.get(field["kind"], "i64")
        return f"std::collections::HashMap<String, {value}>" if field["map"] else f"Vec<{value}>" if field["repeated"] else value

    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use harmos::client::{CallOptions, ClientTransport, Watching, action_error, request_id};",
        "use prost_reflect::ReflectMessage;",
        "use tonic::codegen::http::uri::PathAndQuery;",
    ]
    lines += [f"use {message(full)};" for full in sorted(used) if "::" not in message_names[full]]
    for method in methods:
        name = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
        lines += ["#[derive(Default)]", f"pub struct {name} {{"]
        lines_items = []
        for lines_field in method["fields"]:
            if not (lines_field["optional"]):
                continue
            lines_items.append(f"    pub {_identifier(lines_field['name'])}: Option<{field_type(lines_field)}>,")
        lines += lines_items
        lines += ["    pub context: CallOptions,", "}"]
        lines += [f"impl Clone for {name} {{", "    fn clone(&self) -> Self {", "        Self {"]
        for field in method["fields"]:
            if field["optional"]:
                expression = f"self.{_identifier(field['name'])}" + ("" if field["copy"] else ".clone()")
                lines += [f"            {_identifier(field['name'])}: {expression},"]
        lines += [
            "            context: CallOptions { metadata: self.context.metadata.clone(), timeout: self.context.timeout },",
            "        }",
            "    }",
            "}",
        ]
    for node in sorted(nodes):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = []
        for operations_method in methods:
            if tuple(operations_method["path"][:-1]) != node:
                continue
            operations.append(operations_method)

        lines += ["#[derive(Clone)]", f"pub struct {names[node]} {{"]
        if operations:
            lines += ["    client: ClientTransport,"]
        lines += [f"    pub {_identifier(child[-1])}: {names[child]}," for child in children]
        lines += ["}", f"impl {names[node]} {{", "    pub fn new(client: ClientTransport) -> Self {", "        Self {"]
        lines += [f"            {_identifier(child[-1])}: {names[child]}::new(client.clone())," for child in children]
        if operations:
            lines += ["            client,"]
        lines += ["        }", "    }"]
        for method in operations:
            if method["client_streaming"]:
                raise ValueError(
                    f"Generate Rust Client {method['method']}: Declare a Managed Contract for Client Streaming Before Regenerating"
                )
            request, response = (message_names[method[key]] for key in ("request", "response"))
            required = []
            for required_field in method["fields"]:
                if required_field["optional"]:
                    continue
                required.append(required_field)

            parameters = []
            for parameters_field in required:
                parameters.append(
                    f"{_identifier(parameters_field['name'])}: "
                    + ("impl Into<String>" if field_type(parameters_field) == "String" else field_type(parameters_field))
                )

            args = [_identifier(field["name"]) for field in required]
            options = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            if method["server_streaming"]:
                result = f"Watching<{result}>"
            name = _identifier(method["path"][-1])
            lines += [
                f"    pub async fn {name}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result} , tonic::Status> {{",
                f"        self.{name}_with({', '.join([*args, options + '::default()'])}).await",
                "    }",
            ]
            lines += [
                f"    pub async fn {name}_with(&self, {', '.join([*parameters, 'options: ' + options])}) -> Result<{result} , tonic::Status> {{",
                f"        let value = {request} {{",
            ]
            for group in sorted({field["oneof"] for field in method["fields"] if field["oneof"]}):
                members = []
                for members_field in method["fields"]:
                    if members_field["oneof"] != group:
                        continue
                    members.append(members_field)

                variant = (
                    message(method["request"]).rsplit("::", 1)[0]
                    + "::"
                    + descriptor.snake(method["request"].rsplit(".", 1)[-1])
                    + "::"
                    + "".join(part.title() for part in group.split("_"))
                )
                variants = ", ".join("options." + field["name"] for field in members) + ","
                lines += [f"            {group}: match ({variants}) {{"]
                for index, member in enumerate(members):
                    pattern = ", ".join("Some(value)" if index == other else "None" for other in range(len(members))) + ","
                    lines += [f"                ({pattern}) => Some({variant}::{member['name'].title()}(value)),"]
                empty = ", ".join("None" for _ in members) + ","
                lines += [
                    f"                ({empty}) => None,",
                    f'                _ => return Err(tonic::Status::invalid_argument("Choose At Most One {group} Variant")),',
                    "            },",
                ]
            for field in method["fields"]:
                if field["oneof"]:
                    continue
                name = _identifier(field["name"])
                if name == "request_id":
                    expression = "match options.request_id { Some(value) => value, None => request_id()?, }"
                elif method["request"] == "harmos.host.v1.plugins.SettingsListRequest" and name == "name":
                    expression = "Some(name.into())"
                elif field["optional"]:
                    expression = f"options.{name}"
                    if not field["wire_optional"] and (field["kind"] != 11 or field["repeated"] or field["map"]):
                        expression += ".unwrap_or_default()"
                elif field["kind"] == 11 and not field["repeated"] and not field["map"]:
                    expression = f"Some({name})"
                elif field_type(field) == "String":
                    expression = f"{name}.into()"
                else:
                    expression = name
                if field["kind"] == 14:
                    if field["repeated"]:
                        expression = f"{expression}.into_iter().map(|value| value as i32).collect()"
                    elif field["wire_optional"] and field["optional"]:
                        expression = f"{expression}.map(|value| value as i32)"
                    else:
                        expression = f"{expression} as i32"
                if (
                    field["wire_optional"]
                    and not field["optional"]
                    and field["kind"] != 11
                    and not (method["request"] == "harmos.host.v1.plugins.SettingsListRequest" and name == "name")
                ):
                    expression = f"Some({expression})"
                lines += [f"            {name}," if expression == name else f"            {name}: {expression},"]
            lines += ["        };", "        let request = options.context.request(value);"]
            path = f"/{method['package']}.{method['service']}/{method['method']}"
            call = "watch_known" if method["server_streaming"] else "unary"
            lines += [
                f'        let response = self.client.{call}::<{request}, {response}>(PathAndQuery::from_static({path!r}), request, {response}::default().descriptor()).await.map_err(|error| action_error("{".".join(method["path"])}", error))?;'.replace(
                    repr(path), '"' + path + '"'
                )
            ]
            if method["server_streaming"]:
                lines += [f'        let response = response.with_action("{".".join(method["path"])}");']
                if payload:
                    field = payload["name"]
                    value = f"response.{field}"
                    if payload["kind"] == 11 and not payload["repeated"]:
                        value += f'.ok_or_else(|| tonic::Status::data_loss("Read {method["service"]}.{method["method"]}: Missing Payload"))'
                    else:
                        value = f"Ok({value})"
                    lines += [f"        Ok(response.map(|response| {value}))"]
                else:
                    lines += ["        Ok(response)"]
            else:
                lines += ["        let response = response.into_inner();"]
                if payload:
                    value = f"response.{payload['name']}"
                    if payload["kind"] == 11 and not payload["repeated"]:
                        lines += [
                            f'        {value}.ok_or_else(|| tonic::Status::data_loss("Read {method["service"]}.{method["method"]}: Missing Payload"))'
                        ]
                    else:
                        lines += [f"        Ok({value})"]
                else:
                    lines += ["        Ok(response)"]
            lines += ["    }"]
        lines += ["}"]
    destination = target or root / "sdk/rust/src/connection/resources.rs"
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    if types_target is not None:
        responses = sorted({method["response"] for method in methods if not method["response"].startswith(("harmos.", "google."))})
        exports = ["// @generated from protocol descriptors; do not edit.", *[f"pub use {message(response)};" for response in responses]]
        types_target.write_text("\n".join(exports) + "\n", encoding="utf-8", newline="\n")

    if target is None:
        exports = [*names.values(), *("".join(part.title().replace("_", "") for part in method["path"]) + "Options" for method in methods)]
        (root / "sdk/rust/src/client/resources.rs").write_text(
            "// @generated from protocol descriptors; do not edit.\npub use crate::connection::resources::{"
            + ", ".join(sorted(exports))
            + "};\n",
            encoding="utf-8",
            newline="\n",
        )


def context(root: Path, methods: list[descriptor.Method]) -> None:
    lines = ["// @generated from the protocol descriptor table; do not edit.", "use crate::{Message, OperationRequest};"]
    messages = sorted(
        {method[field] for method in methods for field in ("request", "response")}
        | {"harmos.model.v1.SettingDeclaration", "harmos.model.v1.SettingsDeclarations"}
    )

    lines += ["use prost_types::FileDescriptorSet;"]
    lines += [f"use {message(full)};" for full in messages]

    for full in messages:
        lines += [
            f"impl Message for {full.rsplit('.', 1)[-1]} {{",
            f'    fn name() -> String {{ "{full}".into() }}',
            "    fn types() -> FileDescriptorSet {",
            f'        <FileDescriptorSet as prost::Message>::decode(&include_bytes!("types/{full}.bin")[..]).expect("generated context wrapper")',
            "    }",
            "}",
        ]
    for method in methods:
        lines += [
            f"impl OperationRequest for {method['request'].rsplit('.', 1)[-1]} {{",
            f"    type Response = {method['response'].rsplit('.', 1)[-1]};",
            f'    const ID: &\'static str = "{method["package"]}.{method["service"]}/{method["method"]}";',
            f"    const WATCH: bool = {str(method['server_streaming']).lower()};",
            "}",
        ]
    (root / "sdk/rust/src/context/contracts.rs").parent.mkdir(parents=True, exist_ok=True)
    (root / "sdk/rust/src/context/contracts.rs").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def host(root: Path, methods: list[descriptor.Method], *, target: Path | None = None, configuration: Configuration | None = None) -> None:
    configuration = configuration or Configuration()
    methods = call_fields(methods, configuration=configuration)

    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: descriptor.Field) -> str:
        value = message_names[field["type"]] if field["type"] else SCALARS.get(field["kind"], "i64")
        return f"std::collections::HashMap<String, {value}>" if field["map"] else f"Vec<{value}>" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods if method["surface"] for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
    used = set()
    for used_method in methods:
        for used_key in ("request", "response"):
            if used_key == "response" and not used_method["surface"]:
                continue
            used.add(used_method[used_key])

    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    reserved = {"Context", "Error", "Result", "Plugin", "OperationPolicy", "NonZeroUsize", "HostOperation", "HostRoot", *names.values()}
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        reserved.update({f"Host{name}", f"Host{name}Options"})
    message_names = _message_names(used, reserved)
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use crate::{Context, Error, Result, OperationPolicy};",
        "use crate::plugin::{Dependency, Registrar, Registration};",
        "use std::num::NonZeroUsize;",
    ]
    surface_types = {method[key] for method in methods if method["surface"] for key in ("request", "response")}
    surface_types.update(
        field["type"]
        for method in methods
        if method["surface"]
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    lines += [
        f"use {message(full)};" for full in sorted(used) if "::" not in message_names[full] and (target is None or full in surface_types)
    ]
    lines += ["pub enum HostOperation {"]
    lines += ["    " + "".join(title(part) for part in method["path"]) + "," for method in methods]
    lines += ["}", "impl HostOperation {", f"    pub const ALL: [Self; {len(methods)}] = ["]
    lines += ["        Self::" + "".join(title(part) for part in method["path"]) + "," for method in methods]
    lines += [
        "    ];",
        "}",
        "impl Registration for HostOperation {",
        "    fn register(self, ctx: &mut Registrar<'_>) -> Result<()> {",
        "        match self {",
    ]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += [f"            HostOperation::{name} => ctx.register(Dependency::<{message_names[method['request']]}>::new()),"]
    lines += [
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
    lines_items = []
    for lines_node in nodes:
        if len(lines_node) != 1:
            continue
        lines_items.append(f"    pub {lines_node[-1]}: {names[lines_node]},")
    lines += lines_items
    lines += ["}", "impl HostRoot {", "    fn new(context: Context) -> Self {", "        Self {"]
    lines_items = []
    for lines_node in nodes:
        if len(lines_node) != 1:
            continue
        lines_items.append(f"            {lines_node[-1]}: {names[lines_node]}::new(context.clone()),")
    lines += lines_items
    lines += ["        }", "    }", "}"]
    for method in methods:
        if not method["surface"]:
            continue
        name = "".join(title(part) for part in method["path"])
        lines += ["#[derive(Default)]", f"pub struct Host{name}Options {{"]
        lines_items = []
        for lines_field in method["fields"]:
            if not (lines_field["optional"]):
                continue
            lines_items.append(f"    pub {_identifier(lines_field['name'])}: Option<{field_type(lines_field)}>,")
        lines += lines_items
        lines += (["    pub policy: Option<OperationPolicy>,"] if not method["server_streaming"] else []) + ["}"]
        if method["server_streaming"]:
            q, r = (message_names[method[key]] for key in ("request", "response"))
            payload = method["payload_field"]
            result = field_type(payload) if payload else r
            lines += [
                f"pub struct Host{name} {{",
                f"    incoming: crate::context::Watching<{r}>,",
                "}",
                f"impl Host{name} {{",
                f"    pub async fn next(&mut self) -> Result<Option<{result}>> {{",
                "        let Some(response) = self.incoming.next().await? else {",
                "            return Ok(None);",
                "        };",
            ]
            if payload and payload["kind"] == 11 and not payload["repeated"]:
                lines += [
                    f'        response.{payload["name"]}.map(Some).ok_or_else(|| Error::Invalid("Read {name}: Missing Payload".into()))'
                ]
            else:
                lines += [f"        Ok(Some(response{'.' + payload['name'] if payload else ''}))"]
            lines += ["    }", "}"]
    for node in nodes:
        lines += [
            "#[derive(Clone)]",
            f"pub struct {names[node]} {{",
            *(["    context: Context,"] if any(tuple(method["path"][:-1]) == node for method in methods) else []),
            *[f"    pub {_identifier(child[-1])}: {names[child]}," for child in nodes if child[:-1] == node],
            "}",
            f"impl {names[node]} {{",
            "    fn new(context: Context) -> Self { Self {",
            *[f"        {_identifier(child[-1])}: {names[child]}::new(context.clone())," for child in nodes if child[:-1] == node],
            *(["        context,"] if any(tuple(method["path"][:-1]) == node for method in methods) else []),
            "    } }",
        ]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            name = "".join(title(part) for part in method["path"])
            q, r = (message_names[method[key]] for key in ("request", "response"))
            required = []
            for required_field in method["fields"]:
                if required_field["optional"]:
                    continue
                required.append(required_field)

            parameters = [f"{_identifier(field['name'])}: {field_type(field)}" for field in required]
            args = [_identifier(field["name"]) for field in required]
            if method["server_streaming"]:
                parameters += ["max_item_bytes: NonZeroUsize"]
                args += ["max_item_bytes"]
            payload = method["payload_field"]
            result = f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else r
            action = _identifier(method["path"][-1])
            lines += [
                f"    pub async fn {action}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result}> {{",
                f"        self.{action}_with({', '.join([*args, 'Host' + name + 'Options::default()'])}).await",
                "    }",
                f"    pub async fn {action}_with(&self, {', '.join([*parameters, 'options: Host' + name + 'Options'])}) -> Result<{result}> {{",
                f"        let request = {q} {{",
            ]
            for group in sorted({field["oneof"] for field in method["fields"] if field["oneof"]}):
                members = []
                for members_field in method["fields"]:
                    if members_field["oneof"] != group:
                        continue
                    members.append(members_field)

                variant = (
                    message(method["request"]).rsplit("::", 1)[0]
                    + "::"
                    + descriptor.snake(method["request"].rsplit(".", 1)[-1])
                    + "::"
                    + title(group)
                )
                selected = ", ".join("options." + field["name"] for field in members)
                lines += [f"            {group}: match ({selected}) {{"]
                for index, member in enumerate(members):
                    pattern = ", ".join("Some(value)" if index == other else "None" for other in range(len(members))) + ","
                    lines += [f"                ({pattern}) => Some({variant}::{title(member['name'])}(value)),"]
                lines += [
                    f'                _ => return Err(Error::Invalid("Choose Exactly One {group} Variant".into())),',
                    "            },",
                ]
            for field in method["fields"]:
                if field["oneof"]:
                    continue
                value = (
                    f"options.{_identifier(field['name'])}"
                    if field["optional"]
                    else f"Some({_identifier(field['name'])})"
                    if field["kind"] == 11 and not field["map"] and not field["repeated"]
                    else field["name"]
                )
                if field["name"] == "request_id":
                    value = "match options.request_id { Some(value) => value, None => harmos::client::request_id()?, }"
                elif field["optional"] and not field["wire_optional"] and (field["kind"] != 11 or field["repeated"] or field["map"]):
                    value += ".unwrap_or_default()"
                if field["kind"] == 14:
                    if field["repeated"]:
                        value = f"{value}.into_iter().map(|value| value as i32).collect()"
                    elif field["wire_optional"] and field["optional"]:
                        value = f"{value}.map(|value| value as i32)"
                    else:
                        value = f"{value} as i32"
                lines += [
                    f"            {_identifier(field['name'])},"
                    if value == field["name"]
                    else f"            {_identifier(field['name'])}: {value},"
                ]
            lines += ["        };"]
            if method["server_streaming"]:
                if not any(field["optional"] for field in method["fields"]):
                    lines += [f"        let Host{name}Options {{}} = options;"]
                lines += [
                    f"        Ok(Host{name} {{ incoming: self.context.watch(request, max_item_bytes).await? }})",
                ]
            else:
                lines += [f"        let response: {r} = self.context.call_with(request, options.policy).await?;"]
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    lines += [f'        response.{payload["name"]}.ok_or_else(|| Error::Invalid("Read {name}: Missing Payload".into()))']
                else:
                    lines += [f"        Ok(response{'.' + payload['name'] if payload else ''})"]
            lines += ["    }"]
        lines += ["}"]
    if target is not None:
        source = "\n".join(lines)
        start = source.index("impl Registration for HostOperation {")
        end = source.index("#[derive(Clone)]", start)
        source = source[:start] + source[end:]
        source = source.replace("use crate::{Context, Error, Result, OperationPolicy};", "use harmos::{Error, Result, OperationPolicy};")
        source = source.replace("use crate::plugin::{Dependency, Registrar, Registration};", "")
        source = re.sub(r"(?<![:\w])Context\b", "harmos::plugin::Context", source)
        source = source.replace("crate::context::Watching", "harmos::plugin::Watching")
        source += "\n#[derive(Clone)]\npub struct PluginContext { pub host: HostRoot }\nimpl PluginContext {\n"
        source += "pub fn new(context: harmos::plugin::Context) -> Self { Self { host: HostRoot::new(context) } }\n}\n"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(source + "\n", encoding="utf-8", newline="\n")
        return
    target = root / "sdk/rust/src/plugin/host.rs"
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    exports = ["HostOperation", "HostRoot", *names.values()]
    for method in methods:
        if not method["surface"]:
            continue
        name = "".join(title(part) for part in method["path"])
        exports += [f"Host{name}Options"]
        if method["server_streaming"]:
            exports += [f"Host{name}"]
    (target.parent / "host_exports.rs").write_text(
        "// @generated from protocol descriptors; do not edit.\npub use self::host::{" + ", ".join(sorted(exports)) + "};\n",
        encoding="utf-8",
        newline="\n",
    )


# === Private Functions === #


def _message_names(used: set[str], reserved: set[str]) -> dict[str, str]:
    names = {full: full.rsplit(".", 1)[-1] for full in used}
    duplicates = reserved | {name for name in names.values() if list(names.values()).count(name) > 1}
    return {full: message(full) if name in duplicates else name for full, name in names.items()}


def _identifier(name: str) -> str:
    if name in {"self", "Self", "super", "crate"}:
        return name + "_"
    return (
        "r#" + name
        if name
        in {
            "gen",
            "type",
            "while",
            "const",
            "crate",
            "await",
            "true",
            "final",
            "struct",
            "impl",
            "mut",
            "override",
            "match",
            "return",
            "use",
            "as",
            "box",
            "if",
            "priv",
            "abstract",
            "virtual",
            "become",
            "try",
            "continue",
            "unsafe",
            "mod",
            "yield",
            "pub",
            "static",
            "loop",
            "where",
            "do",
            "in",
            "async",
            "fn",
            "typeof",
            "macro",
            "let",
            "move",
            "trait",
            "dyn",
            "unsized",
            "false",
            "for",
            "else",
            "ref",
            "super",
            "break",
            "enum",
            "extern",
        }
        else name
    )


# === Entry Point === #
