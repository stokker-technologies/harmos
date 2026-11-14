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
use crate::v1::{StorageTransferSettings, TransferFraming};

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
    NonZeroUsize::new(crate::v1::MessageSettings::default().message_bytes() as usize).expect("positive default message ceiling")
}

pub fn message_bytes(settings: &StorageTransferSettings) -> Option<NonZeroUsize> {
    let ceiling = default_message_bytes();
    (required_message_bytes(settings)? <= ceiling).then_some(ceiling)
}

/* === Private Functions === */

/* === Tests === */
