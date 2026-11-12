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
pub const CLOCK_OPERATIONS: [&str; 2] = ["harmos.context.v1.Time/Sleep", "harmos.context.v1.Time/Get"];

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
    if id == "conformance.host.watch" {
        return harmos_protocol::schema::host_fingerprint(
            id,
            &Protobuf {
                descriptors: types().encode_to_vec(),
                request: ROW.into(),
                response: ROW.into(),
                input: None,
                output: Some(ROW.into()),
            },
        )
        .unwrap();
    }
    if CLOCK_OPERATIONS.contains(&id) {
        let (request, response) = if id == "harmos.context.v1.Time/Sleep" {
            ("harmos.context.v1.TimeSleepRequest", "harmos.context.v1.TimeSleepResponse")
        } else {
            ("harmos.context.v1.TimeGetRequest", "harmos.context.v1.TimeGetResponse")
        };
        return harmos_protocol::schema::fingerprint(id, &harmos_protocol::clock::catalog_types(), request, response)
            .expect("clock contract");
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
    let mut requires: Vec<_> = [
        "conformance.host.echo",
        "conformance.host.pending",
        "conformance.host.fail",
        "conformance.host.watch",
        "conformance.peer.public",
        "conformance.peer.protected",
        "conformance.peer.local",
        "conformance.protected",
    ]
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
    for (id, visibility, replay_safe) in [
        ("conformance.protected", Visibility::Protected, false),
        ("conformance.resources", Visibility::Public, true),
        ("conformance.cancel_reason", Visibility::Public, false),
        ("conformance.watch", Visibility::Public, false),
        ("conformance.peer_call", Visibility::Public, false),
    ] {
        operations.push(OperationContract {
            id: id.into(),
            request: ROW.into(),
            response: ROW.into(),
            visibility: visibility as i32,
            replay_safe,
            fingerprint: fingerprint(id),
            ..Default::default()
        });
    }
    operations.insert(
        0,
        OperationContract {
            id: "conformance.local_call".into(),
            request: ROW.into(),
            response: ROW.into(),
            visibility: Visibility::Public as i32,
            fingerprint: fingerprint("conformance.local_call"),
            ..Default::default()
        },
    );
    for id in CLOCK_OPERATIONS {
        requires.push(Requirement {
            id: id.into(),
            fingerprint: fingerprint(id),
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
