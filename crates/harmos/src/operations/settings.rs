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
use std::num::NonZeroUsize;

// Third Party Imports
use tokio::sync::Semaphore;

// Local Imports
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub queued: NonZeroUsize,
    pub lifecycle_queued: NonZeroUsize,
    pub preparation_bytes: Option<NonZeroUsize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for Settings {
    fn default() -> Self {
        Self {
            queued: NonZeroUsize::new(256).expect("nonzero ordinary admission bound"),
            lifecycle_queued: NonZeroUsize::new(256).expect("nonzero lifecycle admission bound"),
            preparation_bytes: None,
        }
    }
}

impl Settings {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.queued.get() > Semaphore::MAX_PERMITS
            || self.lifecycle_queued.get() > Semaphore::MAX_PERMITS
        {
            return Err(Error::InvalidInput {
                field: "operation queue capacity",
                reason: "capacity exceeds the semaphore claim limit",
            });
        }
        if self.preparation_bytes.is_some_and(|bytes| {
            bytes.get() > Semaphore::MAX_PERMITS || bytes.get() > u32::MAX as usize
        }) {
            return Err(Error::InvalidInput {
                field: "operation preparation bytes",
                reason: "budget exceeds the semaphore byte-claim limit",
            });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
