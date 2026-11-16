/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Generate Authoritative Protocol Packages and Descriptor Projections.

/* === Modules === */

mod client;
mod conformance;
mod reflection;

/* === Re-Exports === */

pub use client::generate_client_fixture;
pub use conformance::generate_conformance;

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// Third Party Imports
use prost::Message;
use prost_build::Config;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorSet;

// Local Imports
use harmos_protocol::schema;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn generate(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let mut inputs = Vec::new();
    let mut folders = vec![proto.join("harmos")];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "proto") {
                inputs.push(path);
            }
        }
    }
    inputs.sort();
    let status = Command::new(compiler)
        .arg(format!("--proto_path={}", proto.display()))
        .arg(format!("--proto_path={}", includes.display()))
        .arg(format!("--descriptor_set_out={}", output.join("descriptors.bin").display()))
        .arg("--include_imports")
        .args(&inputs)
        .status()?;
    if !status.success() {
        return Err("Generate Protocol Descriptors: Fix the Reported Compiler Errors".into());
    }
    let descriptors: FileDescriptorSet = FileDescriptorSet::decode(fs::read(output.join("descriptors.bin"))?.as_slice())?;
    let mut packages = BTreeMap::<String, Vec<PathBuf>>::new();
    for file in &descriptors.file {
        let package = file.package.as_deref().unwrap_or_default();
        if package.starts_with("harmos.") {
            packages.entry(package.to_owned()).or_default().push(proto.join(file.name.as_ref().ok_or("missing proto filename")?));
        }
    }
    // Each Package Owns Its Output; External Packages Use Absolute Module Paths.
    for (package, files) in &packages {
        let mut config = Config::new();
        config.protoc_executable(compiler);
        config.type_attribute(".harmos.model.v1.Protobuf", "#[derive(serde::Serialize, serde::Deserialize)]");
        config.type_attribute(".harmos.model.v1.StorageTransferSettings", "#[derive(serde::Serialize, serde::Deserialize)]");
        for external in packages.keys().filter(|external| *external != package) {
            let module = module_path(external)?;
            if package.starts_with(&format!("{external}.")) {
                // Map Ancestor Payloads Individually Without Externalizing This Package.
                for file in descriptors.file.iter().filter(|file| file.package.as_ref() == Some(external)) {
                    for name in file
                        .message_type
                        .iter()
                        .filter_map(|message| message.name.as_ref())
                        .chain(file.enum_type.iter().filter_map(|value| value.name.as_ref()))
                    {
                        config.extern_path(format!(".{external}.{name}"), format!("crate::{module}::{name}"));
                    }
                }
            } else {
                config.extern_path(format!(".{external}"), format!("crate::{module}"));
            }
        }
        let services = package.starts_with("harmos.host.") || package.starts_with("harmos.plugin.");
        let package_output = output.join(package);
        fs::create_dir_all(&package_output)?;
        tonic_prost_build::configure()
            .client_mod_attribute(package, "#[cfg(not(target_arch = \"wasm32\"))]")
            .server_mod_attribute(package, "#[cfg(not(target_arch = \"wasm32\"))]")
            .build_client(services)
            .build_server(services)
            .out_dir(&package_output)
            .compile_with_config(config, files, &[proto.clone(), includes.into()])?;
        let filename = format!("{package}.rs");
        fs::copy(package_output.join(&filename), output.join(filename))?;
    }
    // Parent Modules Include Their Generated Nested Resource Packages.
    for package in packages.keys() {
        let path = output.join(format!("{package}.rs"));
        let mut source = fs::read_to_string(&path)?;
        for child in packages.keys().filter(|child| child.rsplit_once('.').is_some_and(|(parent, _)| parent == package)) {
            let name = child.rsplit('.').next().ok_or("missing child package")?;
            source.push_str(&format!("\npub mod {name};\n"));
        }
        fs::write(path, source)?;
    }
    // Keep protoc's Original Descriptor Bytes: prost_types Discards Custom Method Options.
    // Preproject Guest Graphs During Generation, Before Fuel-Bounded Registration.
    let guest_types = schema::operation_types(&descriptors, &["harmos.plugin.v1.GuestInitialize", "harmos.plugin.v1.GuestShutdown"])?;
    fs::write(output.join("guest-descriptors.bin"), guest_types.encode_to_vec())?;
    let options = FileDescriptorSet {
        file: DescriptorPool::global().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let mut options = schema::message_types(&options, "google.protobuf.MessageOptions")?;
    for file in &mut options.file {
        let mut pending: Vec<_> = file.message_type.iter_mut().collect();
        while let Some(message) = pending.pop() {
            message.options = None;
            for field in &mut message.field {
                field.options = None;
            }
            for enumeration in &mut message.enum_type {
                enumeration.options = None;
                for value in &mut enumeration.value {
                    value.options = None;
                }
            }
            pending.extend(message.nested_type.iter_mut());
        }
        for enumeration in &mut file.enum_type {
            enumeration.options = None;
            for value in &mut enumeration.value {
                value.options = None;
            }
        }
    }
    fs::write(output.join("message-options-descriptors.bin"), options.encode_to_vec())?;
    let clock_roots =
        ["TimeSleepRequest", "TimeSleepResponse", "TimeGetRequest", "TimeGetResponse"].map(|name| format!("harmos.host.v1.{name}"));
    let clock_names: Vec<_> = clock_roots.iter().map(String::as_str).collect();
    let clock_types = schema::operation_types(&descriptors, &clock_names)?;
    fs::write(output.join("clock-descriptors.bin"), clock_types.encode_to_vec())?;
    // SDK Metadata Loads Only Its Reachable Graph Inside a Fuel-Bounded Guest.
    let sdk_types = output.join("context-types");
    fs::create_dir_all(&sdk_types)?;
    for file in &descriptors.file {
        let package = file.package.as_deref().unwrap_or_default();
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("missing message name")?;
            if !package.starts_with("harmos.host.v1")
                && !(package == "harmos.model.v1" && matches!(name, "SettingDeclaration" | "SettingsDeclarations"))
            {
                continue;
            }
            let qualified = format!("{package}.{name}");
            let types = schema::message_types(&descriptors, &qualified)?;
            fs::write(sdk_types.join(format!("{qualified}.bin")), types.encode_to_vec())?;
        }
    }
    let modules = packages
        .keys()
        .map(|package| {
            let module = module_path(package)?;
            Ok((package.clone(), format!("crate::{module}")))
        })
        .collect::<Result<BTreeMap<_, _>, Box<dyn Error>>>()?;
    let reflection = reflection::generate(&descriptors, &modules, "descriptors.bin")?;
    fs::write(output.join("reflection.rs"), reflection)?;
    Ok(())
}

pub fn module_path(package: &str) -> Result<String, Box<dyn Error>> {
    for (prefix, root) in [("harmos.host.v1", "host"), ("harmos.plugin.v1", "plugin"), ("harmos.model.v1", "model")] {
        if package == prefix {
            return Ok(root.into());
        }
        if let Some(child) = package.strip_prefix(&format!("{prefix}.")) {
            return Ok(format!("{root}::{}", child.replace('.', "::")));
        }
    }
    Err(format!("Generate Protocol Module: Use a Model, Host or Plugin Package Instead of {package}").into())
}

/* === Private Functions === */

/* === Tests === */
