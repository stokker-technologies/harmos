/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Shared Built-In Clock Descriptors.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Complete descriptor inventory for the two built-in clock operations.
pub fn catalog_types() -> FileDescriptorSet {
    let mut result =
        types("Sleep", Some(("duration", ".google.protobuf.Duration")), time_types("Duration", "google/protobuf/duration.proto"));
    let timestamp = time_types("Timestamp", "google/protobuf/timestamp.proto");
    result.file[0].dependency.push("google/protobuf/timestamp.proto".into());
    for (name, field) in [("SleepResponse", None), ("Now", None), ("Time", Some(("now", ".google.protobuf.Timestamp")))] {
        result.file[0].message_type.extend(types(name, field, FileDescriptorSet::default()).file.remove(0).message_type);
    }
    result.file.extend(timestamp.file);
    result
}

/// Describe a built-in clock message and its imported types.
pub fn types(name: &str, field: Option<(&str, &str)>, mut dependencies: FileDescriptorSet) -> FileDescriptorSet {
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

/// Describe a protobuf Duration or Timestamp.
pub fn time_types(name: &str, file: &str) -> FileDescriptorSet {
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

/* === Private Functions === */

/* === Tests === */
