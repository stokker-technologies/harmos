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
use crate::storage::{StorageTransferSettings, TransferFraming};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn message_bytes(settings: &StorageTransferSettings) -> Option<NonZeroUsize> {
    let chunk = usize::try_from(settings.chunk_bytes()).ok().and_then(NonZeroUsize::new)?;
    chunk.checked_add(TransferFraming::Bytes as usize)
}

pub fn default_message_bytes() -> NonZeroUsize {
    message_bytes(&StorageTransferSettings::default()).expect("representable default transfer settings")
}

/* === Private Functions === */

/* === Tests === */
