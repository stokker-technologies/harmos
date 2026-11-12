/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Protocol Helpers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::v1;
use prost::{
    Message,
    bytes::Buf,
    encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field},
};
use std::{collections::BTreeSet, sync::OnceLock};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const COMMAND_BYTES: usize = 16 * 1024 * 1024;
pub const CONTEXT_VALUES: usize = 64;
pub const CONTEXT_BYTES: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug, thiserror::Error)]
#[error("Invalid Guest Message: {0}")]
pub struct Error(String);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> &'static prost_types::FileDescriptorSet {
    static TYPES: OnceLock<prost_types::FileDescriptorSet> = OnceLock::new();
    TYPES.get_or_init(|| {
        prost_types::FileDescriptorSet::decode(include_bytes!("guest-descriptors.bin").as_slice()).expect("generated protocol descriptors")
    })
}

pub fn lifecycle_fingerprint(operation: &str) -> Result<Vec<u8>, Error> {
    let request = match operation {
        "harmos.initialize" => "harmos.v1.GuestInitialize",
        "harmos.shutdown" => "harmos.v1.GuestShutdown",
        _ => return Err(Error("Unknown Lifecycle Action".into())),
    };
    let mut types = types().clone();
    types.file.push(prost_types::FileDescriptorProto {
        name: Some("google/protobuf/empty.proto".into()),
        package: Some("google.protobuf".into()),
        syntax: Some("proto3".into()),
        message_type: vec![prost_types::DescriptorProto {
            name: Some("Empty".into()),
            ..Default::default()
        }],
        ..Default::default()
    });
    crate::schema::fingerprint(operation, &types, request, "google.protobuf.Empty").map_err(|error| Error(error.to_string()))
}

pub fn encode<M: Message>(message: &M) -> Result<Vec<u8>, Error> {
    if message.encoded_len() > COMMAND_BYTES {
        return Err(Error("Byte Bound Exceeded".into()));
    }
    Ok(message.encode_to_vec())
}

pub fn decode_command(bytes: &[u8]) -> Result<v1::GuestCommand, Error> {
    validate_envelope(bytes, false)?;
    let command = v1::GuestCommand::decode(bytes).map_err(|error| Error(error.to_string()))?;
    if command.phase.is_none() {
        return Err(Error("Command Phase Is Required".into()));
    }
    if let Some(v1::guest_command::Phase::Initialize(invocation) | v1::guest_command::Phase::Attempt(invocation)) = &command.phase {
        validate_values(&invocation.values)?;
    }
    Ok(command)
}

pub fn decode_child(bytes: &[u8]) -> Result<v1::GuestChild, Error> {
    validate_envelope(bytes, true)?;
    let child = v1::GuestChild::decode(bytes).map_err(|error| Error(error.to_string()))?;
    if child.action.is_none() {
        return Err(Error("Child Action Is Required".into()));
    }
    if let Some(v1::guest_child::Action::Call(call)) = &child.action {
        validate_values(&call.values)?;
    }
    Ok(child)
}

pub fn validate_values(values: &[v1::GuestValue]) -> Result<(), Error> {
    let mut keys = BTreeSet::new();
    let mut bytes = 0usize;
    for value in values {
        if value.key.is_empty() || !keys.insert(&value.key) {
            return Err(Error("Invalid or Duplicate Context Value".into()));
        }
        bytes = bytes
            .checked_add(value.key.len())
            .and_then(|n| n.checked_add(value.payload.len()))
            .ok_or_else(|| Error("Context Size Overflow".into()))?;
    }
    if values.len() > CONTEXT_VALUES || bytes > CONTEXT_BYTES {
        return Err(Error("Context Bound Exceeded".into()));
    }
    Ok(())
}

/* === Private Functions === */

// Inspect Repeated Context Entries Before Prost Allocates Their Owned Values.
fn validate_envelope(mut bytes: &[u8], child: bool) -> Result<(), Error> {
    if bytes.len() > COMMAND_BYTES {
        return Err(Error("Byte Bound Exceeded".into()));
    }
    let mut budget = (0usize, 0usize);
    while bytes.has_remaining() {
        let (tag, wire) = decode_key(&mut bytes).map_err(encoding)?;
        let values = if child { (tag == 1).then_some(6) } else { matches!(tag, 2 | 3).then_some(3) };
        if wire != WireType::LengthDelimited || values.is_none() {
            skip_field(wire, tag, &mut bytes, DecodeContext::default()).map_err(encoding)?;
            continue;
        }
        let nested = delimited(&mut bytes)?;
        validate_nested(nested, values.expect("selected value field"), &mut budget)?;
    }
    Ok(())
}
fn validate_nested(mut bytes: &[u8], values_tag: u32, budget: &mut (usize, usize)) -> Result<(), Error> {
    while bytes.has_remaining() {
        let (tag, wire) = decode_key(&mut bytes).map_err(encoding)?;
        if tag != values_tag || wire != WireType::LengthDelimited {
            skip_field(wire, tag, &mut bytes, DecodeContext::default()).map_err(encoding)?;
            continue;
        }
        let value = delimited(&mut bytes)?;
        budget.0 += 1;
        budget.1 = budget.1.checked_add(value.len()).ok_or_else(|| Error("Context Size Overflow".into()))?;
        // Entry framing is bounded separately from authored key/payload bytes.
        if budget.0 > CONTEXT_VALUES || budget.1 > CONTEXT_BYTES + CONTEXT_VALUES * 20 {
            return Err(Error("Context Bound Exceeded".into()));
        }
    }
    Ok(())
}
fn delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let size = usize::try_from(decode_varint(bytes).map_err(encoding)?).map_err(|error| Error(error.to_string()))?;
    if size > bytes.len() {
        return Err(Error("Truncated Message".into()));
    }
    let (value, rest) = bytes.split_at(size);
    *bytes = rest;
    Ok(value)
}
fn encoding(error: prost::DecodeError) -> Error {
    Error(error.to_string())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiving_context_rejects_amplification_and_malformed_values() {
        let command = |values| {
            v1::GuestCommand {
                phase: Some(v1::guest_command::Phase::Attempt(v1::GuestInvocation {
                    invocation: 1,
                    context: vec![1; 32],
                    values,
                })),
            }
            .encode_to_vec()
        };
        let valid = v1::GuestValue {
            key: "trace".into(),
            payload: vec![1, 2, 3],
        };
        assert!(decode_command(&command(vec![valid.clone()])).is_ok());
        assert!(decode_command(&command(vec![valid.clone(), valid.clone()])).is_err());
        let fragment = command(vec![valid]);
        assert!(decode_command(&fragment.repeat(CONTEXT_VALUES + 1)).is_err());
        assert!(decode_command(&[0x1a, 0xff]).is_err());
        assert!(decode_command(&command(vec![v1::GuestValue { key: String::new(), payload: Vec::new() }])).is_err());
        assert!(
            decode_command(&command(vec![v1::GuestValue {
                key: "large".into(),
                payload: vec![0; CONTEXT_BYTES]
            }]))
            .is_err()
        );
    }
}
