/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Storage Capacity: One Reservation Type And One Wait Primitive

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Third Party Imports
use tokio::time::Instant;

// Local Imports
use super::arrow::RetainedMemory;
use super::manager::{Inner, State};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub(super) struct Held(Arc<AtomicUsize>);

pub(super) struct Reservation {
    inner: Arc<Inner>,
    pool: Pool,
    bytes: usize,
    held: Held,
}

#[derive(Default)]
pub(super) struct Growers {
    next: u64,
    waiting: BTreeMap<u64, Grower>,
}

struct Grower {
    held: Held,
    need: usize,
}

struct Waiting {
    inner: Arc<Inner>,
    pool: Pool,
    id: u64,
    active: bool,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pool {
    Read,

    Write,
}

#[derive(Debug)]
pub(super) enum Refusal {
    Exceeded(Pool),

    Busy,

    Cycle,

    Elapsed,

    Failed(Error),
}

pub(super) enum Attempt<T> {
    Ready(T),
    Wait,
    Refuse(Refusal),
}

/* === Traits === */

/* === Implementations === */

impl Held {
    pub(super) fn bytes(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Pool {
    fn resource(self) -> &'static str {
        match self {
            Self::Read => "storage read memory",
            Self::Write => "storage write memory",
        }
    }

    // Owner Ceiling: Bytes One Owner Can Ever Hold In This Pool.
    fn ceiling(self, inner: &Inner) -> usize {
        match self {
            Self::Read => inner.settings.budget.read_memory_bytes.get(),
            Self::Write => inner.settings.request.max_pending_write_bytes.get() / 2,
        }
    }

    // Charge Additional Bytes When They Fit Now.
    fn charge(self, inner: &Inner, state: &mut State, bytes: usize) -> bool {
        state.collect();
        let fits = match self {
            Self::Read => state.status.read_bytes.checked_add(bytes).is_some_and(|total| total <= self.ceiling(inner)),
            Self::Write => {
                let capacity = inner.settings.request.max_pending_write_bytes.get();
                state.scratch_bytes.checked_add(bytes).is_some_and(|total| total <= capacity / 2)
                    && state.status.pending_bytes.checked_add(bytes).is_some_and(|total| total <= capacity)
            }
        };
        if !fits {
            return false;
        }
        match self {
            Self::Read => state.read_bytes += bytes,
            Self::Write => state.scratch_bytes += bytes,
        }
        state.collect();
        true
    }

    fn index(self) -> usize {
        match self {
            Self::Read => 0,
            Self::Write => 1,
        }
    }

    fn release(self, state: &mut State, bytes: usize) {
        match self {
            Self::Read => state.read_bytes -= bytes,
            Self::Write => state.scratch_bytes -= bytes,
        }
        state.collect();
    }
}

impl Refusal {
    pub(super) fn into_error(self) -> Error {
        match self {
            Self::Exceeded(pool) => Error::ResourceExhausted { resource: pool.resource() },
            Self::Busy => Error::ResourceExhausted { resource: "storage capacity" },
            Self::Cycle => Error::ResourceExhausted { resource: "storage capacity cycle" },
            Self::Elapsed => Error::ResourceExhausted { resource: "storage capacity deadline" },
            Self::Failed(error) => error,
        }
    }
}

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Self {
        refusal.into_error()
    }
}

impl Reservation {
    pub(super) fn empty(inner: &Arc<Inner>, pool: Pool, held: &Held) -> Self {
        Self { inner: Arc::clone(inner), pool, bytes: 0, held: held.clone() }
    }

    pub(super) fn charged(inner: &Arc<Inner>, pool: Pool, bytes: usize) -> Self {
        let held = Held::default();
        held.0.fetch_add(bytes, Ordering::SeqCst);
        Self { inner: Arc::clone(inner), pool, bytes, held }
    }

    pub(super) fn lifecycle(inner: &Arc<Inner>, pool: Pool, bytes: usize) -> Result<Self> {
        let mut state = inner.state.lock().unwrap();
        if bytes > pool.ceiling(inner) || !pool.charge(inner, &mut state, bytes) {
            return Err(Error::ResourceExhausted { resource: pool.resource() });
        }
        drop(state);
        let held = Held::default();
        held.0.fetch_add(bytes, Ordering::SeqCst);
        Ok(Self { inner: Arc::clone(inner), pool, bytes, held })
    }

    pub(super) fn try_new(inner: &Arc<Inner>, pool: Pool, held: &Held, bytes: usize) -> std::result::Result<Self, Refusal> {
        let mut reservation = Self::empty(inner, pool, held);
        reservation.try_grow(bytes)?;
        Ok(reservation)
    }

