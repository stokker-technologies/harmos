/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Authoritative Protocol Bindings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn generate_conformance(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/tests/fixtures/conformance/proto");
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.out_dir(output);
    config.skip_source_info();
    config.file_descriptor_set_path(output.join("conformance-descriptors.bin"));
    config.compile_protos(&[proto.join("harmos/conformance/v1/fixture.proto")], &[proto, includes.into()])?;
    let descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("conformance-descriptors.bin"))?.as_slice())?;
    let descriptors =
        harmos_protocol::schema::operation_types(&descriptors, &["harmos.conformance.v1.Graph", "harmos.conformance.v1.PayloadOnly"])?;
    std::fs::write(output.join("conformance-descriptors.bin"), prost::Message::encode_to_vec(&descriptors))?;
    Ok(())
}

pub fn generate(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let mut inputs = Vec::new();
    let mut folders = vec![proto.join("harmos")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "proto") {
                inputs.push(path);
            }
        }
    }
    inputs.sort();
    let status = std::process::Command::new(compiler)
        .arg(format!("--proto_path={}", proto.display()))
        .arg(format!("--proto_path={}", includes.display()))
        .arg(format!("--descriptor_set_out={}", output.join("descriptors.bin").display()))
        .arg("--include_imports")
        .args(&inputs)
        .status()?;
    if !status.success() {
        return Err("protocol descriptor generation failed".into());
    }
    let mut descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("descriptors.bin"))?.as_slice())?;
    let mut packages = BTreeMap::<String, Vec<PathBuf>>::new();
    for file in &descriptors.file {
        let package = file.package.as_deref().unwrap_or_default();
        if package.starts_with("harmos.") {
            packages.entry(package.to_owned()).or_default().push(proto.join(file.name.as_ref().ok_or("missing proto filename")?));
        }
    }
    // Each Package Owns Its Output; External Packages Use Absolute Module Paths.
    for (package, files) in &packages {
        let mut config = prost_build::Config::new();
        config.protoc_executable(compiler);
        config.type_attribute(".harmos.v1.Protobuf", "#[derive(serde::Serialize, serde::Deserialize)]");
        config.type_attribute(".harmos.v1.StorageTransferSettings", "#[derive(serde::Serialize, serde::Deserialize)]");
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
        let services = !package.starts_with("harmos.plugin.host.");
        let package_output = output.join(package);
        std::fs::create_dir_all(&package_output)?;
        tonic_prost_build::configure()
            .client_mod_attribute(package, "#[cfg(not(target_arch = \"wasm32\"))]")
            .server_mod_attribute(package, "#[cfg(not(target_arch = \"wasm32\"))]")
            .build_client(services)
            .build_server(services)
            .out_dir(&package_output)
            .compile_with_config(config, files, &[proto.clone(), includes.into()])?;
        let filename = format!("{package}.rs");
        std::fs::copy(package_output.join(&filename), output.join(filename))?;
    }
    // Parent Modules Include Their Generated Nested Resource Packages.
    for package in packages.keys() {
        let path = output.join(format!("{package}.rs"));
        let mut source = std::fs::read_to_string(&path)?;
        for child in packages.keys().filter(|child| child.rsplit_once('.').is_some_and(|(parent, _)| parent == package)) {
            let name = child.rsplit('.').next().ok_or("missing child package")?;
            let module = module_path(package)?;
            let parent = module.rsplit("::").next().ok_or("missing parent module")?;
            source.push_str(&format!("\npub mod {name} {{ include!(\"{parent}/{name}.rs\"); }}\n"));
        }
        if package == "harmos.plugin.v1" {
            source.push_str("\npub mod host { include!(\"plugin/host.rs\"); }\n");
        }
        std::fs::write(path, source)?;
    }
    // Source Locations Belong to Authoring, Not Runtime Descriptor Validation.
    for file in &mut descriptors.file {
        file.source_code_info = None;
    }
    std::fs::write(output.join("descriptors.bin"), prost::Message::encode_to_vec(&descriptors))?;
    // Preproject Guest Graphs During Generation, Before Fuel-Bounded Registration.
    let guest_types = harmos_protocol::schema::operation_types(&descriptors, &["harmos.v1.GuestInitialize", "harmos.v1.GuestShutdown"])?;
    std::fs::write(output.join("guest-descriptors.bin"), prost::Message::encode_to_vec(&guest_types))?;
    // SDK Metadata Loads Only Its Reachable Graph Inside a Fuel-Bounded Guest.
    let sdk_types = output.join("context-types");
    std::fs::create_dir_all(&sdk_types)?;
    for file in &descriptors.file {
        let package = file.package.as_deref().unwrap_or_default();
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("missing message name")?;
            if package != "harmos.plugin.host.v1"
                && !(package == "harmos.v1" && matches!(name, "SettingDeclaration" | "SettingsDeclarations"))
            {
                continue;
            }
            let qualified = format!("{package}.{name}");
            let types = harmos_protocol::schema::message_types(&descriptors, &qualified)?;
            std::fs::write(sdk_types.join(format!("{qualified}.bin")), prost::Message::encode_to_vec(&types))?;
        }
    }
    let mut reflection = String::from(
        "// This file is @generated by the protocol generator.\nuse std::sync::OnceLock;\nuse prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};\nfn pool() -> &'static DescriptorPool {\n    static POOL: OnceLock<DescriptorPool> = OnceLock::new();\n    POOL.get_or_init(|| DescriptorPool::decode(include_bytes!(\"descriptors.bin\").as_slice()).expect(\"generated protocol descriptors\"))\n}\n",
    );
    for package in packages.keys() {
        let module = module_path(package)?;
        for file in descriptors.file.iter().filter(|file| file.package.as_deref() == Some(package.as_str())) {
            for message in &file.message_type {
                let name = message.name.as_deref().ok_or("missing message name")?;
                reflection.push_str(&format!("impl ReflectMessage for crate::{module}::{name} {{\n    fn descriptor(&self) -> MessageDescriptor {{\n        pool().get_message_by_name(\"{package}.{name}\").expect(\"generated message\")\n    }}\n}}\n"));
            }
        }
    }

    std::fs::write(output.join("reflection.rs"), reflection)?;
    Ok(())
}

