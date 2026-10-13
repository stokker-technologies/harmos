/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
};

// Third Party Imports
use prost_reflect::{
    DescriptorPool,
    prost::Message as _,
    prost_types::{
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
        field_descriptor_proto::{Label, Type},
    },
};

// Local Imports
use crate::operations::Contract;

/* === Constants === */
const MAX_METHODS: usize = 4096;
const MAX_DESCRIPTOR_BYTES: usize = 16 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Artifacts {
    files: BTreeMap<String, String>,
    descriptors: Vec<u8>,
    contracts: Vec<u8>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema: u32,
    #[serde(deserialize_with = "bounded_contracts")]
    contracts: Vec<Contract>,
}

struct MetadataWriter(Vec<u8>);

struct ContractVisitor;
/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Descriptor(#[from] super::DescriptorError),
    #[error("conflicting generated protobuf file: {0}")]
    Collision(String),
    #[error("protobuf artifacts exceed their bounded export limit")]
    Limit,
    #[error("invalid artifact name")]
    Name,
    #[error("unsupported generated protobuf descriptor")]
    Unsupported,
    #[error("Cargo build script did not supply OUT_DIR")]
    TargetDirectory,
    #[error("invalid canonical contract export: {0}")]
    Json(#[from] serde_json::Error),
    #[error("descriptor and canonical contract exports differ")]
    Incompatible,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/* === Traits === */

/* === Implementations === */
impl std::io::Write for MetadataWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let length = self
            .0
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= MAX_DESCRIPTOR_BYTES)
            .ok_or_else(|| std::io::Error::other("contract export byte limit"))?;
        if length > self.0.capacity() {
            let capacity = length
                .checked_next_power_of_two()
                .unwrap_or(MAX_DESCRIPTOR_BYTES)
                .min(MAX_DESCRIPTOR_BYTES);
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(std::io::Error::other)?;
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'de> serde::de::Visitor<'de> for ContractVisitor {
    type Value = Vec<Contract>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a bounded operation contract catalog")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut input: A,
    ) -> Result<Self::Value, A::Error> {
        use serde::de::Error as _;
        let mut contracts = Vec::new();
        while contracts.len() < MAX_METHODS {
            let Some(contract) = input.next_element::<Contract>()? else {
                return Ok(contracts);
            };
            if contracts.len() == contracts.capacity() {
                let capacity = (contracts.len() + 1).next_power_of_two().min(MAX_METHODS);
                contracts
                    .try_reserve_exact(capacity - contracts.len())
                    .map_err(A::Error::custom)?;
            }
            contracts.push(contract);
        }
        if input.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(
                "operation catalog exceeds its contract limit",
            ));
        }
        Ok(contracts)
    }
}