    pub(super) async fn wait(
        inner: &Arc<Inner>, pool: Pool, held: &Held, bytes: usize, deadline: Option<Instant>,
    ) -> std::result::Result<Self, Refusal> {
        let mut reservation = Self::empty(inner, pool, held);
        reservation.grow(bytes, deadline).await?;
        Ok(reservation)
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn try_grow(&mut self, additional: usize) -> std::result::Result<(), Refusal> {
        if additional == 0 {
            return Ok(());
        }
        let inner = Arc::clone(&self.inner);
        let mut state = inner.state.lock().unwrap();
        match self.attempt(&mut state, additional) {
            Attempt::Ready(()) => Ok(()),
            Attempt::Wait => Err(Refusal::Busy),
            Attempt::Refuse(refusal) => Err(refusal),
        }
    }

    pub(super) async fn grow(&mut self, additional: usize, deadline: Option<Instant>) -> std::result::Result<(), Refusal> {
        if additional == 0 {
            return Ok(());
        }
        let inner = Arc::clone(&self.inner);
        let mut waiting: Option<Waiting> = None;
        let result = inner.wait(deadline, |state| self.grow_step(state, additional, &mut waiting)).await;
        drop(waiting);
        result
    }

    pub(super) fn shrink_to(&mut self, bytes: usize) {
        if bytes >= self.bytes {
            return;
        }
        let released = self.bytes - bytes;
        self.bytes = bytes;
        self.held.0.fetch_sub(released, Ordering::SeqCst);
        let mut state = self.inner.state.lock().unwrap();
        self.pool.release(&mut state, released);
        drop(state);
        self.inner.notify();
    }

    pub(super) fn disown(&mut self) -> usize {
        let bytes = std::mem::take(&mut self.bytes);
        self.held.0.fetch_sub(bytes, Ordering::SeqCst);
        bytes
    }

    pub(super) fn split(&mut self, bytes: usize) -> Self {
        assert!(bytes <= self.bytes, "split exceeds reservation");
        self.bytes -= bytes;
        Self { inner: Arc::clone(&self.inner), pool: self.pool, bytes, held: self.held.clone() }
    }

    pub(super) fn retain(self) -> RetainedMemory {
        let owner = Arc::new(Mutex::new(self));
        RetainedMemory::new(owner)
    }

    fn grow_step(&mut self, state: &mut State, additional: usize, waiting: &mut Option<Waiting>) -> Attempt<()> {
        match self.attempt(state, additional) {
            Attempt::Wait if waiting.is_none() && self.held.bytes() != 0 => {
                match Waiting::join(&self.inner, state, self.pool, &self.held, additional) {
                    Some(joined) => {
                        *waiting = Some(joined);
                        Attempt::Wait
                    }
                    None => Attempt::Refuse(Refusal::Cycle),
                }
            }
            attempt => {
                if !matches!(&attempt, Attempt::Wait)
                    && let Some(waiting) = waiting
                {
                    waiting.leave(state);
                }
                attempt
            }
        }
    }

    fn attempt(&mut self, state: &mut State, additional: usize) -> Attempt<()> {
        if let Err(error) = admitting(state, self.pool) {
            return Attempt::Refuse(Refusal::Failed(error));
        }
        let owned = self.held.bytes();
        if owned.checked_add(additional).is_none_or(|total| total > self.pool.ceiling(&self.inner)) {
            return Attempt::Refuse(Refusal::Exceeded(self.pool));
        }
        if !self.pool.charge(&self.inner, state, additional) {
            if self.pool == Pool::Write {
                self.inner.relieve_pressure(state, additional, 0);
            }
            return Attempt::Wait;
        }
        self.bytes += additional;
        self.held.0.fetch_add(additional, Ordering::SeqCst);
        Attempt::Ready(())
    }
}

impl Growers {
    // Some Grower's Need Fits Once Every Owner That Is Not Growing Released Everything.
    fn can_progress(&self, ceiling: usize, held: usize, need: usize) -> bool {
        let waiters = self.waiting.values();
        let held = waiters.clone().fold(held, |total, grower| total.saturating_add(grower.held.bytes()));
        let free = ceiling.saturating_sub(held);
        need <= free || waiters.into_iter().any(|grower| grower.need <= free)
    }
}

impl Waiting {
    fn join(inner: &Arc<Inner>, state: &mut State, pool: Pool, held: &Held, need: usize) -> Option<Self> {
        let growers = &mut state.growers[pool.index()];
        if !growers.can_progress(pool.ceiling(inner), held.bytes(), need) {
            return None;
        }
        let id = growers.next;
        growers.next += 1;
        growers.waiting.insert(id, Grower { held: held.clone(), need });
        Some(Self { inner: Arc::clone(inner), pool, id, active: true })
    }

