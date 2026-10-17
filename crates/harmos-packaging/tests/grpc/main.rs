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

// Third Party Imports
use prost_reflect::DescriptorPool;

// Local Imports
use harmos_domain::operations::{Contracts, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_packaging::proto::{self, Artifacts};

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
    restart: RestartPolicy::Never,
};
#[harmos_domain::message(harmos_path = ::harmos_domain)]
struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}
#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Read"))]
struct Read {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, input = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Exchange"))]
struct Exchange {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, input = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Upload"))]
struct Upload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Download"))]
struct Download {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
enum Routes {
    Read(Read),
    Exchange(Exchange),
    Upload(Upload),
    Download(Download),
}
#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, id = "annotate", response = Annotated, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Portable", method = "Annotate"))]
#[derive(Clone, PartialEq)]
struct Annotated {
    identifier: harmos_domain::Identifier,
    metadata: harmos_domain::Metadata,
    value: harmos_domain::Value,
    optional: Option<harmos_domain::Value>,
    values: Vec<harmos_domain::Value>,
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
enum Portable {
    Annotate(Annotated),
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, id = "wrap", response = Delivered, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Boxed", method = "Wrap"))]
#[derive(Clone, PartialEq)]
struct Envelope {
    inner: Box<Annotated>,
    sequence: u64,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
enum Delivered {
    Missing,
    Boxed(Box<Annotated>),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
enum Boxed {
    Wrap(Envelope),
}

fn annotated() -> Annotated {
    use harmos_domain::{Identifier, Metadata, Value};
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
fn portable_messages_cross() {
            use harmos_domain::{Message, Protobuf};
            let value = annotated();
            let postcard = Message::encode_to_vec(&value);
            assert!(<Annotated as Message>::decode(&postcard).unwrap() == value);
            let protobuf = Protobuf::encode_to_vec(&value);
            assert!(<Annotated as Protobuf>::decode(&protobuf).unwrap() == value);
            assert!(<Annotated as Protobuf>::decoding_memory(&protobuf).is_ok());
            let contracts = Contracts::of::<Portable>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let target = tempfile::tempdir().unwrap();
            let root = proto::generate(
                &artifacts.operation_contracts().unwrap(),
                &target.path().join("grpc/portable"),
            )
            .unwrap();
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
        }

#[test]
fn boxed_messages_cross() {
            use harmos_domain::{Message, Protobuf};
            let request = Envelope {
                inner: Box::new(annotated()),
                sequence: 3,
            };
            let response = Delivered::Boxed(Box::new(annotated()));
            let postcard = Message::encode_to_vec(&request);
            assert!(<Envelope as Message>::decode(&postcard).unwrap() == request);
            let protobuf = Protobuf::encode_to_vec(&response);
            assert!(<Delivered as Protobuf>::decode(&protobuf).unwrap() == response);
            let contracts = Contracts::of::<Boxed>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let target = tempfile::tempdir().unwrap();
            let root = proto::generate(
                &artifacts.operation_contracts().unwrap(),
                &target.path().join("grpc/boxed"),
            )
            .unwrap();
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

            // The Reflected Request Keeps The Plain Nested Message As A Singular Field
            use harmos_domain::schema::Described;
            use harmos_packaging::proto::reflection;
            let settings = harmos_domain::config::SchemaSettings::default();
            let graph = harmos_domain::schema::graph::Schema::from_declaration(
                Envelope::DEFINITION,
                &settings,
            )
            .unwrap();
            let envelope = reflection::descriptor_schema(&graph, "test.v1", &settings).unwrap();
            let inner = envelope.get_field_by_name("inner").unwrap();
            assert!(!inner.is_list());
            let prost_reflect::Kind::Message(nested) = inner.kind() else {
                panic!("boxed field reflects its nested message");
            };
            assert!(nested.get_field_by_name("identifier").is_some());
            let decoded = prost_reflect::DynamicMessage::decode(
                envelope,
                Protobuf::encode_to_vec(&request).as_slice(),
            )
            .unwrap();
            assert_eq!(
                prost_reflect::prost::Message::encode_to_vec(&decoded),
                Protobuf::encode_to_vec(&request)
            );
        }

#[test]
fn export_removes_retired() {
            let target = tempfile::tempdir().unwrap();
            let contracts = Contracts::of::<Routes>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let root = proto::generate(
                &artifacts.operation_contracts().unwrap(),
                &target.path().join("grpc/application"),
            )
            .unwrap();
            assert_eq!(root, target.path().join("grpc/application"));
            let paths: Vec<_> = artifacts
                .files()
                .keys()
                .map(|name| root.join(name))
                .collect();
            assert!(paths.iter().all(|path| path.is_file()));
            std::fs::write(root.join("consumer.txt"), "retained").unwrap();
            let empty = Artifacts::new(std::iter::empty()).unwrap();
            proto::generate(
                &empty.operation_contracts().unwrap(),
                &target.path().join("grpc/application"),
            )
            .unwrap();
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
        }

#[test]
fn independent_schema_catalogs() {
            let target = tempfile::tempdir().unwrap();
            let first = target.path().join("producer-a");
            let second = target.path().join("producer-b");
            let routes = Artifacts::new(&Contracts::of::<Routes>().unwrap()).unwrap();
            let portable = Artifacts::new(&Contracts::of::<Portable>().unwrap()).unwrap();
            let native = proto::generate(
                &routes.operation_contracts().unwrap(),
                &first.join("grpc/native"),
            )
            .unwrap();
            let guest = proto::generate(
                &portable.operation_contracts().unwrap(),
                &first.join("grpc/portable"),
            )
            .unwrap();
            let peer = proto::generate(
                &portable.operation_contracts().unwrap(),
                &second.join("grpc/portable"),
            )
            .unwrap();
            let snapshot = std::fs::read(guest.join("descriptors.bin")).unwrap();
            assert_eq!(
                snapshot,
                std::fs::read(peer.join("descriptors.bin")).unwrap()
            );
            for name in portable.files().keys() {
                assert_eq!(
                    std::fs::read(guest.join(name)).unwrap(),
                    std::fs::read(peer.join(name)).unwrap()
                );
            }
            // Repeated And Reduced Exports Affect Only Their Selected Producer And Catalog.
            let repeated = proto::generate(
                &portable.operation_contracts().unwrap(),
                &first.join("grpc/portable"),
            )
            .unwrap();
            assert_eq!(
                std::fs::read(repeated.join("descriptors.bin")).unwrap(),
                snapshot
            );
            proto::generate(std::iter::empty(), &first.join("grpc/native")).unwrap();
            assert_eq!(
                std::fs::read(guest.join("descriptors.bin")).unwrap(),
                snapshot
            );
            assert_eq!(
                std::fs::read(peer.join("descriptors.bin")).unwrap(),
                snapshot
            );
            assert!(
                DescriptorPool::decode(
                    std::fs::read(native.join("descriptors.bin"))
                        .unwrap()
                        .as_slice()
                )
                .unwrap()
                .services()
                .next()
                .is_none()
            );
            let manifest = std::fs::read_to_string(guest.join("manifest.txt")).unwrap();
            assert!(manifest.starts_with("descriptors.bin\n"));
            assert!(
                portable
                    .files()
                    .keys()
                    .all(|name| manifest.lines().any(|line| line == name))
            );
        }

#[test]
fn exported_sources_compile() {
            let target = tempfile::tempdir().unwrap();
            let contracts = Contracts::of::<Routes>().unwrap();
            let artifacts = Artifacts::new(&contracts).unwrap();
            let root = proto::generate(
                &artifacts.operation_contracts().unwrap(),
                &target.path().join("grpc/application"),
            )
            .unwrap();
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
