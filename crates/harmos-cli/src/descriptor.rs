/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Descriptor Inventory, Caller Contracts and the Method Table Shared by Every Language.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

// Third Party Imports
use prost::Message;
use prost_reflect::{DescriptorPool, Kind, Value};
use prost_types::field_descriptor_proto::{Label, Type};
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};

// Local Imports
use crate::error::{Error, Result};

/* === Constants === */

/// The descriptor set this CLI revision was built with; it decides where library-owned payloads live.
const LIBRARY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../harmos-protocol/src/descriptors.bin"));

const CORRECTION: &str = "Correct the source protocol declarations or explicit public exports before regenerating";

/* === Macros === */

/* === Type Aliases === */

/// Payload names mapped to the faces (`client`, `plugin`) whose calls reach them.
pub(crate) type Audiences = BTreeMap<String, BTreeSet<&'static str>>;

/* === Structs === */

/// One request field projected for generated call signatures.
#[derive(Clone)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: i32,
    pub(crate) type_name: String,
    pub(crate) csharp: String,
    pub(crate) repeated: bool,
    pub(crate) optional: bool,
    pub(crate) map: bool,
    pub(crate) oneof: String,
    pub(crate) wire_optional: bool,
    pub(crate) copy: bool,
}

/// One call as one face sees it.
#[derive(Clone)]
pub(crate) struct Method {
    pub(crate) surface: bool,
    pub(crate) owner: String,
    pub(crate) face: Face,
    pub(crate) service: String,
    pub(crate) method: String,
    pub(crate) package: String,
    pub(crate) path: Vec<String>,
    pub(crate) request: String,
    pub(crate) response: String,
    pub(crate) request_csharp: String,
    pub(crate) response_csharp: String,
    pub(crate) client_streaming: bool,
    pub(crate) server_streaming: bool,
    pub(crate) file: String,
    pub(crate) fields: Vec<Field>,
    pub(crate) payload_field: Option<Field>,
}

/// An ordered descriptor inventory with its validated caller declarations.
pub(crate) struct Schema {
    files: Vec<FileDescriptorProto>,
    callers: BTreeMap<String, String>,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Face {
    Client,
    Context,
    Plugin,
}

/* === Traits === */

/* === Implementations === */

impl Face {
    fn name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Context => "context",
            Self::Plugin => "plugin",
        }
    }
}

impl Method {
    /// The full call identity, `package.Service/Method`.
    pub(crate) fn identity(&self) -> String {
        format!("{}.{}/{}", self.package, self.service, self.method)
    }

    /// Signature fields followed by the payload field, the set whose types a rendering imports.
    pub(crate) fn typed_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().chain(self.payload_field.as_ref())
    }
}

impl Schema {
    /// Read one descriptor set as-is, or merge several by file name in name order.
    pub(crate) fn load(paths: &[PathBuf]) -> Result<Self> {
        let mut files: Vec<FileDescriptorProto> = Vec::new();
        let mut pool = DescriptorPool::new();
        for path in paths {
            let bytes = fs::read(path).map_err(|source| Error::Io {
                action: "Read Descriptors",
                path: path.clone(),
                source,
            })?;
            let set = FileDescriptorSet::decode(&bytes[..])
                .map_err(|error| Error::Input(format!("Read Descriptors {}: {error}; Supply a FileDescriptorSet", path.display())))?;
            pool.decode_file_descriptor_set(&bytes[..]).map_err(|error| {
                Error::Input(format!("Read Descriptors {}: {error}; Supply Resolvable Descriptor Dependencies", path.display()))
            })?;
            for mut file in set.file {
                file.source_code_info = None;
                match files.iter().find(|existing| existing.name == file.name) {
                    Some(existing) if *existing != file => {
                        return Err(Error::Input(format!(
                            "Read Descriptors: Conflicting Descriptor File {}; Supply One Definition per File",
                            file.name()
                        )));
                    }
                    Some(_) => {}
                    None => files.push(file),
                }
            }
        }
        if paths.len() > 1 {
            files.sort_by(|first, second| first.name.cmp(&second.name));
        }
        let callers = callers(&pool, &files)?;
        Ok(Self { files, callers })
    }

