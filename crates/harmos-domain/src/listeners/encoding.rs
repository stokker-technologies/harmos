/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::mem::size_of;
// Third Party Imports

// Local Imports
use super::{Context, ContextError};
use crate::Message;
use crate::message::PostcardValue;

/* === Constants === */
const HEADERS: usize = size_of::<Context>() + 2 * size_of::<Vec<u8>>();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error("listener delivery encoding size overflows")]
    Overflow,
    #[error("listener delivery encoding requires {required} bytes, budget is {budget}")]
    Budget { required: usize, budget: usize },
    #[error("{part} encoder returned {actual} bytes, declared length is {expected}")]
    Length {
        part: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{part} encoder allocated capacity {actual}, declared capacity is {expected}")]
    Capacity {
        part: &'static str,
        expected: usize,
        actual: usize,
    },
}

/* === Traits === */

/* === Implementations === */
/* === Public Functions === */
// The Envelope Of One Delivered Row, Encoded Within A Memory Budget.
pub fn encoding_memory_bytes<R: Message>(
    context: Context,
    row: &R,
) -> Result<usize, EncodingError> {
    context.validate()?;
    let row_len = row.encoded_len();
    let encoded_len = envelope_len(context, row_len).ok_or(EncodingError::Overflow)?;
    memory_bytes(row_len, encoded_len)
}

pub fn encode_borrowed<R: Message>(
    context: Context,
    row: &R,
    total_budget: usize,
) -> Result<Vec<u8>, EncodingError> {
    context.validate()?;
    let row_len = row.encoded_len();
    let encoded_len = envelope_len(context, row_len).ok_or(EncodingError::Overflow)?;
    let required = memory_bytes(row_len, encoded_len)?;
    if required > total_budget {
        return Err(EncodingError::Budget {
            required,
            budget: total_budget,
        });
    }

    let payload = row.encode_to_vec();
    validate_buffer("row", payload.len(), payload.capacity(), row_len)?;
    let payload_capacity = payload.capacity();
    let mut encoded = Vec::with_capacity(encoded_len);
    context.write(&mut encoded);
    encoded.extend_from_slice(&payload);
    validate_buffer("envelope", encoded.len(), encoded.capacity(), encoded_len)?;
    let actual = memory_bytes(payload_capacity, encoded.capacity())?;
    if actual > total_budget {
        return Err(EncodingError::Budget {
            required: actual,
            budget: total_budget,
        });
    }
    Ok(encoded)
}

// Same Envelope Arithmetic For Owned Length Queries And Borrowed Preflight
pub(super) fn envelope_len(context: Context, row_len: usize) -> Option<usize> {
    context.size().checked_add(row_len)
}

/* === Private Functions === */
fn memory_bytes(row_capacity: usize, envelope_capacity: usize) -> Result<usize, EncodingError> {
    HEADERS
        .checked_add(row_capacity)
        .and_then(|size| size.checked_add(envelope_capacity))
        .ok_or(EncodingError::Overflow)
}

fn validate_buffer(
    part: &'static str,
    length: usize,
    capacity: usize,
    expected: usize,
) -> Result<(), EncodingError> {
    if length != expected {
        return Err(EncodingError::Length {
            part,
            expected,
            actual: length,
        });
    }
    if capacity > expected {
        return Err(EncodingError::Capacity {
            part,
            expected,
            actual: capacity,
        });
    }
    Ok(())
}

/* === Tests === */
