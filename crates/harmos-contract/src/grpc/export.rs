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
    collections::BTreeMap,
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
}

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
    #[error("cargo metadata failed: {0}")]
    Cargo(String),
    #[error("cargo metadata returned no absolute target directory")]
    TargetDirectory,
    #[error(transparent)]
    Metadata(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/* === Traits === */

/* === Implementations === */
impl Artifacts {
    pub fn new<'a>(contracts: impl IntoIterator<Item = &'a Contract>) -> Result<Self, ExportError> {
        let mut services: BTreeMap<(&str, &str), Vec<(&str, &Contract)>> = BTreeMap::new();
        let mut count = 0;
        for contract in contracts {
            let Some(binding) = contract.grpc() else {
                continue;
            };
            count += 1;
            if count > MAX_METHODS {
                return Err(ExportError::Limit);
            }
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
        })
    }

    pub fn descriptors(&self) -> &[u8] {
        &self.descriptors
    }
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_cargo(&self, manifest: &Path, name: &str) -> Result<PathBuf, ExportError> {
        let manifest = manifest.canonicalize()?;
        let directory = manifest.parent().ok_or(ExportError::TargetDirectory)?;
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = std::process::Command::new(cargo)
            .current_dir(directory)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
            ])
            .arg(&manifest)
            .output()?;
        if !output.status.success() {
            return Err(ExportError::Cargo(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        #[derive(serde::Deserialize)]
        struct Metadata {
            target_directory: PathBuf,
        }
        let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
        if !metadata.target_directory.is_absolute() {
            return Err(ExportError::TargetDirectory);
        }
        self.export(&metadata.target_directory, name)
    }

    pub fn export(&self, target_directory: &Path, name: &str) -> Result<PathBuf, ExportError> {
        if name.is_empty()
            || name.len() > 255
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ExportError::Name);
        }
        let root = target_directory.join("grpc").join(name);
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
        for name in stale {
            match std::fs::remove_file(root.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        let mut manifest = String::from("descriptors.bin\n");
        for name in self.files.keys() {
            writeln!(manifest, "{name}").expect("String writer");
        }
        std::fs::write(root.join("manifest.txt"), manifest)?;
        Ok(root)
    }
}

/* === Public Functions === */

/* === Private Functions === */
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
    writeln!(
        output,
        "{indent}{label}{kind} {} = {};",
        field.name(),
        field.number()
    )
    .expect("String writer");
    Ok(())
}

/* === Tests === */
