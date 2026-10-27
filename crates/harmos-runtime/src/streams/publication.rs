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
use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// Local Imports
use super::{PublicationRow, StreamPublicationError};

/* === Constants === */

const RETAINED_BYTES: usize = 64 * 1024 * 1024;
const RETAINED_ROWS: usize = 65_536;

// Includes requested row/receipt headers, queue cell and Arc counters.
const OWNER_BYTES: usize = 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ManagedPublication(Arc<Owned>);

#[derive(Debug)]
struct Owned {
    stream: Arc<str>,
    owner: Arc<str>,
    schema: u32,
    generation: Option<u64>,
    wire_sequence: u64,
    logical_sequence: OnceLock<u64>,
    missed: u64,
    row: PublicationRow,
    _bytes: OwnedSemaphorePermit,
    _slot: OwnedSemaphorePermit,
}

#[derive(Debug)]
pub(super) struct Pool {
    bytes: Arc<Semaphore>,
    rows: Arc<Semaphore>,
    admission: Mutex<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Pool {
    pub(super) fn new() -> Self {
        Self {
            bytes: Arc::new(Semaphore::new(RETAINED_BYTES)),
            rows: Arc::new(Semaphore::new(RETAINED_ROWS)),
            admission: Mutex::new(()),
        }
    }

    pub(super) fn retain(
        &self, definition: &crate::streams::Definition, generation: Option<u64>, row: PublicationRow, wire_sequence: u64, missed: u64,
        window: Option<(&super::window::Window, u64)>,
    ) -> Result<ManagedPublication, StreamPublicationError> {
        let bytes = row
            .payload
            .capacity()
            .checked_add(row.context.as_ref().map_or(0, String::capacity))
            .and_then(|bytes| bytes.checked_add(OWNER_BYTES))
            .filter(|bytes| *bytes <= RETAINED_BYTES)
            .ok_or(StreamPublicationError::Capacity)?;
        let _admission = self.admission.lock().expect("publication budget poisoned");
        let missing_bytes = bytes.saturating_sub(self.bytes.available_permits());
        let missing_rows = usize::from(self.rows.available_permits() == 0);
        if missing_bytes > 0 || missing_rows > 0 {
            let (window, taken) = window.ok_or(StreamPublicationError::Capacity)?;
            window.reclaim(missing_bytes, missing_rows, taken).ok_or(StreamPublicationError::Capacity)?;
        }
        let slot = self.rows.clone().try_acquire_owned().map_err(|_| StreamPublicationError::Capacity)?;
        let bytes = self.bytes.clone().try_acquire_many_owned(bytes as u32).map_err(|_| StreamPublicationError::Capacity)?;
        Ok(ManagedPublication(Arc::new(Owned {
            stream: Arc::<str>::from(definition.id()),
            owner: Arc::from(definition.owner().expect("plugin publication owner")),
            schema: definition.version(),
            generation,
            wire_sequence,
            logical_sequence: OnceLock::new(),
            missed,
            row,
            _bytes: bytes,
            _slot: slot,
        })))
    }
}

impl ManagedPublication {
    pub fn stream(&self) -> &str {
        &self.0.stream
    }

    pub fn owner(&self) -> &str {
        &self.0.owner
    }

    pub fn schema(&self) -> u32 {
        self.0.schema
    }

    pub fn generation(&self) -> Option<u64> {
        self.0.generation
    }

    pub fn sequence(&self) -> u64 {
        *self.0.logical_sequence.get().expect("committed publication")
    }

    pub(super) fn assign_sequence(&self, sequence: u64) {
        self.0.logical_sequence.set(sequence).expect("one original publication append");
    }

    pub(super) fn reclaimable_bytes(&self) -> Option<usize> {
        (Arc::strong_count(&self.0) == 1).then(|| self.0._bytes.num_permits())
    }

    pub fn wire_sequence(&self) -> u64 {
        self.0.wire_sequence
    }

    pub fn missed(&self) -> u64 {
        self.0.missed
    }

    pub fn payload(&self) -> &[u8] {
        &self.0.row.payload
    }

    pub fn published_unix_micros(&self) -> Option<i64> {
        self.0.row.published_unix_micros
    }

    pub fn context(&self) -> Option<&str> {
        self.0.row.context.as_deref()
    }

    pub(super) fn row(&self) -> &PublicationRow {
        &self.0.row
    }
}

/* === Public Functions === */

pub(crate) fn row_value<S: 'static>(row: &dyn Any) -> Option<&S> {
    row.downcast_ref::<S>().or_else(|| {
        let managed = row.downcast_ref::<ManagedPublication>()?;
        (managed.row() as &dyn Any).downcast_ref::<S>()
    })
}

/* === Private Functions === */

/* === Tests === */
