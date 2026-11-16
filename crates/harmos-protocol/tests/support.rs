/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Descriptor Graph Fixture

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_types::{
    DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> FileDescriptorSet {
    FileDescriptorSet {
        file: vec![
            FileDescriptorProto {
                name: Some("fixture.proto".into()),
                package: Some("fixture".into()),
                syntax: Some("proto3".into()),
                dependency: vec!["status.proto".into()],
                message_type: vec![
                    DescriptorProto {
                        name: Some("Value".into()),
                        field: vec![FieldDescriptorProto {
                            name: Some("value".into()),
                            number: Some(1),
                            label: Some(1),
                            r#type: Some(12),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    DescriptorProto {
                        name: Some("Node".into()),
                        field: vec![
                            FieldDescriptorProto {
                                name: Some("children".into()),
                                number: Some(1),
                                label: Some(3),
                                r#type: Some(11),
                                type_name: Some(".fixture.Node".into()),
                                ..Default::default()
                            },
                            FieldDescriptorProto {
                                name: Some("value".into()),
                                number: Some(2),
                                label: Some(1),
                                r#type: Some(11),
                                type_name: Some(".fixture.Value".into()),
                                ..Default::default()
                            },
                            FieldDescriptorProto {
                                name: Some("status".into()),
                                number: Some(3),
                                label: Some(1),
                                r#type: Some(14),
                                type_name: Some(".status.Status".into()),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            FileDescriptorProto {
                name: Some("status.proto".into()),
                package: Some("status".into()),
                syntax: Some("proto3".into()),
                enum_type: vec![EnumDescriptorProto {
                    name: Some("Status".into()),
                    value: vec![EnumValueDescriptorProto {
                        name: Some("UNKNOWN".into()),
                        number: Some(0),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
    }
}

/* === Private Functions === */

/* === Tests === */
