/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Operation Contracts

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::v1::{Now, Sleep, SleepResponse, Time};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};

// Local Imports
use crate::{Message, OperationRequest};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for Sleep {
    fn name() -> String {
        "harmos.v1.Sleep".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Sleep", Some(("duration", ".google.protobuf.Duration")), prost_types::Duration::types())
    }
}

impl Message for SleepResponse {
    fn name() -> String {
        "harmos.v1.SleepResponse".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("SleepResponse", None, FileDescriptorSet::default())
    }
}

impl Message for Now {
    fn name() -> String {
        "harmos.v1.Now".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Now", None, FileDescriptorSet::default())
    }
}

impl Message for Time {
    fn name() -> String {
        "harmos.v1.Time".into()
    }
    fn types() -> FileDescriptorSet {
        clock_types("Time", Some(("now", ".google.protobuf.Timestamp")), prost_types::Timestamp::types())
    }
}

impl OperationRequest for Sleep {
    type Response = SleepResponse;
    const ID: &'static str = "harmos.clock.sleep";
}
impl OperationRequest for Now {
    type Response = Time;
    const ID: &'static str = "harmos.clock.now";
}

impl Message for prost_types::Duration {
    fn name() -> String {
        "google.protobuf.Duration".into()
    }
    fn types() -> FileDescriptorSet {
        time_types("Duration", "google/protobuf/duration.proto")
    }
}

impl Message for prost_types::Timestamp {
    fn name() -> String {
        "google.protobuf.Timestamp".into()
    }
    fn types() -> FileDescriptorSet {
        time_types("Timestamp", "google/protobuf/timestamp.proto")
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn clock_types(name: &str, field: Option<(&str, &str)>, mut dependencies: FileDescriptorSet) -> FileDescriptorSet {
    let dependency = dependencies.file.iter().filter_map(|file| file.name.clone()).collect();
    let field = field
        .map(|(name, type_name)| FieldDescriptorProto {
            name: Some(name.into()),
            number: Some(1),
            label: Some(1),
            r#type: Some(11),
            type_name: Some(type_name.into()),
            ..Default::default()
        })
        .into_iter()
        .collect();
    dependencies.file.insert(
        0,
        FileDescriptorProto {
            name: Some("harmos/v1/plugin.proto".into()),
            package: Some("harmos.v1".into()),
            syntax: Some("proto3".into()),
            dependency,
            message_type: vec![DescriptorProto {
                name: Some(name.into()),
                field,
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    dependencies
}

fn time_types(name: &str, file: &str) -> FileDescriptorSet {
    FileDescriptorSet {
        file: vec![FileDescriptorProto {
            name: Some(file.into()),
            package: Some("google.protobuf".into()),
            syntax: Some("proto3".into()),
            message_type: vec![DescriptorProto {
                name: Some(name.into()),
                field: vec![
                    FieldDescriptorProto {
                        name: Some("seconds".into()),
                        number: Some(1),
                        label: Some(1),
                        r#type: Some(3),
                        ..Default::default()
                    },
                    FieldDescriptorProto {
                        name: Some("nanos".into()),
                        number: Some(2),
                        label: Some(1),
                        r#type: Some(5),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
}

/* === Tests === */