    /// The library inventory this CLI revision bundles.
    pub(crate) fn library() -> Result<Self> {
        let set = FileDescriptorSet::decode(LIBRARY).expect("the bundled library descriptors decode");
        let pool = DescriptorPool::decode(LIBRARY).expect("the bundled library descriptors resolve");
        let callers = callers(&pool, &set.file)?;
        Ok(Self { files: set.file, callers })
    }

    pub(crate) fn files(&self) -> &[FileDescriptorProto] {
        &self.files
    }

    /// Validated caller declarations, `package.Service/Method` to `CLIENTS`, `PLUGINS` or `ALL`.
    pub(crate) fn callers(&self) -> &BTreeMap<String, String> {
        &self.callers
    }

    /// Project every versioned call onto the faces its callers declaration admits.
    pub(crate) fn table(&self) -> Result<Vec<Method>> {
        // Record Message Definitions and Their C# Names
        let mut csharp_types = HashMap::new();
        let mut definitions = HashMap::new();
        for file in &self.files {
            let namespace = file
                .options
                .as_ref()
                .map(|options| options.csharp_namespace().to_owned())
                .filter(|namespace| !namespace.is_empty())
                .unwrap_or_else(|| file.package().split('.').map(capitalize).collect::<Vec<_>>().join("."));
            record_messages(&file.message_type, &format!(".{}", file.package()), &namespace, &mut csharp_types, &mut definitions);
            for enumeration in &file.enum_type {
                csharp_types.insert(format!(".{}.{}", file.package(), enumeration.name()), format!("{namespace}.{}", enumeration.name()));
            }
        }
        let field = |value: &FieldDescriptorProto| {
            let definition = definitions.get(value.type_name());
            let map = definition.is_some_and(|definition| definition.options.as_ref().is_some_and(|options| options.map_entry()));
            let item = if map { &definition.expect("map entries have definitions").field[1] } else { value };
            Field {
                name: value.name().to_owned(),
                kind: item.r#type() as i32,
                type_name: item.type_name().trim_start_matches('.').to_owned(),
                csharp: csharp_types.get(item.type_name()).cloned().unwrap_or_default(),
                repeated: value.label() == Label::Repeated && !map,
                optional: value.proto3_optional() || value.name() == "request_id",
                map,
                oneof: String::new(),
                wire_optional: value.proto3_optional(),
                copy: copyable(value, &definitions, &BTreeSet::new()),
            }
        };

        // Project Each Call Onto Its Faces
        let mut methods = Vec::new();
        let mut leaves = BTreeSet::new();
        for file in &self.files {
            let parts: Vec<&str> = file.package().split('.').collect();
            let Some(version) = parts.iter().position(|part| is_version(part)) else {
                continue;
            };
            if version == 0 {
                continue;
            }
            let boundary = &parts[..version];
            let executor = boundary[boundary.len() - 1];
            let owner =
                if ["host", "plugin", "model"].contains(&executor) { boundary[..boundary.len() - 1].join(".") } else { boundary.join(".") };
            for service in &file.service {
                for method in &service.method {
                    let response = definition(&definitions, method.output_type())?;
                    let payload = if response.field.len() == 1 { Some(&response.field[0]) } else { None };
                    let mut path: Vec<String> = parts[version + 1..].iter().map(|part| (*part).to_owned()).collect();
                    path.push(snake(service.name()));
                    path.push(snake(method.name()));
                    let identity = format!("{}.{}/{}", file.package(), service.name(), method.name());
                    let declared = self.callers.get(&identity);
                    let audience = declared.map_or("CLIENTS", String::as_str);
                    let faces = match executor {
                        "plugin" if declared.is_some() => vec![Face::Context],
                        "plugin" => vec![Face::Plugin],
                        _ => [(Face::Client, ["CLIENTS", "ALL"]), (Face::Context, ["PLUGINS", "ALL"])]
                            .into_iter()
                            .filter(|(_, allowed)| allowed.contains(&audience))
                            .map(|(face, _)| face)
                            .collect(),
                    };
                    for face in faces {
                        let request = definition(&definitions, method.input_type())?;
                        let mut fields: Vec<Field> = request.field.iter().map(field).collect();
                        for (item, value) in fields.iter_mut().zip(&request.field) {
                            if let Some(index) = value.oneof_index
                                && !value.proto3_optional()
                            {
                                item.oneof = request.oneof_decl[index as usize].name().to_owned();
                                item.optional = true;
                            }
                        }
                        let mut leaf = vec![face.name().to_owned()];
                        leaf.extend(path.iter().cloned());
                        if !leaves.insert(leaf) {
                            return Err(collision(&path));
                        }
                        methods.push(Method {
                            surface: identity != "harmos.plugin.v1.Subscriptions/Subscribe",
                            owner: owner.clone(),
                            face,
                            service: service.name().to_owned(),
                            method: method.name().to_owned(),
                            package: file.package().to_owned(),
                            path: path.clone(),
                            request: method.input_type().trim_start_matches('.').to_owned(),
                            response: method.output_type().trim_start_matches('.').to_owned(),
                            request_csharp: csharp_types.get(method.input_type()).cloned().unwrap_or_default(),
                            response_csharp: csharp_types.get(method.output_type()).cloned().unwrap_or_default(),
                            client_streaming: method.client_streaming(),
                            server_streaming: method.server_streaming(),
                            file: file.name().to_owned(),
                            fields,
                            payload_field: payload.map(field),
                        });
                    }
                }
            }
        }

        // Refuse Calls That Sit Below Another Call
        for leaf in &leaves {
            if (2..leaf.len()).any(|length| leaves.contains(&leaf[..length].to_vec())) {
                return Err(collision(&leaf[1..]));
            }
        }
        Ok(methods)
    }

