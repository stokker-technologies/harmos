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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl RecordWriteBytes for RecordWrite<'_> {
    fn encoded_bytes(&self, history: &crate::StoreHistory) -> Result<usize> {
        let value = StoredRecord {
            position: StorageCursor { history: history.clone(), sequence: u64::MAX },
            schema: self.schema.clone(),
            scope: self.scope.cloned(),
            item: self.item.clone(),
            source: self.source.cloned(),
            source_time: self.source_time,
            payload: self.payload,
        };
        encoded_size(&value)
    }
}

impl RecordWriteBytes for RecordInput {
    fn encoded_bytes(&self, history: &crate::StoreHistory) -> Result<usize> {
        self.borrowed().encoded_bytes(history)
    }
}

impl Drop for WriteWaiter {
    fn drop(&mut self) {
        let counter = if self.measurement { &self.inner.measurement_waiting } else { &self.inner.control_waiting };
        counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.notify();
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        if self.active {
            {
                let mut state = self.inner.state.lock().unwrap();
                state.reserved = 0;
                state.reserved_batches = 0;
                state.collect();
            }
            self.inner.notify();
        }
    }
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_add(bytes.len()).ok_or_else(|| io::Error::other("encoded size overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

pub fn encoded_size<T: Serialize + ?Sized>(value: &T) -> Result<usize> {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value)
        .map_err(|_| Error::InvalidInput { field: "storage payload", reason: "cannot measure encoded size" })?;
    Ok(counter.0)
}

pub(crate) fn recovery_encoded_bound(payload: usize, history: &crate::StoreHistory, data_history: &crate::StoreHistory) -> Result<usize> {
    let entry = RecoveryEntry {
        required_data: StorageCursor { history: data_history.clone(), sequence: u64::MAX },
        position: RecoveryPosition { history: history.clone(), sequence: u64::MAX },
        payload: Vec::new(),
    };
    let encoded = payload
        .checked_div(3)
        .and_then(|groups| groups.checked_mul(4))
        .and_then(|bytes| {
            bytes.checked_add(match payload % 3 {
                0 => 0,
                1 => 2,
                _ => 3,
            })
        })
        .ok_or(Error::InvalidInput { field: "managed recovery", reason: "encoded size overflow" })?;
    let snapshot = RecoverySnapshot { required_data: entry.required_data.clone(), through: entry.position.clone(), payload: Vec::new() };
    let header = encoded_size(&entry)?.max(encoded_size(&snapshot)?);
    header.checked_add(encoded).ok_or(Error::InvalidInput { field: "managed recovery", reason: "encoded size overflow" })
}

/* === Private Functions === */

/* === Tests === */