/* === Private Functions === */

pub fn module_path(package: &str) -> Result<String, Box<dyn Error>> {
    for (prefix, root) in
        [("harmos.plugin.host.v1", "plugin::host"), ("harmos.client.v1", "client"), ("harmos.plugin.v1", "plugin"), ("harmos.v1", "v1")]
    {
        if package == prefix {
            return Ok(root.into());
        }
        if let Some(child) = package.strip_prefix(&format!("{prefix}.")) {
            return Ok(format!("{root}::{}", child.replace('.', "::")));
        }
    }
    Err(format!("unrecognized protocol package {package}").into())
}

/* === Tests === */

/// Generate Independent Library and Product Contracts for SDK Composition Stories.
pub fn generate_client_fixture(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/tests/fixtures/client/proto");
    let files = ["application/v1/jobs.proto", "application/v1/jobs/progress.proto", "application/v1/plugins/annotations.proto"]
        .map(|file| proto.join(file));
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.file_descriptor_set_path(output.join("client-descriptors.bin"));
    tonic_prost_build::configure().out_dir(output).compile_with_config(config, &files, &[proto, includes.into()])?;
    let descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("client-descriptors.bin"))?.as_slice())?;
    let mut source = String::from(
        "// @generated from client composition descriptors.\nuse prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};\nfn pool() -> &'static DescriptorPool {\nstatic POOL: std::sync::OnceLock<DescriptorPool> = std::sync::OnceLock::new();\nPOOL.get_or_init(|| DescriptorPool::decode(include_bytes!(\"client-descriptors.bin\").as_slice()).expect(\"generated client descriptors\"))\n}\n",
    );
    for file in &descriptors.file {
        let package = file.package.as_deref().ok_or("fixture package missing")?;
        let suffix = package.split_once(".v1").ok_or("fixture version missing")?.1.trim_start_matches('.');
        let module = if suffix.is_empty() { "crate::v1".to_owned() } else { format!("crate::v1::{}", suffix.replace('.', "::")) };
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("fixture message missing")?;
            source.push_str(&format!("impl ReflectMessage for {module}::{name} {{ fn descriptor(&self) -> MessageDescriptor {{ pool().get_message_by_name(\"{package}.{name}\").expect(\"generated fixture message\") }} }}\n"));
        }
    }
    std::fs::write(output.join("client-reflection.rs"), source)?;
    Ok(())
}
