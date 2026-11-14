/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Fixed Cross-Language Declarations and Protobuf Fixture Values.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::HashMap;

// Third Party Imports
use prost::Message;
use prost_reflect::ReflectMessage;
use prost_types::{DescriptorProto, Duration, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, Timestamp};

// Local Imports
use crate::conformance::Graph;
use harmos_protocol::host::plugins::SettingsListRequest;
use harmos_protocol::model::{
    Classification, Declarations, Failure, OperationContract, Protobuf, Requirement, Section, SettingDeclaration, SettingValue,
    SettingValues, SettingsDeclarations, StreamDeclaration, Visibility, setting_value,
};

/* === Constants === */

pub const OPERATIONS: &[&str] = &["step.0", "children", "cancel", "failure", "child_failure", "policy"];
pub const CLOCK_OPERATIONS: [&str; 2] = ["harmos.host.v1.Time/Sleep", "harmos.host.v1.Time/Get"];

pub const TICK_STREAMS: &[&str] = &[
    "conformance.tick_children",
    "conformance.tick_context",
    "conformance.tick_failure",
    "conformance.tick_watch",
    "conformance.tick_cancel",
    "conformance.tick_caller_refusal",
];
pub const HOST_OPERATIONS: &[(&str, &str, &str)] = &[
    ("harmos.host.v1.Invocations/Create", "harmos.host.v1.InvocationsCreateRequest", "harmos.host.v1.InvocationsCreateResponse"),
    ("harmos.host.v1.plugins.Settings/List", "harmos.host.v1.plugins.SettingsListRequest", "harmos.host.v1.plugins.SettingsListResponse"),
    ("harmos.host.v1.Operations/List", "harmos.host.v1.OperationsListRequest", "harmos.host.v1.OperationsListResponse"),
    ("harmos.host.v1.Operations/Get", "harmos.host.v1.OperationsGetRequest", "harmos.host.v1.OperationsGetResponse"),
];

pub const ROW: &str = "conformance.model.v1.Value";

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
            name: Some("conformance/model/v1/fixture.proto".into()),
            package: Some("conformance.model.v1".into()),
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
    if let Some((_, request, response)) = HOST_OPERATIONS.iter().find(|(operation, _, _)| *operation == id) {
        return harmos_protocol::schema::fingerprint(id, &host_types(), request, response).expect("typed host contract");
    }
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
        let (request, response) = if id == "harmos.host.v1.Time/Sleep" {
            ("harmos.host.v1.TimeSleepRequest", "harmos.host.v1.TimeSleepResponse")
        } else {
            ("harmos.host.v1.TimeGetRequest", "harmos.host.v1.TimeGetResponse")
        };
        return harmos_protocol::schema::fingerprint(id, &harmos_protocol::clock::declarations_types(), request, response)
            .expect("clock contract");
    }
    harmos_protocol::schema::fingerprint(id, &types(), ROW, ROW).expect("the fixed fixture descriptor resolves Value")
}

pub fn host_types() -> FileDescriptorSet {
    let pool = SettingsListRequest::default().descriptor().parent_pool().clone();
    let types = FileDescriptorSet::decode(pool.encode_to_vec().as_slice()).expect("generated host descriptors");
    let names: Vec<_> = HOST_OPERATIONS
        .iter()
        .flat_map(|(_, request, response)| [*request, *response])
        .chain(["harmos.model.v1.SettingsDeclarations"])
        .collect();
    harmos_protocol::schema::operation_types(&types, &names).expect("typed host projections")
}

pub fn settings_values(stored: bool) -> SettingValues {
    use setting_value::Value;
    SettingValues {
        values: HashMap::from([
            ("enabled".into(), SettingValue { value: Some(Value::Flag(stored)) }),
            (
                "count".into(),
                SettingValue {
                    value: Some(Value::Integer(if stored { 9 } else { 3 })),
                },
            ),
            (
                "gain".into(),
                SettingValue {
                    value: Some(Value::Number(if stored { 2.5 } else { 1.5 })),
                },
            ),
            (
                "label".into(),
                SettingValue {
                    value: Some(Value::Text(if stored { "stored" } else { "default" }.into())),
                },
            ),
            (
                "interval".into(),
                SettingValue {
                    value: Some(Value::Duration(Duration {
                        seconds: if stored { 4 } else { 2 },
                        nanos: 0,
                    })),
                },
            ),
        ]),
    }
}

