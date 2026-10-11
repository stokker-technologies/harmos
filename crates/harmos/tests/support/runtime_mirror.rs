/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use harmos::{Application, Entry, JournalConsumer, Position, Recorded};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Watched = (Arc<AtomicU64>, Arc<std::sync::Mutex<Vec<Position>>>);

/* === Structs === */
pub(crate) struct Mirror {
    checkpoint: Arc<AtomicU64>,
    seen: Arc<std::sync::Mutex<Vec<Position>>>,
    ceiling: Option<Position>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Mirror {
    pub(crate) fn new() -> (Self, Watched) {
        let checkpoint = Arc::new(AtomicU64::new(0));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));

        (
            Self {
                checkpoint: Arc::clone(&checkpoint),
                seen: Arc::clone(&seen),
                ceiling: None,
            },
            (checkpoint, seen),
        )
    }

    pub(crate) fn stalled_at(ceiling: Position) -> (Self, Watched) {
        let (mut mirror, watched) = Self::new();
        mirror.ceiling = Some(ceiling);

        (mirror, watched)
    }
}

impl<A: Application> JournalConsumer<A> for Mirror {
    fn resume(&self) -> Position {
        Position(self.checkpoint.load(Ordering::Acquire))
    }

    async fn drain(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<A>>>>,
        applied: Position,
    ) -> Position {
        self.seen
            .lock()
            .expect("the probe is never poisoned")
            .extend(entries.iter().map(|entry| entry.position));

        let reached = self.ceiling.map_or(applied, |ceiling| applied.min(ceiling));
        self.checkpoint.store(reached.0, Ordering::Release);

        reached
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
