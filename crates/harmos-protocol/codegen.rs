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

pub fn generate_conformance(
    compiler: &std::path::Path, includes: &std::path::Path, output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
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

pub fn generate(
    compiler: &std::path::Path, includes: &std::path::Path, output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let mut inputs = Vec::new();
    let mut folders = vec![proto.join("harmos/v1"), proto.join("harmos/context/v1"), proto.join("harmos/plugin/v1")];
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
    let mut packages = std::collections::BTreeMap::<String, Vec<std::path::PathBuf>>::new();
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
        config.type_attribute(".harmos.storage.v1.StorageTransferSettings", "#[derive(serde::Serialize, serde::Deserialize)]");
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
        let services = !package.starts_with("harmos.context.");
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
        std::fs::write(path, source)?;
    }
    // Source Locations Belong to Authoring, Not Runtime Descriptor Validation.
    for file in &mut descriptors.file {
        file.source_code_info = None;
    }
    std::fs::write(output.join("descriptors.bin"), prost::Message::encode_to_vec(&descriptors))?;
    // Preproject Guest Graphs During Generation, Before Fuel-Bounded Registration.
    let guest_types = harmos_protocol::schema::operation_types(
        &descriptors,
        &[
            "harmos.v1.GuestInitialize",
            "harmos.v1.GuestShutdown",
            "harmos.context.v1.SamplesAppendRequest",
            "harmos.context.v1.SamplesAppendResponse",
            "harmos.context.v1.StorageGetFileHashRequest",
            "harmos.context.v1.StorageGetFileHashResponse",
        ],
    )?;
    std::fs::write(output.join("guest-descriptors.bin"), prost::Message::encode_to_vec(&guest_types))?;
    // SDK Metadata Loads Only Its Reachable Graph Inside a Fuel-Bounded Guest.
    let sdk_types = output.join("context-types");
    std::fs::create_dir_all(&sdk_types)?;
    for file in &descriptors.file {
        let package = file.package.as_deref().unwrap_or_default();
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("missing message name")?;
            if package != "harmos.context.v1" && !(package == "harmos.v1" && matches!(name, "SettingDeclaration" | "SettingsDeclarations"))
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

pub fn module_path(package: &str) -> Result<String, Box<dyn std::error::Error>> {
    for (prefix, root) in
        [("harmos.context.v1", "context"), ("harmos.plugin.v1", "plugin"), ("harmos.storage.v1", "storage"), ("harmos.v1", "v1")]
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

/// Generate the Example's Authoritative Contracts Alongside the Core Bindings.
pub fn generate_example(
    compiler: &std::path::Path, includes: &std::path::Path, output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.out_dir(output);
    config.extern_path(".harmos.storage.v1", "::harmos_protocol::storage");
    config.extern_path(".harmos.context.v1", "::harmos_protocol::context");
    config.file_descriptor_set_path(output.join("schematic.descriptors.bin"));
    config.compile_protos(&[proto.join("schematic/v1/operations.proto")], &[proto, includes.into()])?;
    let mut descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("schematic.descriptors.bin"))?.as_slice())?;
    for file in &mut descriptors.file {
        file.source_code_info = None;
    }
    std::fs::write(output.join("schematic.descriptors.bin"), prost::Message::encode_to_vec(&descriptors))?;
    let mut reflection = String::from("// This file is @generated by the protocol generator.\n");
    for file in descriptors.file.iter().filter(|file| file.package.as_deref() == Some("schematic.v1")) {
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("missing example message name")?;
            reflection.push_str(&format!("impl prost_reflect::ReflectMessage for super::{name} {{\n    fn descriptor(&self) -> prost_reflect::MessageDescriptor {{\n        super::pool().get_message_by_name(\"schematic.v1.{name}\").expect(\"generated example message\")\n    }}\n}}\n"));
            let types = harmos_protocol::schema::operation_types(&descriptors, &[&format!("schematic.v1.{name}")])?;
            let encoded = prost::Message::encode_to_vec(&types).iter().map(|byte| format!("\\x{byte:02x}")).collect::<String>();
            reflection.push_str(&format!("#[cfg(feature = \"sdk\")]\nimpl harmos::Message for super::{name} {{\n    fn name() -> String {{ \"schematic.v1.{name}\".into() }}\n    fn types() -> prost_types::FileDescriptorSet {{\n        prost::Message::decode(&b\"{encoded}\"[..]).expect(\"generated example graph\")\n    }}\n}}\n"));
        }
    }
    std::fs::write(output.join("schematic.reflection.rs"), reflection)?;
    Ok(())
}

/// Generate Independent Library and Product Contracts for SDK Composition Stories.
pub fn generate_client_fixture(
    compiler: &std::path::Path, includes: &std::path::Path, output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/tests/fixtures/client/proto");
    let files = [
        "application/v1/jobs.proto",
        "application/v1/jobs/progress.proto",
    ]
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
