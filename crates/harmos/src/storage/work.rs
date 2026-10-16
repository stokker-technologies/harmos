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
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

// Third Party Imports
use futures_core::Stream;

// Local Imports
use super::manager::MemoryReservation;
use super::production::Execution;
use super::{StorageProductionComputation, StorageView, StorageViewComputation};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct StorageReadWorkPreparation {
    profile: Profile,
    active: Active,
}

pub struct StorageReadWork<F> {
    work: Pin<Box<F>>,
    _profile: Profile,
    _reservation: MemoryReservation,
}

/* === Enums === */
enum Profile {
    Saved(StorageView),
    Producer(Arc<Execution>),
}

enum Active {
    Saved(StorageViewComputation),
    Producer(StorageProductionComputation),
}

/* === Traits === */

/* === Implementations === */
impl StorageReadWorkPreparation {
    pub(super) fn saved(view: StorageView) -> Result<Self> {
        let active = view.computation()?.ok_or(Error::InvalidInput {
            field: "storage read work",
            reason: "original view has no bounded computation profile",
        })?;
        Ok(Self {
            profile: Profile::Saved(view),
            active: Active::Saved(active),
        })
    }

    pub(super) fn producer(execution: Arc<Execution>) -> Result<Self> {
        let active = execution.begin()?;
        Ok(Self {
            profile: Profile::Producer(execution),
            active: Active::Producer(active),
        })
    }

    pub fn boxed<F>(self, work: F) -> Result<Pin<Box<StorageReadWork<F>>>> {
        self.active.check()?;
        let bytes = std::mem::size_of::<F>()
            .checked_add(std::mem::size_of::<StorageReadWork<F>>())
            .and_then(NonZeroUsize::new)
            .ok_or_else(super::query::decoded_limit)?;
        let reservation = self.profile.reserve(bytes)?;
        let Self { profile, active } = self;
        let work = Box::pin(StorageReadWork {
            work: Box::pin(work),
            _profile: profile,
            _reservation: reservation,
        });
        active.check()?;
        Ok(work)
    }
}

impl Profile {
    fn reserve(&self, bytes: NonZeroUsize) -> Result<MemoryReservation> {
        match self {
            Self::Saved(view) => view.reserve_read(bytes.get(), 1),
            Self::Producer(execution) => execution.reserve_output(bytes),
        }
    }
}

impl Active {
    fn check(&self) -> Result<()> {
        match self {
            Self::Saved(active) => active.check(),
            Self::Producer(active) => active.check(),
        }
    }
}

impl<F: Future> Future for StorageReadWork<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().work.as_mut().poll(context)
    }
}

impl<S: Stream> Stream for StorageReadWork<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().work.as_mut().poll_next(context)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.work.size_hint()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
