/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.recovery.write_recovery", level = "debug", skip_all)]
    pub async fn write_recovery(&self, payloads: Vec<Vec<u8>>) -> Result<RecoveryPosition> {
        self.check_raw_recovery()?;
        if payloads.is_empty() {
            return Ok(self.status().recovery_accepted);
        }
        let required_data = self.cursor_saved();
        let bytes = payloads.iter().try_fold(0usize, |bytes, payload| {
            bytes.checked_add(recovery_encoded_bound(payload.len(), self.inner.recovery.history(), self.inner.primary.history())?).ok_or(
                Error::InvalidInput {
                    field: "storage admission",
                    reason: "size overflow",
                },
            )
        })?;
        let sequence = self
            .admit(Domain::Recovery, bytes, payloads.len(), |first| {
                Ok(Operation::Recovery(crate::StorageBatch::new(
                    payloads
                        .into_iter()
                        .enumerate()
                        .map(|(index, payload)| RecoveryEntry {
                            required_data: required_data.clone(),
                            position: self.recovery_position(first + index as u64),
                            payload,
                        })
                        .collect(),
                )))
            })
            .await?;
        Ok(self.recovery_position(sequence))
    }

    #[tracing::instrument(name = "storage.manager.recovery.write_snapshot", level = "debug", skip_all)]
    pub async fn write_snapshot(&self, snapshot: RecoverySnapshot) -> Result<()> {
        self.check_raw_recovery()?;
        self.inner.recovery.history().check(&snapshot.through.history)?;
        self.check_state_dependency(&snapshot.required_data)?;
        let bytes = recovery_encoded_bound(snapshot.payload.len(), &snapshot.through.history, &snapshot.required_data.history)?;
        self.admit(Domain::Recovery, bytes, 0, |_| Ok(Operation::Snapshot(StorageBatch::new(vec![snapshot])))).await?;
        Ok(())
    }

    #[tracing::instrument(name = "storage.manager.recovery.flush", level = "debug", skip_all)]
    pub async fn flush(&self) -> Result<StorageCursor> {
        let through = self.cursor_accepted();
        self.flush_until(&through).await
    }

    #[tracing::instrument(name = "storage.manager.recovery.flush_until", level = "debug", skip_all)]
    pub async fn flush_until(&self, through: &StorageCursor) -> Result<StorageCursor> {
        let files = self.capture_files(Some(through))?;
        self.flush_prepared(through, &files).await
    }

    #[tracing::instrument(name = "storage.manager.recovery.flush_prepared", level = "debug", skip_all)]
    pub async fn flush_prepared(&self, through: &StorageCursor, files: &Option<crate::FileSave>) -> Result<StorageCursor> {
        self.flush_domain(Domain::Data, Some(through)).await?;
        if files.is_some() {
            let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.write(), tracing::trace_span!("storage.lock.wait")).await;
            if self.inner.state.lock().unwrap().status.closed {
                return Err(Error::Closing { domain: "storage" });
            }
            self.persist_files(files).await?;
        }
        Ok(self.cursor_saved())
    }

    #[tracing::instrument(name = "storage.manager.recovery.flush_recovery", level = "debug", skip_all)]
    pub async fn flush_recovery(&self) -> Result<RecoveryPosition> {
        self.flush_domain(Domain::Recovery, None).await?;
        Ok(self.status().recovery_saved)
    }

    #[tracing::instrument(name = "storage.manager.recovery.read_recovery", level = "debug", skip_all)]
    pub async fn read_recovery(
        &self, after: &RecoveryPosition, through: &RecoveryPosition, limits: ReadLimits,
    ) -> Result<crate::ReadBatch<RecoveryPage>> {
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        let status = self.status();
        if !status.recovery_saved.covers(through)? {
            return Err(Error::InvalidInput {
                field: "recovery range",
                reason: "frontier is not durable",
            });
        }
        let page = self.inner.bounded("recovery.read_entries", self.inner.recovery.read_entries(after, through, limits)).await?;
        page.check(after, through, limits, |entry| encoded_size(entry).unwrap_or(usize::MAX))?;
        for entry in &page.entries {
            self.check_state_dependency(&entry.required_data)?;
        }
        Ok(crate::ReadBatch::new(page))
    }

    #[tracing::instrument(name = "storage.manager.recovery.read_snapshot", level = "debug", skip_all)]
    pub async fn read_snapshot(&self, max_bytes: std::num::NonZeroUsize) -> Result<Option<crate::ReadBatch<RecoverySnapshot>>> {
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        let snapshot = self.inner.bounded("recovery.read_snapshot", self.inner.recovery.read_snapshot(max_bytes)).await?;
        if let Some(snapshot) = &snapshot
            && (!self.status().recovery_saved.covers(&snapshot.through)?
                || !self.status().saved.covers(&snapshot.required_data)?
                || encoded_size(snapshot)? > max_bytes.get())
        {
            return Err(Error::InvalidInput {
                field: "recovery snapshot",
                reason: "invalid saved boundary or byte size",
            });
        }
        Ok(snapshot.map(crate::ReadBatch::new))
    }
}
