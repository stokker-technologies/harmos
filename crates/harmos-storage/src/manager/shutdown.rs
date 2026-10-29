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
    #[tracing::instrument(name = "storage.manager.shutdown.close", level = "debug", skip_all, fields(outcome = "cancelled"))]
    pub async fn close(&self) -> Result<()> {
        let result = async {
            {
                let mut state = self.inner.state.lock().unwrap();
                state.status.closing = true;
                state.close_followers();
            }
            self.inner.notify();

            let mut lifecycle =
                tracing::Instrument::instrument(self.inner.lifecycle.write(), tracing::trace_span!("storage.lock.wait")).await;
            let ready = {
                let state = self.inner.state.lock().unwrap();
                state.ready && !state.status.closed
            };
            if ready {
                self.settle_transition().await;
                self.settle_files().await?;
                self.settle_deletions().await?;
                let files = self.capture_files(None)?;
                self.flush_domain(Domain::Data, None).await?;
                self.persist_files(&files).await?;
                self.retire_files();
                self.flush_recovery().await?;
            }
            if ready {
                self.release_snapshots().await?;
            }
            // Closing prevents new maintenance; let an in-flight backend call settle
            // before releasing its physical owner, including uncertain failed calls.
            let mut changed = self.inner.changed.subscribe();
            loop {
                {
                    let state = self.inner.state.lock().unwrap();
                    if !state.pruning_objects {
                        break;
                    }
                    if !state.status.running {
                        return Err(Error::NotReady { domain: "storage worker" });
                    }
                }
                changed.changed().await.map_err(|_| Error::NotReady { domain: "storage" })?;
            }
            if lifecycle.primary_touched && !lifecycle.primary_closed {
                tracing::Instrument::instrument(
                    self.inner.primary.close(),
                    tracing::debug_span!("storage.backend", method = "primary.close"),
                )
                .await?;
                lifecycle.primary_closed = true;
            }
            if lifecycle.recovery_touched && !lifecycle.recovery_closed {
                tracing::Instrument::instrument(
                    self.inner.recovery.close(),
                    tracing::debug_span!("storage.backend", method = "recovery.close"),
                )
                .await?;
                lifecycle.recovery_closed = true;
            }
            // Release Recovered Files Even When Startup Failed Before Readiness.
            self.retire_files();
            {
                let mut state = self.inner.state.lock().unwrap();
                state.receipts.clear();
                state.status.closed = true;
                state.collect();
            }
            self.inner.notify();
            if let Some(owner) = &lifecycle.owner {
                owner.stop().await?;
            }
            lifecycle.owner = None;
            // A supervisor stopped before its first poll never enters the worker's
            // running guard. The completed join is authoritative in either case.
            self.inner.state.lock().unwrap().status.running = false;
            self.inner.notify();
            Ok(())
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub(super) fn check_raw_recovery(&self) -> Result<()> {
        if self.inner.state.lock().unwrap().journal_saved.is_some() {
            return Err(Error::InvalidInput {
                field: "recovery ownership",
                reason: "managed journal owns recovery writes",
            });
        }
        Ok(())
    }
}
