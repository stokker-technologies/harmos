/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Read Memory Admission

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
use super::arrow::RetainedMemory;
use super::manager::MemoryReservation;
use crate::{Error, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct MemoryBank {
    state: Arc<Mutex<BankState>>,
}
struct BankState {
    reservation: Option<MemoryReservation>,
    capacity: usize,
    used: usize,
    closed: bool,
}
struct Claim {
    bank: Arc<Mutex<BankState>>,
    bytes: AtomicUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemoryBank {
    pub(super) fn new(reservation: MemoryReservation, capacity: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(BankState {
                reservation: Some(reservation),
                capacity,
                used: 0,
                closed: false,
            })),
        }
    }

    pub(super) fn reserve(&self, bytes: usize) -> Result<RetainedMemory> {
        {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return Err(Error::NotReady {
                    domain: "closed query allocation owner",
                });
            }
            let required = state
                .used
                .checked_add(bytes)
                .ok_or(Error::ResourceExhausted {
                    resource: "storage read memory",
                })?;
            if required > state.capacity {
                let additional = required - state.capacity;
                state
                    .reservation
                    .as_mut()
                    .expect("open bank owns reservation")
                    .grow_read(additional)?;
                state.capacity = required;
            }
            state.used = required;
        }
        let claim = Arc::new(Claim {
            bank: self.state.clone(),
            bytes: AtomicUsize::new(bytes),
        });
        let shrink = claim.clone();
        Ok(RetainedMemory::new(claim, move |bytes| {
            shrink.shrink_to(bytes)
        }))
    }
}

impl Claim {
    fn shrink_to(&self, bytes: usize) -> Result<()> {
        let mut state = self.bank.lock().unwrap();
        let previous = self.bytes.load(Ordering::Relaxed);
        if bytes > previous {
            return Err(Error::InvalidInput {
                field: "query kernel memory",
                reason: "retained claim cannot grow after allocation",
            });
        }
        self.bytes.store(bytes, Ordering::Relaxed);
        state.used -= previous - bytes;
        state.release_unused();
        Ok(())
    }
}

impl BankState {
    fn release_unused(&mut self) {
        if !self.closed {
            return;
        }
        if self.used == 0 {
            self.reservation.take();
            return;
        }
        // The Original Queue/Owner Metadata Charge Covers This Small Bank.
        if let Some(reservation) = &mut self.reservation {
            reservation.retain_read(self.used.saturating_add(super::manager::ENTRY_OVERHEAD));
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut state = self.bank.lock().unwrap();
        state.used -= self.bytes.load(Ordering::Relaxed);
        state.release_unused();
    }
}

impl Drop for MemoryBank {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        state.release_unused();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
