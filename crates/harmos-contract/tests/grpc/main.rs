/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_reflect::DescriptorPool;

// Local Imports
use harmos_contract::grpc::Artifacts;
use harmos_contract::operations::{Contracts, OperationPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: RetryPolicy::Never,
};
#[harmos_contract::message(harmos_path = ::harmos_contract)]
struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}
#[harmos_macros::operation(request = Payload, response = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Read"))]
struct Read;

#[harmos_macros::operation(request = Payload, response = Payload, input = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Exchange"))]
struct Exchange;

#[harmos_macros::operation(request = Payload, response = Payload, input = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Upload"))]
struct Upload;

#[harmos_macros::operation(request = Payload, response = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Download"))]
struct Download;

#[harmos_macros::catalog(harmos_path = ::harmos_contract)]
enum Routes {
    Read(Read),
    Exchange(Exchange),
    Upload(Upload),
    Download(Download),
}
#[harmos_contract::message(harmos_path = ::harmos_contract)]
#[derive(Clone, PartialEq)]
struct Annotated {
    identifier: harmos_contract::Identifier,
    metadata: harmos_contract::Metadata,
    value: harmos_contract::Value,
    optional: Option<harmos_contract::Value>,
    values: Vec<harmos_contract::Value>,
}

#[harmos_macros::operation(request = Annotated, response = Annotated, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Portable", method = "Annotate"))]
struct Annotate;

#[harmos_macros::catalog(harmos_path = ::harmos_contract)]
enum Portable {
    Annotate(Annotate),
}

fn annotated() -> Annotated {
    use harmos_contract::{Identifier, Metadata, Value};
    let mut metadata = Metadata::definition(
        Identifier::new("device.reading").unwrap(),
        "Reading",
        None,
        ["lab"],
    );
    metadata
        .attributes
        .insert("channel".into(), Value::Unsigned(1));
    Annotated {
        identifier: Identifier::new("device").unwrap(),
        metadata,
        value: Value::Array(vec![Value::Text("x".into()), Value::Null]),
        optional: Some(Value::Number(2.5)),
        values: vec![Value::Bool(true), Value::Integer(-1)],
    }
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("portable_messages_cross", || {
            use harmos_contract::{Message, Protobuf};
            let value = annotated();
            let postcard = Message::encode_to_vec(&value);
            assert!(<Annotated as Message>::decode(&postcard).unwrap() == value);
            let protobuf = Protobuf::encode_to_vec(&value);
            assert!(<Annotated as Protobuf>::decode(&protobuf).unwrap() == value);
            assert!(<Annotated as Protobuf>::decoding_memory(&protobuf).is_ok());
            let contracts = Contracts::of::<Portable>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let target = tempfile::tempdir().unwrap();
            let root = artifacts.export(target.path(), "portable").unwrap();
            let output =
                std::process::Command::new(protoc_bin_vendored::protoc_bin_path().unwrap())
                    .current_dir(&root)
                    .arg("--proto_path=.")
                    .arg(format!(
                        "--descriptor_set_out={}",
                        target.path().join("compiled.bin").display()
                    ))
                    .args(artifacts.files().keys())
                    .output()
                    .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        });
    }
    {
        scenario_runtime::check("export_removes_retired", || {
            let target = tempfile::tempdir().unwrap();
            let contracts = Contracts::of::<Routes>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let root = artifacts.export(target.path(), "application").unwrap();
            assert_eq!(root, target.path().join("grpc/application"));
            let paths: Vec<_> = artifacts
                .files()
                .keys()
                .map(|name| root.join(name))
                .collect();
            assert!(paths.iter().all(|path| path.is_file()));
            std::fs::write(root.join("consumer.txt"), "retained").unwrap();
            let empty = Artifacts::new(std::iter::empty()).unwrap();
            empty.export(target.path(), "application").unwrap();
            assert!(paths.iter().all(|path| !path.exists()));
            assert!(root.join("consumer.txt").is_file());
            assert!(
                DescriptorPool::decode(
                    std::fs::read(root.join("descriptors.bin"))
                        .unwrap()
                        .as_slice()
                )
                .unwrap()
                .services()
                .next()
                .is_none()
            );
            assert!(artifacts.export(target.path(), "../escape").is_err());
        });
    }
    {
        scenario_runtime::check("exported_sources_compile", || {
            let target = tempfile::tempdir().unwrap();
            let contracts = Contracts::of::<Routes>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let root = artifacts.export(target.path(), "application").unwrap();
            let compiled = target.path().join("compiled.bin");
            let output =
                std::process::Command::new(protoc_bin_vendored::protoc_bin_path().unwrap())
                    .current_dir(&root)
                    .arg("--proto_path=.")
                    .arg("--include_imports")
                    .arg(format!("--descriptor_set_out={}", compiled.display()))
                    .args(artifacts.files().keys())
                    .output()
                    .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let compiled =
                DescriptorPool::decode(std::fs::read(compiled).unwrap().as_slice()).unwrap();
            let original = DescriptorPool::decode(artifacts.descriptors()).unwrap();
            assert_eq!(compiled.files().len(), original.files().len());
            for file in original.files() {
                let actual = compiled.get_file_by_name(file.name()).unwrap();
                let mut expected = file.file_descriptor_proto().clone();
                let mut actual = actual.file_descriptor_proto().clone();
                for message in &mut expected.message_type {
                    normalize(message);
                }
                for message in &mut actual.message_type {
                    normalize(message);
                }
                // Absent Streaming Flags And Explicit False Have The Same Proto Meaning.
                for descriptor in [&mut actual, &mut expected] {
                    for service in &mut descriptor.service {
                        for method in &mut service.method {
                            method.client_streaming =
                                method.client_streaming.filter(|value| *value);
                            method.server_streaming =
                                method.server_streaming.filter(|value| *value);
                        }
                    }
                }
                assert_eq!(actual, expected, "{}", file.name());
            }
        });
    }
}

fn normalize(message: &mut prost_reflect::prost_types::DescriptorProto) {
    // protoc Materializes JSON Names Which Are Implicit In Authored Descriptors.
    for field in &mut message.field {
        field.json_name = None;
        // Synthetic Presence Oneof Names Are Compiler Details, Not Authored
        // Fields. Preserve Their Count, Index And Optional Flag Exactly.
        if field.proto3_optional() {
            let index = field
                .oneof_index
                .expect("optional scalar has a presence oneof");
            message.oneof_decl[index as usize].name = Some(format!("presence_{}", field.number()));
        }
    }
    for nested in &mut message.nested_type {
        normalize(nested);
    }
}
