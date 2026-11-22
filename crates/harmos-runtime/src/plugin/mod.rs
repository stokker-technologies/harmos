/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod grpc;
pub mod manager;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use crate::{Error, Result};

/* === Constants === */

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn fresh_generation() -> Result<u64> {
    NEXT_GENERATION
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| next.checked_add(1))
        .map_err(|_| Error::ResourceExhausted { resource: "plugin generations" })
}

/* === Private Functions === */

/* === Tests === */
