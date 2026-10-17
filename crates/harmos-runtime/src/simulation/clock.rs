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
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub(crate) struct Clock {
    elapsed_ms: Arc<AtomicU64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Clock {
    pub(crate) fn new() -> Self {
        Self { elapsed_ms: Arc::new(AtomicU64::new(0)) }
    }

    pub(crate) async fn advance_ms(&self, milliseconds: u64) {
        tokio::time::advance(Duration::from_millis(milliseconds)).await;
        self.elapsed_ms.fetch_add(milliseconds, Ordering::AcqRel);
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms.load(Ordering::Acquire)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
