# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Rust Resources, Operation Contracts and Payload Exports."""

# === Imports === #

# Standard Library Imports
import re
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import DescriptorProto, FileDescriptorSet

# Local Imports
from harmos_tools.build.resources.descriptor import Field, Method, payload_audiences, snake, table

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


# === Public Functions === #


def exports(root: Path, descriptors: FileDescriptorSet) -> None:
    requests = {method["request"] for method in table(descriptors)}
    audiences = payload_audiences(descriptors)
    groups: dict[str, list[str]] = {"shared": [], "client": [], "plugin": []}
    for file in descriptors.file:
        if not file.package.startswith("harmos.") or file.name.endswith("guest.proto") or ".plugin." in file.package:
            continue
        for definition in [*file.message_type, *file.enum_type]:
            full = f"{file.package}.{definition.name}"
            if full in requests or definition.name in {"Request", "Response"} or definition.name.startswith("Child"):
                continue
            source = (
                message_type(full)
                .replace("harmos_protocol::model", "crate::wire::shared")
                .replace("harmos_protocol::host", "crate::wire::host")
            )
            groups_for_type = (
                {"shared"} if ".model." in file.package or len(audiences.get(full, set())) > 1 else audiences.get(full, {"client"})
            )
            for group in groups_for_type:
                groups[group].append(f"pub use {source};")
            if isinstance(definition, DescriptorProto) and (
                any(not nested.options.map_entry for nested in definition.nested_type)
                or definition.enum_type
                or any(field.HasField("oneof_index") and not field.proto3_optional for field in definition.field)
            ):
                parent = source.rsplit("::", 1)[0]
                for group in groups_for_type:
                    groups[group].append(f"pub use {parent}::{snake(definition.name)};")
    for group, lines in groups.items():
        target = root / "sdk/rust/src" / ("exports.rs" if group == "shared" else f"{group}/types.rs")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            "\n".join(["// @generated from protocol descriptors; do not edit.", *sorted(lines), ""]), encoding="utf-8", newline="\n"
        )


def message_type(full: str) -> str:
    for package, module in [("harmos.host.v1.", "host"), ("harmos.plugin.v1.", "plugin"), ("harmos.model.v1.", "model")]:
        if full.startswith(package):
            return f"harmos_protocol::{module}::" + "::".join(full.removeprefix(package).split("."))
    if full.startswith("google.protobuf."):
        return "prost_types::" + full.rsplit(".", 1)[-1]
    parts = full.split(".")
    version = next(index for index, part in enumerate(parts) if re.fullmatch(r"v[0-9]+", part))
    return snake(parts[0]) + "_protocol::" + "::".join([parts[version - 1], *parts[version + 1 :]])


