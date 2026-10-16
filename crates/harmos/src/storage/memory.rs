/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Read Work Budget: One Admitted Attempt Growing On Concrete Demand

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{Arc, Mutex};

// Third Party Imports
use tokio::time::Instant;

// Local Imports
use super::arrow::RetainedMemory;
use super::capacity::{Held, Pool, Refusal, Reservation};
use super::manager::Inner;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// The budget of one reloadable unit of read work. Scoped claims cover transient
/// scratch; retained claims move bytes into their own reservations that outlive
/// the attempt. Growth never waits; a refusal is recorded so the caller can
/// release the attempt, wait for the recorded demand and retry the same work.
pub(super) struct MemoryBank {
    state: Arc<Mutex<BankState>>,
}

pub(super) struct MemoryScope<'a> {
    bank: &'a MemoryBank,
    used: usize,
}

struct BankState {
    reservation: Reservation,
    used: usize,
    retained: usize,
    requested: usize,
    refusal: Option<Shortfall>,
}

/* === Enums === */
/// Why an attempt could not grow: other owners hold capacity, or it can never fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shortfall {
    Busy,
    Exceeded,
}

/* === Traits === */

/* === Implementations === */
impl MemoryBank {
    pub(super) fn new(reservation: Reservation) -> Self {
        Self {
            state: Arc::new(Mutex::new(BankState {
                requested: reservation.bytes(),
                reservation,
                used: 0,
                retained: 0,
                refusal: None,
            })),
        }
    }

    /// Admit `bytes` of credit for the attempt, waiting for other owners.
    pub(super) async fn admit(
        inner: &Arc<Inner>,
        held: &Held,
        bytes: usize,
        deadline: Option<Instant>,
    ) -> std::result::Result<Self, Refusal> {
        let reservation = Reservation::wait(inner, Pool::Read, held, bytes, deadline).await?;
        Ok(Self::new(reservation))
    }

    /// Total demand observed during this attempt, including refused growth.
    pub(super) fn requested(&self) -> usize {
        self.state.lock().unwrap().requested
    }

    /// The refusal that stopped this attempt, if growth was refused.
    pub(super) fn shortfall(&self) -> Option<Shortfall> {
        self.state.lock().unwrap().refusal
    }

    pub(super) fn scope(&self) -> MemoryScope<'_> {
        MemoryScope {
            bank: self,
            used: 0,
        }
    }

    /// Claim bytes that outlive this attempt, such as a retained Arrow batch.
    pub(super) fn reserve(&self, bytes: usize) -> Result<RetainedMemory> {
        let mut state = self.state.lock().unwrap();
        state.ensure(bytes)?;
        state.retained += bytes;
        Ok(state.reservation.split(bytes).retain())
    }
}

impl MemoryScope<'_> {
    pub(super) fn claim(&mut self, bytes: usize) -> Result<()> {
        let mut state = self.bank.state.lock().unwrap();
        state.ensure(bytes)?;
        state.used += bytes;
        self.used += bytes;
        Ok(())
    }
}

impl Drop for MemoryScope<'_> {
    fn drop(&mut self) {
        self.bank.state.lock().unwrap().used -= self.used;
    }
}

impl BankState {
    // Grow The Attempt's Reservation So `bytes` More Fit Beside Current Claims.
    fn ensure(&mut self, bytes: usize) -> Result<()> {
        let required = self
            .used
            .checked_add(bytes)
            .ok_or(Error::ResourceExhausted {
                resource: "storage read memory",
            })?;
        self.requested = self.requested.max(required.saturating_add(self.retained));
        let available = self.reservation.bytes();
        if required <= available {
            return Ok(());
        }
        match self.reservation.try_grow(required - available) {
            Ok(()) => Ok(()),
            Err(refusal) => {
                self.refusal = Some(match refusal {
                    Refusal::Busy => Shortfall::Busy,
                    Refusal::Exceeded(_) => Shortfall::Exceeded,
                    Refusal::Elapsed | Refusal::Failed(_) => return Err(refusal.into_error()),
                });
                Err(Refusal::Busy.into_error())
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
