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

impl<C> StorageManager<C> {
    pub fn check_state_dependency(&self, required: &StorageCursor) -> Result<()> {
        if !self.cursor_saved().covers(required)? {
            return Err(Error::InvalidInput { field: "state evidence dependency", reason: "required primary evidence is not durable" });
        }
        Ok(())
    }

    pub fn retry_transient_persistence(&self) -> bool {
        let mut state = self.inner.state.lock().unwrap();
        let Some(failure) = &state.status.failure else {
            return false;
        };
        let mut cause = failure.as_ref();
        while let Error::Persistence { source } = cause {
            cause = source.as_ref();
        }
        if !matches!(cause, Error::Backend { kind: crate::BackendFailure::Transient, .. }) {
            return false;
        }
        state.status.failure = None;
        state.retry = state.retry.wrapping_add(1);
        drop(state);
        self.inner.notify();
        true
    }

    pub fn reserve_arrow_memory(&self, bytes: usize) -> Result<RetainedMemory> {
        Reservation::try_new(&self.inner, Pool::Read, &Held::default(), bytes).map(Reservation::retain).map_err(Refusal::into_error)
    }

    pub fn write_receipt_capacity(&self) -> usize {
        self.inner.settings.request.max_pending_write_batches.get()
    }

    pub async fn await_durable(&self, cursor: &StorageCursor) -> Result<()> {
        self.inner.primary.history().check(&cursor.history)?;
        self.flush_domain(Domain::Data, Some(cursor)).await
    }

    pub async fn release_object_history(&self, schema: &SchemaKey, item: &crate::ObjectKey, through: &StorageCursor) -> Result<()> {
        self.inner.primary.history().check(&through.history)?;
        if !self.status().saved.covers(through)? {
            return Err(Error::InvalidInput { field: "object retention", reason: "release exceeds durable frontier" });
        }
        self.inner.bounded(self.inner.primary.release_object_history(schema, item, through)).await?;
        self.inner.state.lock().unwrap().objects_pruned = 0;
        self.inner.notify();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