def client(root: Path, methods: list[Method], *, target: Path | None = None) -> None:
    selected: list[Method] = []
    for method in methods:
        method = method.copy()
        if method["face"] == "client" and method["request"] == "harmos.host.v1.plugins.SettingsListRequest":
            method["fields"] = [Field(**{**field, "optional": False}) if field["name"] == "name" else field for field in method["fields"]]
        selected.append(method)
    methods = selected
    nodes = {()}
    for method in methods:
        nodes.update(tuple(method["path"][:length]) for length in range(1, len(method["path"])))
    names = {node: "Root" if not node else "".join(part.title().replace("_", "") for part in node) for node in nodes}
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    message_names = {full: full.rsplit(".", 1)[-1] for full in used}
    reserved = set(names.values()) | {"CallOptions", "ClientTransport", "Watching", "ReflectMessage", "PathAndQuery"}
    duplicates = reserved | {name for name in message_names.values() if list(message_names.values()).count(name) > 1}
    for full in used:
        if message_names[full] in duplicates:
            message_names[full] = message_type(full)

    def field_type(field: Field) -> str:
        value = (
            message_names[field["type"]]
            if field["type"]
            else {
                1: "f64",
                2: "f32",
                8: "bool",
                9: "String",
                12: "Vec<u8>",
                4: "u64",
                13: "u32",
                6: "u64",
                7: "u32",
                5: "i32",
                15: "i32",
                17: "i32",
            }.get(field["kind"], "i64")
        )
        return f"Vec<{value}>" if field["repeated"] else value

    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use harmos::client::{CallOptions, ClientTransport, Watching, action_error, request_id};",
        "use prost_reflect::ReflectMessage;",
        "use tonic::codegen::http::uri::PathAndQuery;",
    ]
    lines += [
        f"use {message_type(full)};" for full in sorted(used) if message_names[full] not in duplicates and "::" not in message_names[full]
    ]

    for method in methods:
        name = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
        lines += ["#[derive(Default)]", f"pub struct {name} {{"]
        lines += [f"    pub {field['name']}: Option<{field_type(field)}>," for field in method["fields"] if field["optional"]]
        lines += ["    pub context: CallOptions,", "}"]
    for node in sorted(nodes):
        children = sorted(child for child in nodes if len(child) == len(node) + 1 and child[:-1] == node)
        operations = [method for method in methods if tuple(method["path"][:-1]) == node]
        lines += ["#[derive(Clone)]", f"pub struct {names[node]} {{"]
        if operations:
            lines += ["    client: ClientTransport,"]
        lines += [f"    pub {child[-1]}: {names[child]}," for child in children]
        lines += ["}", f"impl {names[node]} {{", "    pub fn new(client: ClientTransport) -> Self {", "        Self {"]
        lines += [f"            {child[-1]}: {names[child]}::new(client.clone())," for child in children]
        if operations:
            lines += ["            client,"]
        lines += ["        }", "    }"]
        for method in operations:
            if method["client_streaming"]:
                raise ValueError(
                    f"Generate Rust Client {method['method']}: Managed Contract Required; Correct the source protocol declarations or explicit public exports before regenerating"
                )
            request, response = (message_names[method[key]] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            parameters = [
                f"{field['name']}: " + ("impl Into<String>" if field_type(field) == "String" else field_type(field)) for field in required
            ]
            args = [field["name"] for field in required]
            options = "".join(part.title().replace("_", "") for part in method["path"]) + "Options"
            payload = method["payload_field"]
            result = field_type(payload) if payload else response
            if method["server_streaming"]:
                result = f"Watching<{result}>"
            name = method["path"][-1]
            lines += [
                f"    pub async fn {name}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result}, tonic::Status> {{",
                f"        self.{name}_with({', '.join([*args, options + '::default()'])}).await",
                "    }",
            ]
            lines += [
                f"    pub async fn {name}_with(&self, {', '.join([*parameters, 'options: ' + options])}) -> Result<{result}, tonic::Status> {{",
                f"        let value = {request} {{",
            ]
            for field in method["fields"]:
                name = field["name"]
                if name == "request_id":
                    expression = "match options.request_id { Some(value) => value, None => request_id()?, }"
                elif method["request"] == "harmos.host.v1.plugins.SettingsListRequest" and name == "name":
                    expression = "Some(name.into())"
                elif field["optional"]:
                    expression = f"options.{name}"
                elif field["kind"] == 11 and not field["repeated"]:
                    expression = f"Some({name})"
                elif field["kind"] == 9:
                    expression = f"{name}.into()"
                else:
                    expression = name
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
    if target is not None:
        exports = sorted({method["response"] for method in methods if method["owner"] != "harmos"})
        facade = ["// @generated from protocol descriptors; do not edit."]
        facade += [f"pub use {re.sub(r'^\w+_protocol::(?:host|plugin|model)', 'crate::wire', message_type(name))};" for name in exports]
        (target.parent / "client_types.rs").write_text("\n".join(facade) + "\n", encoding="utf-8", newline="\n")
    if target is None:
        exports = [*names.values(), *("".join(part.title().replace("_", "") for part in method["path"]) + "Options" for method in methods)]
        (root / "sdk/rust/src/client/resources.rs").write_text(
            "// @generated from protocol descriptors; do not edit.\npub use crate::connection::resources::{"
            + ", ".join(sorted(exports))
            + "};\n",
            encoding="utf-8",
            newline="\n",
        )


def context(root: Path, methods: list[Method]) -> None:
    lines = ["// @generated from the protocol descriptor table; do not edit.", "use crate::{Message, OperationRequest};"]
    messages = sorted(
        {method[field] for method in methods for field in ("request", "response")}
        | {"harmos.model.v1.SettingDeclaration", "harmos.model.v1.SettingsDeclarations"}
    )

    lines += ["use prost_types::FileDescriptorSet;"]
    lines += [f"use {message_type(message)};" for message in messages]

    for message in messages:
        lines += [
            f"impl Message for {message.rsplit('.', 1)[-1]} {{",
            f'    fn name() -> String {{ "{message}".into() }}',
            "    fn types() -> FileDescriptorSet {",
            f'        <FileDescriptorSet as prost::Message>::decode(&include_bytes!("types/{message}.bin")[..]).expect("generated context wrapper")',
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


def host(root: Path, methods: list[Method], *, target: Path | None = None) -> None:
    def title(value: str) -> str:
        return "".join(word.title() for word in value.split("_"))

    def field_type(field: Field) -> str:
        value = (
            field["type"].rsplit(".", 1)[-1]
            if field["type"]
            else {1: "f64", 2: "f32", 8: "bool", 9: "String", 12: "Vec<u8>", 4: "u64", 13: "u32"}.get(field["kind"], "i64")
        )
        return f"Vec<{value}>" if field["repeated"] else value

    nodes = sorted({tuple(method["path"][:length]) for method in methods for length in range(1, len(method["path"]))})
    names = {node: "Host" + "".join(title(part) for part in node) + "Resource" for node in nodes}
    used = {method[key] for method in methods for key in ("request", "response")}
    used.update(
        field["type"]
        for method in methods
        for field in [*method["fields"], *([method["payload_field"]] if method["payload_field"] else [])]
        if field["type"]
    )
    lines = [
        "// @generated from protocol descriptors; do not edit.",
        "use crate::{Context, Error, Result, Plugin, OperationPolicy};",
        "use std::num::NonZeroUsize;",
    ]
    lines += [f"use {message_type(full)};" for full in sorted(used)]
    lines += ["pub enum HostOperation {"]
    lines += ["    " + "".join(title(part) for part in method["path"]) + "," for method in methods]
    lines += ["}", "impl Plugin {", "    pub fn requires_host(self, operation: HostOperation) -> Self {", "        match operation {"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += [f"            HostOperation::{name} => self.requires::<{method['request'].rsplit('.', 1)[-1]}>(),"]
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
    lines += [f"    pub {node[-1]}: {names[node]}," for node in nodes if len(node) == 1]
    lines += ["}", "impl HostRoot {", "    fn new(context: Context) -> Self {", "        Self {"]
    lines += [f"            {node[-1]}: {names[node]}::new(context.clone())," for node in nodes if len(node) == 1]
    lines += ["        }", "    }", "}"]
    for method in methods:
        name = "".join(title(part) for part in method["path"])
        lines += ["#[derive(Default)]", f"pub struct Host{name}Options {{"]
        lines += [f"    pub {field['name']}: Option<{field_type(field)}>," for field in method["fields"] if field["optional"]]
        lines += (["    pub policy: Option<OperationPolicy>,"] if not method["server_streaming"] else []) + ["}"]
        if method["server_streaming"]:
            request_type, response_type = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            payload = method["payload_field"]
            result = field_type(payload) if payload else response_type
            lines += [
                f"pub struct Host{name} {{",
                f"    incoming: crate::context::Watching<{response_type}>,",
                "}",
                f"impl Host{name} {{",
                f"    pub async fn next(&mut self) -> Result<Option<{result}>> {{",
                "        let Some(response) = self.incoming.next().await? else {",
                "            return Ok(None);",
                "        };",
            ]
            if payload and payload["kind"] == 11:
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
            *[f"    pub {child[-1]}: {names[child]}," for child in nodes if child[:-1] == node],
            "}",
            f"impl {names[node]} {{",
            "    fn new(context: Context) -> Self { Self {",
            *[f"        {child[-1]}: {names[child]}::new(context.clone())," for child in nodes if child[:-1] == node],
            *(["        context,"] if any(tuple(method["path"][:-1]) == node for method in methods) else []),
            "    } }",
        ]
        for method in (method for method in methods if tuple(method["path"][:-1]) == node):
            name = "".join(title(part) for part in method["path"])
            request_type, response_type = (method[key].rsplit(".", 1)[-1] for key in ("request", "response"))
            required = [field for field in method["fields"] if not field["optional"]]
            parameters = [f"{field['name']}: {field_type(field)}" for field in required]
            args = [field["name"] for field in required]
            if method["server_streaming"]:
                parameters += ["max_item_bytes: NonZeroUsize"]
                args += ["max_item_bytes"]
            payload = method["payload_field"]
            result = f"Host{name}" if method["server_streaming"] else field_type(payload) if payload else response_type
            action = method["path"][-1]
            lines += [
                f"    pub async fn {action}(&self{', ' if parameters else ''}{', '.join(parameters)}) -> Result<{result}> {{",
                f"        self.{action}_with({', '.join([*args, 'Host' + name + 'Options::default()'])}).await",
                "    }",
                f"    pub async fn {action}_with(&self, {', '.join([*parameters, 'options: Host' + name + 'Options'])}) -> Result<{result}> {{",
                f"        let request = {request_type} {{",
            ]
            for field in method["fields"]:
                value = (
                    f"options.{field['name']}" if field["optional"] else f"Some({field['name']})" if field["kind"] == 11 else field["name"]
                )
                lines += [f"            {field['name']}," if value == field["name"] else f"            {field['name']}: {value},"]
            lines += ["        };"]
            if method["server_streaming"]:
                lines += [
                    f"        let Host{name}Options {{}} = options;",
                    f"        Ok(Host{name} {{ incoming: self.context.watch(request, max_item_bytes).await? }})",
                ]
            else:
                lines += [f"        let response: {response_type} = self.context.call_with(request, options.policy).await?;"]
                if payload and payload["kind"] == 11 and not payload["repeated"]:
                    lines += [f'        response.{payload["name"]}.ok_or_else(|| Error::Invalid("Read {name}: Missing Payload".into()))']
                else:
                    lines += [f"        Ok(response{'.' + payload['name'] if payload else ''})"]
            lines += ["    }"]
        lines += ["}"]
    if target is not None:
        source = "\n".join(lines)
        start = source.index("impl Plugin {")
        end = source.index("#[derive(Clone)]", start)
        source = source[:start] + source[end:]
        source = source.replace(
            "use crate::{Context, Error, Result, Plugin, OperationPolicy};", "use harmos::{Error, Result, OperationPolicy};"
        )
        source = re.sub(r"\bContext\b", "harmos::plugin::Context", source)
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
        name = "".join(title(part) for part in method["path"])
        exports += [f"Host{name}Options"]
        if method["server_streaming"]:
            exports += [f"Host{name}"]
    (target.parent / "host_exports.rs").write_text(
        "// @generated from protocol descriptors; do not edit.\npub use host::{" + ", ".join(sorted(exports)) + "};\n",
        encoding="utf-8",
        newline="\n",
    )


# === Private Functions === #

# === Entry Point === #