impl Artifacts {
    pub fn new<'a>(contracts: impl IntoIterator<Item = &'a Contract>) -> Result<Self, ExportError> {
        let mut selected = Vec::new();
        let mut identities = BTreeSet::new();
        for contract in contracts {
            if selected.len() == MAX_METHODS {
                return Err(ExportError::Limit);
            }
            if !identities.insert(contract.operation().id.as_str()) {
                return Err(ExportError::Collision(contract.operation().id.to_string()));
            }
            selected.push(contract);
        }
        selected.sort_unstable_by_key(|contract| contract.operation().id.as_str());
        let metadata = canonical_contracts(&selected)?;
        let mut services: BTreeMap<(&str, &str), Vec<(&str, &Contract)>> = BTreeMap::new();
        for contract in selected {
            let Some(binding) = contract.grpc() else {
                continue;
            };
            services
                .entry((&binding.package, &binding.service))
                .or_default()
                .push((&binding.method, contract));
        }
        let mut files = BTreeMap::<String, FileDescriptorProto>::new();
        let mut bytes = 0usize;
        for ((package, service), mut methods) in services {
            methods.sort_unstable_by_key(|(name, _)| *name);
            let descriptor = super::descriptor(package, service, &methods)?;
            for file in descriptor.parent_pool().files() {
                let name = file.name().to_owned();
                let proto = file.file_descriptor_proto();
                if let Some(previous) = files.get(&name) {
                    if previous != proto {
                        return Err(ExportError::Collision(name));
                    }
                    continue;
                }
                bytes = bytes
                    .checked_add(proto.encoded_len())
                    .ok_or(ExportError::Limit)?;
                if bytes > MAX_DESCRIPTOR_BYTES {
                    return Err(ExportError::Limit);
                }
                files.insert(name, proto.clone());
            }
        }
        let set = FileDescriptorSet {
            file: files.values().cloned().collect(),
        };
        if set.encoded_len() > MAX_DESCRIPTOR_BYTES {
            return Err(ExportError::Limit);
        }
        DescriptorPool::from_file_descriptor_set(set.clone())
            .map_err(super::DescriptorError::from)?;
        let sources = files
            .into_iter()
            .map(|(name, file)| Ok((name, source(&file)?)))
            .collect::<Result<_, ExportError>>()?;
        Ok(Self {
            files: sources,
            descriptors: set.encode_to_vec(),
            contracts: metadata,
        })
    }

    // Read And Revalidate An Exact Export Without Executing Its Producer.
    pub fn read(directory: &Path) -> Result<Self, ExportError> {
        let encoded = read_bounded(&directory.join("contracts.json"))?;
        let contracts = decode_contracts(&encoded)?;
        let artifacts = Self::new(&contracts)?;
        if artifacts.contracts != encoded
            || artifacts.descriptors != read_bounded(&directory.join("descriptors.bin"))?
        {
            return Err(ExportError::Incompatible);
        }
        Ok(artifacts)
    }

    #[cfg(feature = "bindings-python")]
    pub(crate) fn operation_contracts(&self) -> Result<Vec<Contract>, ExportError> {
        decode_contracts(&self.contracts)
    }

    pub fn contracts(&self) -> &[u8] {
        &self.contracts
    }

    pub fn descriptors(&self) -> &[u8] {
        &self.descriptors
    }
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    // Export One Catalog Into The Current Producer Build's Cargo Output Directory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_build(&self, name: &str) -> Result<PathBuf, ExportError> {
        let directory = std::env::var_os("OUT_DIR").ok_or(ExportError::TargetDirectory)?;
        self.export(Path::new(&directory), name)
    }

    pub fn export(&self, output_directory: &Path, name: &str) -> Result<PathBuf, ExportError> {
        if name.is_empty()
            || name.len() > 255
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ExportError::Name);
        }
        let root = output_directory.join("grpc").join(name);
        std::fs::create_dir_all(&root)?;
        let previous = match std::fs::read_to_string(root.join("manifest.txt")) {
            Ok(manifest) => manifest,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let stale: Vec<_> = previous
            .lines()
            .filter(|name| name.ends_with(".proto") && !self.files.contains_key(*name))
            .collect();
        for name in &stale {
            if !Path::new(name)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                return Err(ExportError::Name);
            }
        }
        for (name, source) in &self.files {
            let path = root.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, source)?;
        }
        std::fs::write(root.join("descriptors.bin"), &self.descriptors)?;
        std::fs::write(root.join("contracts.json"), &self.contracts)?;
        for name in stale {
            match std::fs::remove_file(root.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        let mut manifest = String::from("descriptors.bin\ncontracts.json\n");
        for name in self.files.keys() {
            writeln!(manifest, "{name}").expect("String writer");
        }
        std::fs::write(root.join("manifest.txt"), manifest)?;
        Ok(root)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn bounded_contracts<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> Result<Vec<Contract>, D::Error> {
    input.deserialize_seq(ContractVisitor)
}

fn decode_contracts(encoded: &[u8]) -> Result<Vec<Contract>, ExportError> {
    if encoded.len() > MAX_DESCRIPTOR_BYTES {
        return Err(ExportError::Limit);
    }
    let catalog: Catalog = serde_json::from_slice(encoded)?;
    if catalog.schema != 1 {
        return Err(ExportError::Incompatible);
    }
    Ok(catalog.contracts)
}

fn canonical_contracts(contracts: &[&Contract]) -> Result<Vec<u8>, ExportError> {
    #[derive(serde::Serialize)]
    struct Catalog<'a> {
        schema: u32,
        contracts: &'a [&'a Contract],
    }
    let mut writer = MetadataWriter(Vec::new());
    serde_json::to_writer(
        &mut writer,
        &Catalog {
            schema: 1,
            contracts,
        },
    )
    .map_err(|error| {
        if error.is_io() {
            ExportError::Limit
        } else {
            ExportError::Json(error)
        }
    })?;
    Ok(writer.0)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ExportError> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_DESCRIPTOR_BYTES as u64 {
        return Err(ExportError::Limit);
    }
    let mut bytes = Vec::new();
    file.take(MAX_DESCRIPTOR_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(ExportError::Limit);
    }
    Ok(bytes)
}