    /// Requests stay behind their generated calls; host requests are never public payloads.
    pub(crate) fn request_names(&self) -> Result<BTreeSet<String>> {
        let mut names: BTreeSet<String> = self.table()?.into_iter().map(|method| method.request).collect();
        for file in &self.files {
            if !file.package().starts_with("harmos.host.") {
                continue;
            }
            for message in &file.message_type {
                if message.name().ends_with("Request") {
                    names.insert(format!("{}.{}", file.package(), message.name()));
                }
            }
        }
        Ok(names)
    }

    /// Keep type names; place colliding or reserved names under their unversioned package path.
    pub(crate) fn public_paths(&self, namespace: &str, reserved_names: &BTreeSet<String>) -> BTreeMap<String, String> {
        let owned =
            |file: &&FileDescriptorProto| file.package().starts_with(&format!("{namespace}.")) || file.package().starts_with("harmos.");
        let definitions: Vec<(&FileDescriptorProto, &str)> = self
            .files
            .iter()
            .filter(owned)
            .flat_map(|file| {
                let messages = file.message_type.iter().map(DescriptorProto::name);
                let enumerations = file.enum_type.iter().map(|enumeration| enumeration.name());
                messages.chain(enumerations).map(move |name| (file, name))
            })
            .collect();
        let mut packages: HashMap<&str, BTreeSet<&str>> = HashMap::new();
        for (file, name) in &definitions {
            packages.entry(name).or_default().insert(file.package());
        }
        let reserved =
            ["Call", "Client", "ClientRoot", "Context", "Failure", "OperationError", "HostOperation", "HostRoot", "Plugin", "Window"];
        let mut paths = BTreeMap::new();
        for (file, name) in definitions {
            let parts: Vec<&str> = file.package().split('.').collect();
            let mut package: Vec<&str> = parts[1..].iter().copied().filter(|part| !is_version(part)).collect();
            if parts[0] != namespace {
                package.insert(0, parts[0]);
            }
            let colliding = packages[name].len() > 1 || reserved.contains(&name) || reserved_names.contains(name);
            let mut path: Vec<&str> = if colliding { package } else { Vec::new() };
            path.push(name);
            paths.insert(format!("{}.{name}", file.package()), path.join("."));
        }
        paths
    }

