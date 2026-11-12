/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Fixed Cross-Language Catalog and Protobuf Fixture Values.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::HashMap;

// Third Party Imports
use prost::Message;
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, Timestamp};

// Local Imports
use crate::schema_fixture;
use harmos_protocol::v1::*;

/* === Constants === */

pub const OPERATIONS: &[&str] = &["echo", "children", "cancel", "failure", "child_failure", "policy"];
pub const CLOCK_FINGERPRINTS: [(&str, [u8; 32]); 2] = [
    (
        "harmos.clock.sleep",
        [
            225, 132, 164, 249, 80, 106, 186, 165, 169, 217, 165, 220, 137, 211, 126, 66, 239, 172, 93, 254, 210, 30, 29, 19, 96, 55, 253,
            247, 27, 7, 156, 171,
        ],
    ),
    (
        "harmos.clock.now",
        [
            205, 13, 190, 24, 110, 229, 160, 96, 250, 199, 77, 136, 248, 239, 51, 52, 68, 167, 137, 155, 87, 218, 83, 236, 177, 253, 20,
            211, 201, 42, 188, 129,
        ],
    ),
];

pub const ROW: &str = "harmos.conformance.v1.Value";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Message)]
pub struct Value {
    #[prost(bytes = "vec", tag = "1")]
    pub value: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn payload(value: &[u8]) -> Vec<u8> {
    Value { value: value.to_vec() }.encode_to_vec()
}

pub fn timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 123_456_789 }
}

pub fn types() -> FileDescriptorSet {
    FileDescriptorSet {
        file: vec![FileDescriptorProto {
            name: Some("harmos/conformance/v1/fixture.proto".into()),
            package: Some("harmos.conformance.v1".into()),
            syntax: Some("proto3".into()),
            message_type: vec![DescriptorProto {
                name: Some("Value".into()),
                field: vec![FieldDescriptorProto {
                    name: Some("value".into()),
                    number: Some(1),
                    label: Some(1),
                    r#type: Some(12),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
}

pub fn fingerprint(id: &str) -> Vec<u8> {
    if let Some((_, fingerprint)) = CLOCK_FINGERPRINTS.iter().find(|(operation, _)| *operation == id) {
        return fingerprint.to_vec();
    }
    harmos_protocol::schema::fingerprint(id, &types(), ROW, ROW).expect("the fixed fixture descriptor resolves Value")
}

pub fn catalog() -> Catalog {
    let mut operations: Vec<_> = OPERATIONS
        .iter()
        .map(|name| {
            let id = format!("conformance.{name}");
            OperationContract {
                fingerprint: fingerprint(&id),
                id,
                request: ROW.into(),
                response: ROW.into(),
                visibility: Visibility::Public as i32,
                ..Default::default()
            }
        })
        .collect();
    let mut requires: Vec<_> = ["conformance.host.echo", "conformance.host.pending", "conformance.host.fail"]
        .into_iter()
        .map(|id| Requirement {
            id: id.into(),
            fingerprint: fingerprint(id),
        })
        .collect();
    let mut streams: Vec<_> = ["ticks", "device"]
        .into_iter()
        .map(|name| StreamDeclaration {
            id: format!("conformance.{name}"),
            row: ROW.into(),
            interval: (name == "ticks").then_some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        })
        .collect();
    for (id, request, response) in [
        ("conformance.schema", "harmos.conformance.v1.Graph", "harmos.conformance.v1.Graph.Nested"),
        ("conformance.nested", "harmos.conformance.v1.Graph.Nested", "harmos.conformance.v1.Graph"),
        ("conformance.projected", "harmos.conformance.v1.Graph", ROW),
    ] {
        operations.push(OperationContract {
            id: id.into(),
            request: request.into(),
            response: response.into(),
            visibility: Visibility::Public as i32,
            fingerprint: harmos_protocol::schema::fingerprint(id, &schema_fixture::types(), request, response)
                .expect("fixture schema resolves"),
            ..Default::default()
        });
    }
    streams.push(StreamDeclaration {
        id: "conformance.settings".into(),
        row: ROW.into(),
        interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
        interval_setting: "harmos.conformance.v1.Graph.interval".into(),
        ..Default::default()
    });
    requires.push(Requirement {
        id: "conformance.host.schema".into(),
        fingerprint: harmos_protocol::schema::fingerprint(
            "conformance.host.schema",
            &schema_fixture::types(),
            "harmos.conformance.v1.Graph",
            ROW,
        )
        .expect("fixture requirement resolves"),
    });
    operations.push(OperationContract {
        id: "conformance.clock".into(),
        request: ROW.into(),
        response: ROW.into(),
        visibility: Visibility::Public as i32,
        fingerprint: fingerprint("conformance.clock"),
        ..Default::default()
    });
    for (id, fingerprint) in CLOCK_FINGERPRINTS {
        requires.push(Requirement {
            id: id.into(),
            fingerprint: fingerprint.to_vec(),
        });
    }
    let mut types = schema_fixture::types();
    let mut standalone = self::types().file.remove(0).message_type.remove(0);
    standalone.name = Some("PayloadOnly".into());
    types.file.iter_mut().find(|file| file.package.as_deref() == Some("harmos.conformance.v1")).unwrap().message_type.push(standalone);
    for file in harmos_protocol::clock::catalog_types().file {
        if !types.file.iter().any(|existing| existing.name == file.name) {
            types.file.push(file);
        }
    }
    let mut catalog = Catalog {
        operations,
        requires,
        streams,
        types: Some(types),
        sections: vec![Section {
            r#type: "harmos.conformance.v1.Graph".into(),
            payload: schema_fixture::Graph {
                interval: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
                ..Default::default()
            }
            .encode_to_vec(),
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
    catalog
}

pub fn failure(operation: &str) -> Failure {
    Failure {
        classification: Classification::Permanent as i32,
        code: "fixture_failure".into(),
        message: "Fixture Failed".into(),
        action: "Use Another Input".into(),
        operation: operation.into(),
        plugin: "conformance".into(),
        cause: Some(Box::new(Failure {
            classification: Classification::Transient as i32,
            code: "fixture_cause".into(),
            message: "Fixture Cause".into(),
            action: "Retry".into(),
            operation: "conformance.host.fail".into(),
            plugin: "host".into(),
            ..Default::default()
        })),
        details: HashMap::from([("fixture".into(), "value".into())]),
        diagnostics: "Fixture Diagnostics".into(),
    }
}

/* === Private Functions === */

/* === Tests === */
