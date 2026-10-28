/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compile Authored Protocol Exports

#[allow(dead_code, reason = "shared inert contracts also serve export integration")]
#[path = "../support/proto_contracts.rs"]
mod contracts;

use prost_reflect::DescriptorPool;

use contracts::{AnnotatedOperation, Boxed, Delivered, EnvelopeOperation, Portable, Routes, annotated};
use harmos_domain::operations::Contracts;
use harmos_packaging::proto::{self, Artifacts};
use harmos_sdk::grpc::{self, DecodeInput};

fn normalize(message: &mut prost_reflect::prost_types::DescriptorProto) {
    // protoc Materializes JSON Names Which Are Implicit In Authored Descriptors.
    for field in &mut message.field {
        field.json_name = None;
        // Synthetic Presence Oneof Names Are Compiler Details, Not Authored
        // Fields. Preserve Their Count, Index And Optional Flag Exactly.
        if field.proto3_optional() {
            let index = field.oneof_index.expect("optional scalar has a presence oneof");
            message.oneof_decl[index as usize].name = Some(format!("presence_{}", field.number()));
        }
    }
    for nested in &mut message.nested_type {
        normalize(nested);
    }
}

#[test]
fn portable_messages_cross() {
    use harmos_domain::Message;
    let value = annotated();
    let postcard = Message::encode(&value);
    assert!(<AnnotatedOperation as Message>::decode(&postcard).unwrap() == value);
    let protobuf = {
        let mut bytes = Vec::new();
        grpc::encode(&value, &mut bytes).unwrap();
        bytes
    };
    assert!(grpc::decode::<AnnotatedOperation>(DecodeInput::new(&protobuf)).unwrap() == value);
    assert!(grpc::prepare::<AnnotatedOperation>(DecodeInput::new(&protobuf)).map(|prepared| prepared.memory()).is_ok());
    let contracts = Contracts::of::<Portable>().unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let root = proto::generate(&artifacts.operation_contracts().unwrap(), &target.path().join("grpc/portable")).unwrap();
    let output = crate::fixtures::command(crate::fixtures::protoc())
        .current_dir(&root)
        .arg("--proto_path=.")
        .arg(format!("--descriptor_set_out={}", target.path().join("compiled.bin").display()))
        .args(artifacts.files().keys())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn boxed_messages_cross() {
    use harmos_domain::Message;
    let request = EnvelopeOperation {
        inner: Box::new(annotated()),
        sequence: 3,
    };
    let response = Delivered::Boxed(Box::new(annotated()));
    let postcard = Message::encode(&request);
    assert!(<EnvelopeOperation as Message>::decode(&postcard).unwrap() == request);
    let protobuf = {
        let mut bytes = Vec::new();
        grpc::encode(&response, &mut bytes).unwrap();
        bytes
    };
    assert!(grpc::decode::<Delivered>(DecodeInput::new(&protobuf)).unwrap() == response);
    let contracts = Contracts::of::<Boxed>().unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let root = proto::generate(&artifacts.operation_contracts().unwrap(), &target.path().join("grpc/boxed")).unwrap();
    let compiled = target.path().join("compiled.bin");
    let output = crate::fixtures::command(crate::fixtures::protoc())
        .current_dir(&root)
        .arg("--proto_path=.")
        .arg("--include_imports")
        .arg(format!("--descriptor_set_out={}", compiled.display()))
        .args(artifacts.files().keys())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // The Reflected Request Keeps The Plain Nested Message As A Singular Field
    use harmos_domain::schema::Described;
    use harmos_packaging::proto::reflection;
    let settings = harmos_domain::config::SchemaSettings::default();
    let graph = harmos_domain::schema::graph::Schema::from_declaration(EnvelopeOperation::DEFINITION, &settings).unwrap();
    let envelope = reflection::descriptor_schema(&graph, "test.v1", &settings).unwrap();
    let inner = envelope.get_field_by_name("inner").unwrap();
    assert!(!inner.is_list());
    let prost_reflect::Kind::Message(nested) = inner.kind() else {
        panic!("boxed field reflects its nested message");
    };
    assert!(nested.get_field_by_name("identifier").is_some());
    let decoded = prost_reflect::DynamicMessage::decode(
        envelope,
        {
            let mut bytes = Vec::new();
            grpc::encode(&request, &mut bytes).unwrap();
            bytes
        }
        .as_slice(),
    )
    .unwrap();
    assert_eq!(prost_reflect::prost::Message::encode_to_vec(&decoded), {
        let mut bytes = Vec::new();
        grpc::encode(&request, &mut bytes).unwrap();
        bytes
    });
}

#[test]
fn exported_sources_compile() {
    let target = tempfile::tempdir().unwrap();
    let contracts = Contracts::of::<Routes>().unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let root = proto::generate(&artifacts.operation_contracts().unwrap(), &target.path().join("grpc/application")).unwrap();
    let compiled = target.path().join("compiled.bin");
    let output = crate::fixtures::command(crate::fixtures::protoc())
        .current_dir(&root)
        .arg("--proto_path=.")
        .arg("--include_imports")
        .arg(format!("--descriptor_set_out={}", compiled.display()))
        .args(artifacts.files().keys())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let compiled = DescriptorPool::decode(std::fs::read(compiled).unwrap().as_slice()).unwrap();
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
                    method.client_streaming = method.client_streaming.filter(|value| *value);
                    method.server_streaming = method.server_streaming.filter(|value| *value);
                }
            }
        }
        assert_eq!(actual, expected, "{}", file.name());
    }
}
