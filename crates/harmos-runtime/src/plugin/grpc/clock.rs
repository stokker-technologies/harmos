/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// TODO: Describe this module's responsibility.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use std::time::Duration;

// Standard Library Imports

use prost::Message;
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};

// Third Party Imports

use super::{invoke::CallError, v1};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn contracts() -> Vec<v1::OperationContract> {
    [("sleep", "Sleep", "SleepResponse", Some(("duration", "Duration"))), ("now", "Now", "Time", Some(("now", "Timestamp")))]
        .into_iter()
        .map(|(id, request, response, field)| {
            let request_types = types(request, if id == "sleep" { field } else { None });
            let response_types = types(response, if id == "now" { field } else { None });
            let mut descriptors = request_types;
            for file in response_types.file {
                if let Some(existing) = descriptors.file.iter_mut().find(|existing| existing.name == file.name) {
                    existing.message_type.extend(file.message_type);
                    existing.dependency.extend(file.dependency);
                    existing.dependency.sort();
                    existing.dependency.dedup();
                } else {
                    descriptors.file.push(file);
                }
            }
            let id = format!("harmos.clock.{id}");
            let request = format!("harmos.v1.{request}");
            let response = format!("harmos.v1.{response}");
            let fingerprint = harmos_protocol::schema::fingerprint(&id, &descriptors, &request, &response).unwrap();
            v1::OperationContract {
                id,
                request,
                response,
                fingerprint,
                visibility: v1::Visibility::Public as i32,
                ..Default::default()
            }
        })
        .collect()
}

pub(crate) async fn run(clock: &crate::clock::Clock, operation: &str, payload: &[u8]) -> Result<Vec<u8>, CallError> {
    let invalid = |error: String| CallError::Transport(tonic::Status::invalid_argument(error));
    if operation == "harmos.clock.now" {
        v1::Now::decode(payload).map_err(|error| invalid(error.to_string()))?;
        return Ok(v1::Time { now: Some(clock.date().into()) }.encode_to_vec());
    }
    let request = v1::Sleep::decode(payload).map_err(|error| invalid(error.to_string()))?;
    let duration = request.duration.unwrap_or_default();
    if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) || duration.seconds > 315_576_000_000 {
        return Err(invalid("Invalid Sleep Duration".into()));
    }
    crate::clock::waiting(clock.sleep(Duration::new(duration.seconds as u64, duration.nanos as u32))).await;
    Ok(v1::SleepResponse {}.encode_to_vec())
}

/* === Private Functions === */

fn types(name: &str, field: Option<(&str, &str)>) -> FileDescriptorSet {
    let mut file = vec![FileDescriptorProto {
        name: Some("harmos/v1/plugin.proto".into()),
        package: Some("harmos.v1".into()),
        syntax: Some("proto3".into()),
        message_type: vec![DescriptorProto {
            name: Some(name.into()),
            field: field
                .map(|(name, kind)| FieldDescriptorProto {
                    name: Some(name.into()),
                    number: Some(1),
                    label: Some(1),
                    r#type: Some(11),
                    type_name: Some(format!(".google.protobuf.{kind}")),
                    ..Default::default()
                })
                .into_iter()
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    }];
    if let Some((_, kind)) = field {
        let path = format!("google/protobuf/{}.proto", kind.to_lowercase());
        file[0].dependency.push(path.clone());
        file.push(FileDescriptorProto {
            name: Some(path),
            package: Some("google.protobuf".into()),
            syntax: Some("proto3".into()),
            message_type: vec![DescriptorProto {
                name: Some(kind.into()),
                field: [("seconds", 1, 3), ("nanos", 2, 5)]
                    .into_iter()
                    .map(|(name, number, kind)| FieldDescriptorProto {
                        name: Some(name.into()),
                        number: Some(number),
                        label: Some(1),
                        r#type: Some(kind),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    FileDescriptorSet { file }
}

/* === Tests === */
