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
impl<C> StorageManager<C> {
    pub(crate) fn position(&self, sequence: u64) -> StorageCursor {
        StorageCursor {
            history: self.inner.primary.history().clone(),
            sequence,
        }
    }

    pub(super) fn recovery_position(&self, sequence: u64) -> RecoveryPosition {
        RecoveryPosition {
            history: self.inner.recovery.history().clone(),
            sequence,
        }
    }

    pub(crate) fn charge<T: Serialize + ?Sized>(&self, value: &T, entries: usize) -> Result<usize> {
        encoded_size(value)?
            .checked_add(encoded_size(self.inner.primary.history())?.saturating_mul(entries))
            .and_then(|bytes| bytes.checked_add(entries.checked_mul(ENTRY_OVERHEAD)?))
            .ok_or(Error::InvalidInput {
                field: "storage admission",
                reason: "size overflow",
            })
    }

    pub(super) async fn reserve_scratch(&self, bytes: usize) -> Result<Reservation> {
        Reservation::wait(&self.inner, Pool::Write, &Held::default(), bytes, None)
            .await
            .map_err(Refusal::into_error)
    }

    /// The schema registered under `key` in the current saved image.

}
