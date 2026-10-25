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

// Third Party Imports

// Local Imports
use super::{At, Error, Origin, Position, TrackedState};
use crate::Application;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ManagedState<A: Application> {
    tracked: Arc<TrackedState<A>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> ManagedState<A> {
    pub(super) fn new(tracked: Arc<TrackedState<A>>) -> Self {
        Self { tracked }
    }

    #[tracing::instrument(target = "harmos::state", name = "state.read", level = "debug", skip_all)]
    pub async fn read<R>(&self, read: impl FnOnce(&A::State) -> R) -> Result<At<R>, Error> {
        if self.tracked.poisoned() {
            return Err(self.tracked.unavailable());
        }
        let state = self.tracked.state.read().await;
        // The Writer May Have Panicked While This Reader Awaited The Lock.
        if self.tracked.poisoned() {
            return Err(self.tracked.unavailable());
        }

        Ok(At { value: read(&state.value), position: state.position })
    }

    #[tracing::instrument(target = "harmos::state", name = "state.at", level = "debug", skip_all)]
    pub async fn at(&self, origin: Origin<A::State>, at: Position) -> Result<At<A::State>, Error> {
        if self.tracked.poisoned() {
            return Err(self.tracked.unavailable());
        }

        {
            let tail = self.tracked.tail.read().await;
            if tail.splits_commit(at) || tail.splits_commit(origin.position) {
                return Err(Error::IncompleteCommit);
            }
        }
        self.tracked.reads.state(origin, at).await
    }
}

impl<A: Application> Clone for ManagedState<A> {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.tracked))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
