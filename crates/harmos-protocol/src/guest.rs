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

// Standard Library Imports
use std::{collections::BTreeSet, sync::OnceLock};

// Third Party Imports
use prost::{
    DecodeError, Message,
    bytes::Buf,
    encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field},
};
use prost_types::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};

// Local Imports
use crate::{plugin, schema};

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

#[derive(Default)]
struct ContextBudget {
    values: usize,
    encoded_bytes: usize,
}

/* === Enums === */

enum Envelope {
    Command,
    Child,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> &'static FileDescriptorSet {
    static TYPES: OnceLock<FileDescriptorSet> = OnceLock::new();
    TYPES.get_or_init(|| {
        FileDescriptorSet::decode(include_bytes!("guest-descriptors.bin").as_slice()).expect("generated protocol descriptors")
    })
}

pub fn lifecycle_fingerprint(operation: &str) -> Result<Vec<u8>, Error> {
    let request = match operation {
        "harmos.initialize" => "harmos.plugin.v1.GuestInitialize",
        "harmos.shutdown" => "harmos.plugin.v1.GuestShutdown",
        _ => return Err(Error("Declare Guest Lifecycle: Select Initialize or Shutdown".into())),
    };
    let mut types = types().clone();
    types.file.push(FileDescriptorProto {
        name: Some("google/protobuf/empty.proto".into()),
        package: Some("google.protobuf".into()),
        syntax: Some("proto3".into()),
        message_type: vec![DescriptorProto {
            name: Some("Empty".into()),
            ..Default::default()
        }],
        ..Default::default()
    });
    schema::fingerprint(operation, &types, request, "google.protobuf.Empty").map_err(|error| Error(error.to_string()))
}

pub fn encode<M: Message>(message: &M) -> Result<Vec<u8>, Error> {
    if message.encoded_len() > COMMAND_BYTES {
        return Err(Error("Send Guest Message: Reduce the Encoded Payload Below the Command Byte Bound".into()));
    }
    Ok(message.encode_to_vec())
}

pub fn decode_command(bytes: &[u8]) -> Result<plugin::GuestCommand, Error> {
    validate_envelope(bytes, Envelope::Command)?;
    let command = plugin::GuestCommand::decode(bytes).map_err(encoding)?;
    if command.phase.is_none() {
        return Err(Error("Send Guest Command: Select a Command Phase".into()));
    }
    if let Some(plugin::guest_command::Phase::Initialize(invocation) | plugin::guest_command::Phase::Attempt(invocation)) = &command.phase {
        validate_values(&invocation.values)?;
    }
    Ok(command)
}

pub fn decode_child(bytes: &[u8]) -> Result<plugin::GuestChild, Error> {
    validate_envelope(bytes, Envelope::Child)?;
    let child = plugin::GuestChild::decode(bytes).map_err(encoding)?;
    if child.action.is_none() {
        return Err(Error("Send Guest Child: Select a Child Action".into()));
    }
    if let Some(plugin::guest_child::Action::Call(call)) = &child.action {
        validate_values(&call.values)?;
    }
    Ok(child)
}

pub fn validate_values(values: &[plugin::GuestValue]) -> Result<(), Error> {
    let mut keys = BTreeSet::new();
    let mut bytes = 0usize;
    for value in values {
        if value.key.is_empty() || !keys.insert(&value.key) {
            return Err(Error("Send Guest Context: Supply Unique Nonempty Value Keys".into()));
        }
        bytes = bytes
            .checked_add(value.key.len())
            .and_then(|bytes| bytes.checked_add(value.payload.len()))
            .ok_or_else(|| Error("Send Guest Context: Reduce the Encoded Context Size".into()))?;
    }
    if values.len() > CONTEXT_VALUES || bytes > CONTEXT_BYTES {
        return Err(Error("Send Guest Context: Reduce the Value Count or Encoded Context Size".into()));
    }
    Ok(())
}

