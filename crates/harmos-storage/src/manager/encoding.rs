/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */

impl RecordWriteBytes for RecordWrite<'_> {
    fn resident_bytes(&self, history: &crate::StoreHistory) -> Result<usize> {
        [
            self.payload.len(),
            self.schema.id.as_str().len(),
            self.scope.map_or(0, |value| value.as_str().len()),
            self.item.as_str().len(),
            self.source.map_or(0, |value| value.as_str().len()),
            history.store.as_str().len(),
            history.epoch.as_str().len(),
            std::mem::size_of::<StoredRecord>(),
            ENTRY_OVERHEAD,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, next| bytes.checked_add(next))
        .ok_or(Error::InvalidInput { field: "storage admission", reason: "size overflow" })
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

pub(crate) fn recovery_preparation_bytes(
    payload: usize, history: &crate::StoreHistory, data_history: &crate::StoreHistory,
) -> Result<usize> {
    let capacity = payload
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or(Error::InvalidInput { field: "managed recovery", reason: "size overflow" })?;
    recovery_resident_bytes(capacity, history, data_history)
}

pub(crate) fn recovery_resident_bytes(payload: usize, history: &crate::StoreHistory, data_history: &crate::StoreHistory) -> Result<usize> {
    [
        payload,
        history.store.as_str().len(),
        history.epoch.as_str().len(),
        data_history.store.as_str().len(),
        data_history.epoch.as_str().len(),
        std::mem::size_of::<RecoveryEntry>(),
        ENTRY_OVERHEAD,
    ]
    .into_iter()
    .try_fold(0usize, |bytes, next| bytes.checked_add(next))
    .ok_or(Error::InvalidInput { field: "managed recovery", reason: "size overflow" })
}

/* === Private Functions === */

/* === Tests === */