    // Admission And Departure Share The State Lock: A Running Owner Must Never
    // Be Counted As A Waiting Holder By Another Admission.
    fn leave(&mut self, state: &mut State) {
        state.growers[self.pool.index()].waiting.remove(&self.id);
        self.active = false;
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.inner.state.lock().unwrap().growers[self.pool.index()].waiting.remove(&self.id);
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.bytes == 0 {
            return;
        }
        self.held.0.fetch_sub(self.bytes, Ordering::SeqCst);
        let mut state = self.inner.state.lock().unwrap();
        self.pool.release(&mut state, self.bytes);
        drop(state);
        self.inner.notify();
    }
}

impl Inner {
    pub(super) async fn wait<T>(
        &self, deadline: Option<Instant>, mut attempt: impl FnMut(&mut State) -> Attempt<T>,
    ) -> std::result::Result<T, Refusal> {
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            match attempt(&mut self.state.lock().unwrap()) {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Refuse(refusal) => return Err(refusal),
                Attempt::Wait => {}
            }
            let notified = match deadline {
                Some(deadline) => tokio::time::timeout_at(deadline, changed.changed()).await.map_err(|_| Refusal::Elapsed)?,
                None => changed.changed().await,
            };
            notified.map_err(|_| Refusal::Failed(Error::Closing { domain: "storage" }))?;
        }
    }
}

impl Inner {
    pub(super) async fn admit_when<T>(&self, mut check: impl FnMut(&mut State) -> Result<Option<T>>) -> Result<T> {
        self.wait(None, |state| match check(state) {
            Ok(Some(value)) => Attempt::Ready(value),
            Ok(None) => Attempt::Wait,
            Err(error) => Attempt::Refuse(Refusal::Failed(error)),
        })
        .await
        .map_err(Refusal::into_error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn admitting(state: &State, pool: Pool) -> Result<()> {
    if state.status.closing {
        return Err(Error::Closing { domain: "storage" });
    }
    if !state.ready || !state.status.running {
        return Err(Error::NotReady { domain: "storage" });
    }
    if pool == Pool::Write
        && let Some(source) = &state.status.failure
    {
        return Err(Error::Persistence { source: source.clone() });
    }
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::{Data, Recovery, history, storage_settings};
    use std::num::NonZeroUsize;

    #[test]
    fn cycle_decision_accounting() {
        let mut settings = storage_settings();
        settings.budget.read_memory_bytes = NonZeroUsize::new(100).unwrap();
        settings.response.batch_bytes = NonZeroUsize::new(40).unwrap();
        let storage = crate::StorageManager::<()>::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            settings,
        )
        .unwrap();
        let inner = &storage.inner;
        {
            let mut state = inner.state.lock().unwrap();
            state.ready = true;
            state.status.running = true;
        }
        let mut a = Reservation::try_new(inner, Pool::Read, &Held::default(), 20).unwrap();
        let mut b = Reservation::try_new(inner, Pool::Read, &Held::default(), 20).unwrap();
        let external = Reservation::try_new(inner, Pool::Read, &Held::default(), 60).unwrap();
        let mut waiting_a = None;
        let mut waiting_b = None;
        {
            let mut state = inner.state.lock().unwrap();
            assert!(matches!(a.grow_step(&mut state, 40, &mut waiting_a), Attempt::Wait));
            assert!(matches!(b.grow_step(&mut state, 40, &mut waiting_b), Attempt::Wait));
        }
        drop(external);
        let mut waiting_c = None;
        let mut c = Reservation::empty(inner, Pool::Read, &Held::default());
        {
            let mut state = inner.state.lock().unwrap();
            assert!(matches!(a.grow_step(&mut state, 40, &mut waiting_a), Attempt::Ready(())));
            // A Has Not Dropped Its Wait Guard. Its Successful Admission Must
            // Already Have Left The Ledger Before C Can Enter The Same Lock.
            assert!(matches!(c.attempt(&mut state, 10), Attempt::Ready(())));
            assert!(matches!(c.grow_step(&mut state, 15, &mut waiting_c), Attempt::Wait));
        }
        drop((waiting_a, waiting_b, waiting_c));
        drop((a, b, c));
        assert_eq!(storage.status().read_bytes, 0);
    }
}