/* === Private Functions === */

// Inspect Repeated Context Entries Before Prost Allocates Their Owned Values.
fn validate_envelope(mut bytes: &[u8], envelope: Envelope) -> Result<(), Error> {
    if bytes.len() > COMMAND_BYTES {
        return Err(Error("Send Guest Message: Reduce the Encoded Payload Below the Command Byte Bound".into()));
    }
    let mut budget = ContextBudget::default();
    while bytes.has_remaining() {
        let (tag, wire) = decode_key(&mut bytes).map_err(encoding)?;
        let values = match envelope {
            Envelope::Command => matches!(tag, 2 | 3).then_some(3),
            Envelope::Child => (tag == 1).then_some(6),
        };
        if wire != WireType::LengthDelimited || values.is_none() {
            skip_field(wire, tag, &mut bytes, DecodeContext::default()).map_err(encoding)?;
            continue;
        }
        let nested = delimited(&mut bytes)?;
        validate_nested(nested, values.expect("selected value field"), &mut budget)?;
    }
    Ok(())
}

fn validate_nested(mut bytes: &[u8], values_tag: u32, budget: &mut ContextBudget) -> Result<(), Error> {
    while bytes.has_remaining() {
        let (tag, wire) = decode_key(&mut bytes).map_err(encoding)?;
        if tag != values_tag || wire != WireType::LengthDelimited {
            skip_field(wire, tag, &mut bytes, DecodeContext::default()).map_err(encoding)?;
            continue;
        }
        let value = delimited(&mut bytes)?;
        budget.values += 1;
        budget.encoded_bytes = budget
            .encoded_bytes
            .checked_add(value.len())
            .ok_or_else(|| Error("Send Guest Context: Reduce the Encoded Context Size".into()))?;
        // Entry framing is bounded separately from authored key/payload bytes.
        let too_many_values = budget.values > CONTEXT_VALUES;
        let too_many_bytes = budget.encoded_bytes > CONTEXT_BYTES + CONTEXT_VALUES * 20;
        if too_many_values || too_many_bytes {
            return Err(Error("Send Guest Context: Reduce the Value Count or Encoded Context Size".into()));
        }
    }
    Ok(())
}

fn delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let size = usize::try_from(decode_varint(bytes).map_err(encoding)?).map_err(|error| Error(error.to_string()))?;
    if size > bytes.len() {
        return Err(Error("Decode Guest Message: Supply the Complete Length-Delimited Payload".into()));
    }
    let (value, rest) = bytes.split_at(size);
    *bytes = rest;
    Ok(value)
}

fn encoding(error: DecodeError) -> Error {
    Error(format!("Decode Guest Message: Supply Valid Protobuf Bytes: {error}"))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiving_context_rejects_amplification_and_malformed_values() {
        let command = |values| {
            plugin::GuestCommand {
                phase: Some(plugin::guest_command::Phase::Attempt(plugin::GuestInvocation {
                    invocation: 1,
                    context: vec![1; 32],
                    values,
                })),
            }
            .encode_to_vec()
        };
        let valid = plugin::GuestValue {
            key: "trace".into(),
            payload: vec![1, 2, 3],
        };
        assert!(decode_command(&command(vec![valid.clone()])).is_ok());
        assert!(decode_command(&command(vec![valid.clone(), valid.clone()])).is_err());
        let fragment = command(vec![valid]);
        assert!(decode_command(&fragment.repeat(CONTEXT_VALUES + 1)).is_err());
        assert!(decode_command(&[0x1a, 0xff]).is_err());
        assert!(decode_command(&command(vec![plugin::GuestValue { key: String::new(), payload: Vec::new() }])).is_err());
        assert!(
            decode_command(&command(vec![plugin::GuestValue {
                key: "large".into(),
                payload: vec![0; CONTEXT_BYTES]
            }]))
            .is_err()
        );
    }
}