    /// Which faces reach each payload; library payloads keep the homes the library itself gives them.
    pub(crate) fn payload_audiences(&self, library: Option<&Self>) -> Result<Audiences> {
        let mut definitions = HashMap::new();
        for file in &self.files {
            record_definitions(&file.message_type, file.package(), &mut definitions);
        }

        // Read Library Calls From the Library's Own Files
        let mut inventory = Self {
            files: self.files.clone(),
            callers: self.callers.clone(),
        };
        if let Some(library) = library {
            for file in &mut inventory.files {
                let Some(original) =
                    library.files.iter().find(|original| original.name == file.name && original.package().starts_with("harmos."))
                else {
                    continue;
                };
                for identity in identities(file) {
                    inventory.callers.remove(&identity);
                }
                for identity in identities(original) {
                    if let Some(audience) = library.callers.get(&identity) {
                        inventory.callers.insert(identity, audience.clone());
                    }
                }
                *file = original.clone();
            }
        }

        // Walk Each Call's Payload Graph
        let mut audiences = Audiences::new();
        for method in inventory.table()? {
            if method.face == Face::Plugin {
                continue;
            }
            let audience = if method.face == Face::Client { "client" } else { "plugin" };
            let mut pending = vec![method.request.clone(), method.response.clone()];
            let mut visited = BTreeSet::new();
            while let Some(name) = pending.pop() {
                if !visited.insert(name.clone()) {
                    continue;
                }
                audiences.entry(name.clone()).or_default().insert(audience);
                let mut parent = name.rsplit_once('.').map_or(name.as_str(), |(parent, _)| parent);
                while definitions.contains_key(parent) {
                    audiences.entry(parent.to_owned()).or_default().insert(audience);
                    parent = parent.rsplit_once('.').map_or(parent, |(parent, _)| parent);
                }
                if let Some(definition) = definitions.get(name.as_str()) {
                    let types = definition.field.iter().filter(|field| !field.type_name().is_empty());
                    pending.extend(types.map(|field| field.type_name().trim_start_matches('.').to_owned()));
                }
            }
        }
        if let Some(library) = library {
            let homes = library.payload_audiences(None)?;
            for (name, audience) in &mut audiences {
                if name.starts_with("harmos.") {
                    *audience = homes.get(name).cloned().unwrap_or_else(|| BTreeSet::from(["client"]));
                }
            }
        }
        Ok(audiences)
    }
}

/* === Public Functions === */

/// Python's `str.title()`: upper-case each letter that follows a non-letter, lower-case the rest.
pub(crate) fn title(value: &str) -> String {
    let mut cased = false;
    let mut result = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_alphabetic() {
            if cased {
                result.extend(character.to_lowercase());
            } else {
                result.extend(character.to_uppercase());
            }
            cased = true;
        } else {
            result.push(character);
            cased = false;
        }
    }
    result
}

/// Join `snake_case` words as `CamelCase`, titling each word.
pub(crate) fn camel(value: &str) -> String {
    value.split('_').map(title).collect()
}

