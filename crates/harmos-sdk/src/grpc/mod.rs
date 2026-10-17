/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Protobuf Transport Adapter Shared By The Host Runtime And Sidecars.
//
// Domain messages stay codec-free: each carries its canonical Postcard value
// and its described shape. This module projects every such message onto the
// protobuf form packaging publishes, so no per-type protobuf code is generated.

/* === Modules === */
mod input;
mod transcode;
mod validator;

/* === Re-Exports === */
pub use input::{DecodeField, DecodeInput, DecodeScalar, DecodeValue};
pub use prost;
pub use validator::{PayloadError, Validator};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::__private::decoding_add;
use harmos_domain::schema::Described;
use harmos_domain::{Message, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
// These Read And Write `T`'s Message Bytes As Its Canonical Postcard Value; Host
// Adapters Whose Message Bytes Are Opaque Payloads Forward Them Instead.

/// Append the protobuf form of `value`; fails only when `T`'s Message bytes
/// break its described shape.
pub fn encode<T: Message + Described>(value: &T, bytes: &mut Vec<u8>) -> Result<(), MessageError> {
    transcode::encode(&T::DEFINITION, &value.encode_to_vec(), bytes)
}

/// Decode one protobuf `T`.
pub fn decode<T: Message + Described>(input: DecodeInput<'_>) -> Result<T, MessageError> {
    T::decode(&transcode::decode(&T::DEFINITION, input)?)
}

/// The memory decoding one protobuf `T` holds: the intermediate Postcard bytes
/// beside the decoded value.
pub fn decoding_memory<T: Message + Described>(input: DecodeInput<'_>) -> Result<usize, MessageError> {
    let postcard = transcode::decode(&T::DEFINITION, input)?;
    decoding_add(postcard.len(), T::decoding_memory(&postcard)?)
}

/* === Private Functions === */

/* === Tests === */
