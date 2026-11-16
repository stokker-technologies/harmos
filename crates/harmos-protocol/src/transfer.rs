/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage-Owned Transfer Framing

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use crate::model::{MessageSettings, StorageTransferSettings, TransferFraming};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn required_message_bytes(settings: &StorageTransferSettings) -> Option<NonZeroUsize> {
    let chunk = usize::try_from(settings.chunk_bytes()).ok().and_then(NonZeroUsize::new)?;
    chunk.checked_add(TransferFraming::Bytes as usize)
}

pub fn default_message_bytes() -> NonZeroUsize {
    let message_bytes = MessageSettings::default().message_bytes() as usize;
    NonZeroUsize::new(message_bytes).expect("positive default message ceiling")
}

pub fn message_bytes(settings: &StorageTransferSettings) -> Option<NonZeroUsize> {
    let ceiling = default_message_bytes();
    let required = required_message_bytes(settings)?;
    (required <= ceiling).then_some(required)
}

/* === Private Functions === */

/* === Tests === */