/// Upper-case the first character only.
pub(crate) fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// `CamelCase` to `snake_case`, splitting acronyms before their last capital.
pub(crate) fn snake(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut acronyms = Vec::with_capacity(characters.len());
    for (index, character) in characters.iter().enumerate() {
        let split = index > 0
            && character.is_ascii_uppercase()
            && characters[index - 1].is_ascii_uppercase()
            && characters.get(index + 1).is_some_and(char::is_ascii_lowercase);
        if split {
            acronyms.push('_');
        }
        acronyms.push(*character);
    }
    let mut result = String::with_capacity(acronyms.len());
    for (index, character) in acronyms.iter().enumerate() {
        let split = index > 0
            && character.is_ascii_uppercase()
            && (acronyms[index - 1].is_ascii_lowercase() || acronyms[index - 1].is_ascii_digit());
        if split {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

/// A package segment such as `v1`.
pub(crate) fn is_version(part: &str) -> bool {
    part.len() > 1 && part.starts_with('v') && part[1..].bytes().all(|byte| byte.is_ascii_digit())
}

/* === Private Functions === */

fn callers(pool: &DescriptorPool, files: &[FileDescriptorProto]) -> Result<BTreeMap<String, String>> {
    // Select Annotated Calls
    let mut methods = Vec::new();
    for file in files {
        let folders: Vec<&str> = file.name().split('/').collect();
        let hosted = folders[..folders.len() - 1].contains(&"host");
        let annotated = file.dependency.iter().any(|dependency| dependency == "harmos/model/v1/callers.proto");
        if !hosted && !annotated {
            continue;
        }
        for service in &file.service {
            let full = if file.package().is_empty() { service.name().to_owned() } else { format!("{}.{}", file.package(), service.name()) };
            for method in &service.method {
                methods.push((format!("{full}/{}", method.name()), full.clone(), method.name().to_owned()));
            }
        }
    }
    let Some((first, _, _)) = methods.first() else {
        return Ok(BTreeMap::new());
    };

    // Resolve the Shared Option Schema
    let Some(extension) = pool.get_extension_by_name("harmos.model.v1.callers") else {
        return Err(Error::Contract(format!("Protocol Callers: {first} Has No Callers Declaration; {CORRECTION}")));
    };
    let Kind::Enum(enumeration) = extension.kind() else {
        return Err(Error::Contract(format!("Protocol Callers: Option Must Be an Enum; {CORRECTION}")));
    };

    // Require One Declared Audience per Call
    let mut result = BTreeMap::new();
    for (identity, service, name) in methods {
        let refusal = || Error::Contract(format!("Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL; {CORRECTION}"));
        let method = pool
            .get_service_by_name(&service)
            .and_then(|service| service.methods().find(|method| method.name() == name))
            .expect("the pool holds every inventory call");
        let options = method.options();
        let number = match options.get_extension(&extension).as_ref() {
            Value::EnumNumber(number) => *number,
            _ => 0,
        };
        let allowed = enumeration.get_value(number).ok_or_else(refusal)?;
        let declared = options.has_extension(&extension);
        let valid = ["CLIENTS", "PLUGINS", "ALL"].contains(&allowed.name());
        if !declared || !valid {
            return Err(refusal());
        }
        result.insert(identity, allowed.name().to_owned());
    }
    Ok(result)
}

fn record_messages<'a>(
    messages: &'a [DescriptorProto], package: &str, namespace: &str, csharp_types: &mut HashMap<String, String>,
    definitions: &mut HashMap<String, &'a DescriptorProto>,
) {
    for message in messages {
        let full = format!("{package}.{}", message.name());
        csharp_types.insert(full.clone(), format!("{namespace}.{}", message.name()));
        definitions.insert(full.clone(), message);
        for enumeration in &message.enum_type {
            csharp_types
                .insert(format!("{full}.{}", enumeration.name()), format!("{namespace}.{}.Types.{}", message.name(), enumeration.name()));
        }
        record_messages(&message.nested_type, &full, &format!("{namespace}.{}.Types", message.name()), csharp_types, definitions);
    }
}

fn record_definitions<'a>(messages: &'a [DescriptorProto], package: &str, definitions: &mut HashMap<String, &'a DescriptorProto>) {
    for message in messages {
        let name = format!("{package}.{}", message.name());
        record_definitions(&message.nested_type, &name, definitions);
        definitions.insert(name, message);
    }
}

fn definition<'a>(definitions: &HashMap<String, &'a DescriptorProto>, name: &str) -> Result<&'a DescriptorProto> {
    definitions
        .get(name)
        .copied()
        .ok_or_else(|| Error::Input(format!("Read Descriptors: Missing Definition {name}; Supply Descriptors With Imports")))
}

fn copyable(value: &FieldDescriptorProto, definitions: &HashMap<String, &DescriptorProto>, pending: &BTreeSet<String>) -> bool {
    if value.label() == Label::Repeated || matches!(value.r#type(), Type::String | Type::Bytes) {
        return false;
    }
    if value.r#type() != Type::Message {
        return true;
    }
    if pending.contains(value.type_name()) {
        return false;
    }
    let Some(definition) = definitions.get(value.type_name()) else {
        return false;
    };
    let mut nested = pending.clone();
    nested.insert(value.type_name().to_owned());
    definition.field.iter().all(|child| copyable(child, definitions, &nested))
}

fn identities(file: &FileDescriptorProto) -> Vec<String> {
    let services = file.service.iter();
    services
        .flat_map(|service| service.method.iter().map(move |method| format!("{}.{}/{}", file.package(), service.name(), method.name())))
        .collect()
}

fn collision(path: &[String]) -> Error {
    Error::Contract(format!(
        "Generate Resource Bindings: Client Resource Collision: {}; Correct the Conflicting Protocol Paths Before Regenerating",
        path.join(".")
    ))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_python_reference() {
        assert_eq!(snake("TraceEvents"), "trace_events");
        assert_eq!(snake("HTTPServer"), "http_server");
        assert_eq!(snake("ABcDEf"), "a_bc_d_ef");
        assert_eq!(snake("Value2Name"), "value2_name");
        assert_eq!(title("by_name"), "By_Name");
        assert_eq!(title("x2y"), "X2Y");
        assert_eq!(camel("trace_events"), "TraceEvents");
        assert_eq!(capitalize("harmos"), "Harmos");
        assert!(is_version("v12"));
        assert!(!is_version("v") && !is_version("version"));
    }
}
