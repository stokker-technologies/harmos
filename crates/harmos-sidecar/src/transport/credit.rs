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
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// Local Imports
use super::{Packet, Role, invalid};
use crate::Frame;

/* === Constants === */

pub(super) const WINDOW: usize = 2 * (Frame::LIMIT + 4);
pub(super) const MINIMUM: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Credit {
    permits: Arc<Semaphore>,
    ledger: Mutex<Ledger>,
}

#[derive(Default)]
struct Ledger {
    issued: u64,
    returned: u64,
}

#[must_use = "retain the reservation until writing or abandoning its packet"]
pub struct Reservation {
    credit: Arc<Credit>,
    permit: OwnedSemaphorePermit,
    charge: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Credit {
    pub fn new() -> Arc<Self> {
        Self::with_window(WINDOW)
    }

    fn with_window(window: usize) -> Arc<Self> {
        Arc::new(Self { permits: Arc::new(Semaphore::new(window)), ledger: Mutex::new(Ledger::default()) })
    }

    #[tracing::instrument(target = "harmos::carrier", name = "sidecar.credit.reserve", level = "trace", skip_all)]
    pub async fn reserve(self: &Arc<Self>, packet: &Packet) -> io::Result<Reservation> {
        let charge = Self::charge(packet)?;
        let permit = self
            .permits
            .clone()
            .acquire_many_owned(charge as u32)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "sidecar credit closed"))?;
        Ok(Reservation { credit: self.clone(), permit, charge })
    }

    pub fn charge(packet: &Packet) -> io::Result<usize> {
        if packet.channel() != Role::Data {
            return Err(invalid("reserved control packets do not consume data credit"));
        }
        Ok(packet.encoded_len()?.max(MINIMUM))
    }

    pub fn returned(&self, cumulative: u64) -> io::Result<()> {
        let mut ledger = self.ledger.lock().map_err(|_| invalid("sidecar credit ledger poisoned"))?;
        if cumulative <= ledger.returned || cumulative > ledger.issued {
            return Err(invalid("invalid cumulative sidecar credit"));
        }
        let amount = usize::try_from(cumulative - ledger.returned).map_err(|_| invalid("sidecar credit exceeds addressable capacity"))?;
        ledger.returned = cumulative;
        self.permits.add_permits(amount);
        Ok(())
    }

    pub fn close(&self) {
        let _ledger = self.ledger.lock();
        self.permits.close();
    }
}

impl Reservation {
    pub fn issued(self) -> io::Result<()> {
        let mut ledger = self.credit.ledger.lock().map_err(|_| invalid("sidecar credit ledger poisoned"))?;
        if self.credit.permits.is_closed() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "sidecar credit closed"));
        }
        let issued = ledger.issued.checked_add(self.charge as u64).ok_or_else(|| invalid("sidecar cumulative credit overflow"))?;
        ledger.issued = issued;
        self.permit.forget();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