fn source(file: &FileDescriptorProto) -> Result<String, ExportError> {
    let mut output = String::from(
        "// Generated From Harmos Operation Contracts. Do Not Edit.\nsyntax = \"proto3\";\n\n",
    );
    writeln!(output, "package {};\n", file.package()).expect("String writer");
    for dependency in &file.dependency {
        writeln!(output, "import {dependency:?};").expect("String writer");
    }
    for message in &file.message_type {
        message_source(message, &mut output)?;
    }
    for service in &file.service {
        writeln!(output, "\nservice {} {{", service.name()).expect("String writer");
        for method in &service.method {
            writeln!(
                output,
                "  rpc {} ({}{}) returns ({}{});",
                method.name(),
                if method.client_streaming() {
                    "stream "
                } else {
                    ""
                },
                method.input_type(),
                if method.server_streaming() {
                    "stream "
                } else {
                    ""
                },
                method.output_type()
            )
            .expect("String writer");
        }
        output.push_str("}\n");
    }
    Ok(output)
}

fn message_source(message: &DescriptorProto, output: &mut String) -> Result<(), ExportError> {
    if !message.nested_type.is_empty() || !message.enum_type.is_empty() {
        return Err(ExportError::Unsupported);
    }
    writeln!(output, "\nmessage {} {{", message.name()).expect("String writer");
    for field in &message.field {
        if field.oneof_index.is_none() || field.proto3_optional() {
            let label = if field.proto3_optional() {
                "optional "
            } else if field.label() == Label::Repeated {
                "repeated "
            } else {
                ""
            };
            field_source(field, label, "  ", output)?;
        }
    }
    for (index, oneof) in message.oneof_decl.iter().enumerate() {
        let fields: Vec<_> = message
            .field
            .iter()
            .filter(|field| field.oneof_index == Some(index as i32) && !field.proto3_optional())
            .collect();
        if fields.is_empty() {
            continue;
        }
        writeln!(output, "  oneof {} {{", oneof.name()).expect("String writer");
        for field in fields {
            field_source(field, "", "    ", output)?;
        }
        output.push_str("  }\n");
    }
    output.push_str("}\n");
    Ok(())
}

fn field_source(
    field: &FieldDescriptorProto,
    label: &str,
    indent: &str,
    output: &mut String,
) -> Result<(), ExportError> {
    let kind = match field.r#type() {
        Type::Bool => "bool",
        Type::Uint32 => "uint32",
        Type::Uint64 => "uint64",
        Type::Int32 => "int32",
        Type::Int64 => "int64",
        Type::Float => "float",
        Type::Double => "double",
        Type::String => "string",
        Type::Bytes => "bytes",
        Type::Message => field.type_name(),
        _ => return Err(ExportError::Unsupported),
    };
    let option = field
        .json_name
        .as_ref()
        .map(|name| {
            format!(
                " [json_name = {}]",
                serde_json::to_string(name).expect("JSON string")
            )
        })
        .unwrap_or_default();
    writeln!(
        output,
        "{indent}{label}{kind} {} = {}{option};",
        field.name(),
        field.number()
    )
    .expect("String writer");
    Ok(())
}

/* === Tests === */
