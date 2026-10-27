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
use std::io;
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::credit::{MINIMUM, WINDOW};
use super::invalid;
use crate::Frame;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ReceiveCredit {
    ledger: Mutex<Ledger>,
    released: watch::Sender<u64>,
    closed: watch::Sender<bool>,
}

#[derive(Default)]
struct Ledger {
    received: u64,
    consumed: u64,
    closed: bool,
}

#[must_use = "retain until consumption, or drop to fail the connection"]
pub struct Consumption {
    credit: Arc<ReceiveCredit>,
    charge: u64,
    acknowledged: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReceiveCredit {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            ledger: Mutex::new(Ledger::default()),
            released: watch::channel(0).0,
            closed: watch::channel(false).0,
        })
    }

    #[tracing::instrument(target = "harmos::carrier", name = "sidecar.credit.admit", level = "trace", skip_all, fields(wire_bytes = encoded))]
    pub fn admit(self: &Arc<Self>, encoded: usize) -> io::Result<Consumption> {
        if !(5..=Frame::LIMIT + 4).contains(&encoded) {
            return Err(invalid("invalid credited packet length"));
        }
        let charge = encoded.max(MINIMUM) as u64;
        let mut ledger = self.ledger.lock().map_err(|_| invalid("receive credit ledger poisoned"))?;
        if ledger.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "receive credit closed"));
        }
        let received = ledger.received.checked_add(charge).ok_or_else(|| invalid("receive credit counter overflow"))?;
        if received - ledger.consumed > WINDOW as u64 {
            return Err(invalid("peer exceeded sidecar byte credit"));
        }
        ledger.received = received;
        Ok(Consumption {
            credit: self.clone(),
            charge,
            acknowledged: false,
        })
    }

    pub fn released(&self) -> watch::Receiver<u64> {
        self.released.subscribe()
    }

    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }

    pub fn close(&self) {
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.closed = true;
        }
        self.closed.send_replace(true);
    }
}

impl Consumption {
    pub fn consumed(mut self) -> io::Result<()> {
        let mut ledger = self.credit.ledger.lock().map_err(|_| invalid("receive credit ledger poisoned"))?;
        if ledger.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "receive credit closed"));
        }
        let consumed = ledger.consumed.checked_add(self.charge).ok_or_else(|| invalid("consumption counter overflow"))?;
        if consumed > ledger.received {
            return Err(invalid("consumption exceeds received credit"));
        }
        ledger.consumed = consumed;
        self.acknowledged = true;
        self.credit.released.send_replace(consumed);
        Ok(())
    }
}

impl Drop for Consumption {
    fn drop(&mut self) {
        if !self.acknowledged {
            self.credit.close();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
