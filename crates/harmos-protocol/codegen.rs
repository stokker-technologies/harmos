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
use harmos_protocol::host::InvocationsCreateRequest;

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
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    config.compile_protos(
        &[proto.join("conformance/model/v1/fixture.proto"), proto.join("conformance/host/v1/caller_probe.proto")],
        &[proto, library, includes.into()],
    )?;
    let descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("conformance-descriptors.bin"))?.as_slice())?;
    let request = InvocationsCreateRequest {
        operation: "conformance.host.v1.CallerProbe/Get".into(),
        fingerprint: harmos_protocol::schema::fingerprint(
            "conformance.host.v1.CallerProbe/Get",
            &descriptors,
            "conformance.host.v1.CallerProbeGetRequest",
            "conformance.host.v1.CallerProbeGetResponse",
        )?,
        payload: Vec::new(),
    };
    std::fs::write(output.join("caller-request.bin"), prost::Message::encode_to_vec(&request))?;
    let descriptors =
        harmos_protocol::schema::operation_types(&descriptors, &["conformance.model.v1.Graph", "conformance.model.v1.PayloadOnly"])?;
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
    // Keep Protoc's Bytes: prost_types Would Discard Custom Method Options.
    // Protoc Omits Source Info Unless Explicitly Requested.
    // Preproject Guest Graphs During Generation, Before Fuel-Bounded Registration.
    let guest_types =
        harmos_protocol::schema::operation_types(&descriptors, &["harmos.plugin.v1.GuestInitialize", "harmos.plugin.v1.GuestShutdown"])?;
    std::fs::write(output.join("guest-descriptors.bin"), prost::Message::encode_to_vec(&guest_types))?;
    let options = prost_types::FileDescriptorSet {
        file: prost_reflect::DescriptorPool::global().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let mut options = harmos_protocol::schema::message_types(&options, "google.protobuf.MessageOptions")?;
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
    std::fs::write(output.join("message-options-descriptors.bin"), prost::Message::encode_to_vec(&options))?;
    let clock_roots =
        ["TimeSleepRequest", "TimeSleepResponse", "TimeGetRequest", "TimeGetResponse"].map(|name| format!("harmos.host.v1.{name}"));
    let clock_names: Vec<_> = clock_roots.iter().map(String::as_str).collect();
    let clock_types = harmos_protocol::schema::operation_types(&descriptors, &clock_names)?;
    std::fs::write(output.join("clock-descriptors.bin"), prost::Message::encode_to_vec(&clock_types))?;
    // SDK Metadata Loads Only Its Reachable Graph Inside a Fuel-Bounded Guest.
    let sdk_types = output.join("context-types");
    std::fs::create_dir_all(&sdk_types)?;
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
    for (prefix, root) in [("harmos.host.v1", "host"), ("harmos.plugin.v1", "plugin"), ("harmos.model.v1", "model")] {
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
    let files =
        ["application/host/v1/jobs.proto", "application/host/v1/jobs/progress.proto", "application/host/v1/plugins/annotations.proto"]
            .map(|file| proto.join(file));
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.extern_path(".harmos.model.v1", "::harmos_protocol::model");
    config.file_descriptor_set_path(output.join("client-descriptors.bin"));
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    tonic_prost_build::configure().out_dir(output).compile_with_config(config, &files, &[proto, library, includes.into()])?;
    let descriptors: prost_types::FileDescriptorSet =
        prost::Message::decode(std::fs::read(output.join("client-descriptors.bin"))?.as_slice())?;
    let mut source = String::from(
        "// @generated from client composition descriptors.\nuse prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};\nfn pool() -> &'static DescriptorPool {\nstatic POOL: std::sync::OnceLock<DescriptorPool> = std::sync::OnceLock::new();\nPOOL.get_or_init(|| DescriptorPool::decode(include_bytes!(\"client-descriptors.bin\").as_slice()).expect(\"generated client descriptors\"))\n}\n",
    );
    for file in &descriptors.file {
        let package = file.package.as_deref().ok_or("fixture package missing")?;
        let Some(suffix) = package.strip_prefix("application.host.v1") else {
            continue;
        };
        let suffix = suffix.trim_start_matches('.');
        let module = if suffix.is_empty() { "crate::host".to_owned() } else { format!("crate::host::{}", suffix.replace('.', "::")) };
        for message in &file.message_type {
            let name = message.name.as_deref().ok_or("fixture message missing")?;
            source.push_str(&format!("impl ReflectMessage for {module}::{name} {{ fn descriptor(&self) -> MessageDescriptor {{ pool().get_message_by_name(\"{package}.{name}\").expect(\"generated fixture message\") }} }}\n"));
        }
    }
    std::fs::write(output.join("client-reflection.rs"), source)?;
    Ok(())
}
