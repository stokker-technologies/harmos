/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Descriptor Fingerprints, Caller Projection and Canonical Message Graphs.

/* === Modules === */

mod callers;
mod graph;
mod identity;

/* === Re-Exports === */

pub use callers::{method_callers, with_method_callers};
pub use graph::{merge_message_types, message_types, operation_types};
pub use identity::{Executor, operation_identity, plugin_operation_identity};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;
use prost_types::FileDescriptorSet;
use sha2::{Digest, Sha256};

// Local Imports
use crate::model::{Declarations, Protobuf};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug, thiserror::Error)]
#[error("Invalid Protobuf Schema: {0}")]
pub struct Error(String);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Hash an operation's reachable request and response descriptors, independently
/// of unrelated declarations. See the v1 behaviour contract.
pub fn fingerprint(id: &str, types: &FileDescriptorSet, request: &str, response: &str) -> Result<Vec<u8>, Error> {
    let request = message_types(types, request)?;
    let response = message_types(types, response)?;
    let mut hash = Sha256::new();
    hash.update(id.as_bytes());
    hash.update([0]);
    hash.update(request.encode_to_vec());
    hash.update(response.encode_to_vec());
    Ok(hash.finalize().to_vec())
}

/// Fingerprint A Host Operation, Including Its Optional Stream Message Graphs.
pub fn host_fingerprint(id: &str, contract: &Protobuf) -> Result<Vec<u8>, Error> {
    let descriptors = FileDescriptorSet::decode(contract.descriptors.as_slice()).map_err(|error| Error(error.to_string()))?;
    let fingerprint = fingerprint(id, &descriptors, &contract.request, &contract.response)?;
    if contract.input.is_none() && contract.output.is_none() {
        return Ok(fingerprint);
    }
    let mut hash = Sha256::new();
    hash.update(b"harmos.protobuf.stream.v1\0");
    hash.update(fingerprint);
    for message in [&contract.input, &contract.output] {
        match message {
            None => hash.update([0]),
            Some(message) => {
                hash.update([1]);
                hash.update(self::fingerprint(id, &descriptors, message, message)?);
            }
        }
    }
    Ok(hash.finalize().to_vec())
}

/// Compute the wire declarations digest without its advertised digest field.
pub fn declarations_digest(declarations: &Declarations) -> Vec<u8> {
    let mut declarations = declarations.clone();
    declarations.digest.clear();
    Sha256::digest(declarations.encode_to_vec()).to_vec()
}

/// Preprojected Message Options for Validation Without Loading Unrelated Meta-Schemas.
pub fn descriptor_options() -> FileDescriptorSet {
    FileDescriptorSet::decode(include_bytes!("../message-options-descriptors.bin").as_slice())
        .expect("generated message option descriptors")
}

/* === Private Functions === */

/* === Tests === */