pub fn settings_declarations() -> SettingsDeclarations {
    let mut defaults: Vec<_> = settings_values(false).values.into_iter().collect();
    defaults.sort_by(|first, second| first.0.cmp(&second.0));
    SettingsDeclarations {
        values: defaults
            .into_iter()
            .map(|(name, value)| SettingDeclaration {
                title: format!("{}{}", name[..1].to_uppercase(), &name[1..]),
                description: format!("Conformance {name}"),
                name,
                default: Some(value),
            })
            .collect(),
    }
}

pub fn declarations() -> Declarations {
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
        "example.plugin.v1.Lifecycle/Initialize",
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
            interval: (name == "ticks").then_some(Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        })
        .collect();
    for (id, request, response) in [
        ("conformance.schema", "conformance.model.v1.Graph", "conformance.model.v1.Graph.Nested"),
        ("conformance.nested", "conformance.model.v1.Graph.Nested", "conformance.model.v1.Graph"),
        ("conformance.projected", "conformance.model.v1.Graph", ROW),
    ] {
        operations.push(OperationContract {
            id: id.into(),
            request: request.into(),
            response: response.into(),
            visibility: Visibility::Public as i32,
            fingerprint: harmos_protocol::schema::fingerprint(id, &crate::conformance::types(), request, response)
                .expect("fixture schema resolves"),
            ..Default::default()
        });
    }
    streams.push(StreamDeclaration {
        id: "conformance.settings".into(),
        row: ROW.into(),
        interval: Some(Duration { seconds: 1, nanos: 0 }),
        interval_setting: "interval".into(),
        ..Default::default()
    });
    requires.push(Requirement {
        id: "conformance.host.schema".into(),
        fingerprint: harmos_protocol::schema::fingerprint(
            "conformance.host.schema",
            &crate::conformance::types(),
            "conformance.model.v1.Graph",
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
        ("example.plugin.v1.Lifecycle/Initialize", Visibility::Protected, false),
        ("example.plugin.v1.Steps/Execute", Visibility::Public, true),
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
    for id in ["conformance.settings_read", "conformance.discover", "conformance.caller_refusal"] {
        operations.push(OperationContract {
            id: id.into(),
            request: ROW.into(),
            response: ROW.into(),
            visibility: Visibility::Public as i32,
            fingerprint: fingerprint(id),
            ..Default::default()
        });
    }
    for name in TICK_STREAMS {
        streams.push(StreamDeclaration {
            id: (*name).into(),
            row: ROW.into(),
            interval: Some(Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        });
    }
    for (id, _, _) in HOST_OPERATIONS {
        requires.push(Requirement {
            id: (*id).into(),
            fingerprint: fingerprint(id),
        });
    }
    let mut types = crate::conformance::types();
    for file in host_types().file {
        if !types.file.iter().any(|existing| existing.name == file.name) {
            types.file.push(file);
        }
    }
    for file in harmos_protocol::clock::declarations_types().file {
        if !types.file.iter().any(|existing| existing.name == file.name) {
            types.file.push(file);
        }
    }
    let mut declarations = Declarations {
        operations,
        requires,
        streams,
        types: Some(types),
        sections: vec![
            Section {
                r#type: "conformance.model.v1.Graph".into(),
                payload: Graph {
                    interval: Some(Duration { seconds: 2, nanos: 0 }),
                    ..Default::default()
                }
                .encode_to_vec(),
            },
            Section {
                r#type: "harmos.model.v1.SettingsDeclarations".into(),
                payload: settings_declarations().encode_to_vec(),
            },
        ],
        ..Default::default()
    };
    declarations.digest = harmos_protocol::schema::declarations_digest(&declarations);
    declarations
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
