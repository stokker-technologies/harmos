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
    /// Waits passively for a strictly newer durable primary frontier.
    ///
    /// Intermediate frontiers may coalesce. This does not request persistence or
    /// retry failed work. Cancellation only removes the notification waiter.
    /// A newer saved cursor is returned before checking terminal storage state.
    #[tracing::instrument(name = "storage.manager.status.wait_saved_after", level = "debug", skip_all, fields(after = cursor.sequence, saved = tracing::field::Empty, outcome = "cancelled"))]
    pub async fn wait_saved_after(&self, cursor: &StorageCursor) -> Result<StorageCursor> {
        let result = async {
            self.inner.primary.history().check(&cursor.history)?;
            self.inner
                .wait(None, |state| {
                    if state.status.saved.sequence > cursor.sequence {
                        return crate::capacity::Attempt::Ready(state.status.saved.clone());
                    }
                    let error = if state.status.closing || state.status.closed {
                        Some(Error::Closing { domain: "storage" })
                    } else if let Some(source) = &state.status.failure {
                        Some(Error::Persistence { source: Arc::clone(source) })
                    } else if !state.ready || !state.status.running {
                        Some(Error::NotReady { domain: "storage" })
                    } else {
                        None
                    };
                    match error {
                        Some(error) => crate::capacity::Attempt::Refuse(Refusal::Failed(error)),
                        None => crate::capacity::Attempt::Wait,
                    }
                })
                .await
                .map_err(Refusal::into_error)
        }
        .await;
        if let Ok(saved) = &result {
            tracing::Span::current().record("saved", saved.sequence);
        }
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    pub fn check_state_dependency(&self, required: &StorageCursor) -> Result<()> {
        if !self.cursor_saved().covers(required)? {
            return Err(Error::InvalidInput {
                field: "state evidence dependency",
                reason: "required primary evidence is not durable",
            });
        }
        Ok(())
    }

    #[tracing::instrument(name = "storage.manager.status.retry_transient_persistence", level = "trace", skip_all)]
    pub fn retry_transient_persistence(&self) -> bool {
        let mut state = self.inner.state.lock().unwrap();
        let Some(failure) = &state.status.failure else {
            return false;
        };
        let mut cause = failure.as_ref();
        while let Error::Persistence { source } = cause {
            cause = source.as_ref();
        }
        if !matches!(
            cause,
            Error::Backend {
                kind: crate::BackendFailure::Transient,
                ..
            }
        ) {
            return false;
        }
        state.status.failure = None;
        state.retry = state.retry.wrapping_add(1);
        drop(state);
        self.inner.notify();
        true
    }

    pub fn write_receipt_capacity(&self) -> usize {
        self.inner.settings.request.max_pending_write_batches.get()
    }

    #[tracing::instrument(name = "storage.manager.status.await_durable", level = "debug", skip_all)]
    pub async fn await_durable(&self, cursor: &StorageCursor) -> Result<()> {
        self.inner.primary.history().check(&cursor.history)?;
        self.flush_domain(Domain::Data, Some(cursor)).await
    }

    #[tracing::instrument(name = "storage.manager.status.release_object_history", level = "debug", skip_all)]
    pub async fn release_object_history(&self, schema: &SchemaKey, item: &crate::ObjectKey, through: &StorageCursor) -> Result<()> {
        self.inner.primary.history().check(&through.history)?;
        if !self.status().saved.covers(through)? {
            return Err(Error::InvalidInput {
                field: "object retention",
                reason: "release exceeds durable frontier",
            });
        }
        self.inner.bounded("primary.release_object_history", self.inner.primary.release_object_history(schema, item, through)).await?;
        self.inner.state.lock().unwrap().objects_pruned = 0;
        self.inner.notify();
        Ok(())
    }
}

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
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::{Data, Recovery, history, storage_settings};

    fn storage() -> StorageManager {
        StorageManager::new(
            crate::StorageBackends {
                primary: Arc::new(Data::<()>::new(history())),
                state: Arc::new(Recovery::new(history())),
            },
            storage_settings(),
        )
        .unwrap()
    }

    async fn pending<F: std::future::Future + ?Sized>(mut future: std::pin::Pin<&mut F>) -> bool {
        std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx).is_pending())).await
    }

    #[tokio::test]
    async fn saved_wait_cancels() {
        let storage = storage();
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.ready = true;
            state.status.running = true;
            state.retry = 41;
        }
        let before = storage.cursor_saved();
        let future = StorageCursor { sequence: 42, ..before.clone() };
        let revision = *storage.inner.changed.borrow();
        let subscribers = storage.inner.changed.receiver_count();
        let mut waiting = Box::pin(storage.wait_saved_after(&future));
        assert!(pending(waiting.as_mut()).await);
        assert_eq!(storage.inner.changed.receiver_count(), subscribers + 1);
        drop(waiting);
        assert_eq!(storage.inner.changed.receiver_count(), subscribers);
        assert_eq!(*storage.inner.changed.borrow(), revision);
        {
            let state = storage.inner.state.lock().unwrap();
            assert_eq!(state.retry, 41);
            assert!(state.data_request.is_none());
            assert!(state.recovery_request.is_none());
            assert_eq!(state.queue.len(), 0);
            assert_eq!(state.status.saved, before);
            assert_eq!(state.status.accepted, before);
        }
        let mut waiting = Box::pin(storage.wait_saved_after(&future));
        assert!(pending(waiting.as_mut()).await);
        storage.inner.state.lock().unwrap().status.closed = true;
        storage.inner.notify();
        assert!(matches!(waiting.await, Err(Error::Closing { .. })));
    }

    #[tokio::test]
    async fn saved_wait_errors() {
        let storage = storage();
        let cursor = storage.cursor_saved();
        assert!(matches!(storage.wait_saved_after(&cursor).await, Err(Error::NotReady { .. })));
        let failure = Arc::new(Error::NotReady { domain: "injected persistence" });
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.status.failure = Some(Arc::clone(&failure));
            state.retry = 41;
        }
        let result = storage.wait_saved_after(&cursor).await;
        assert!(matches!(result, Err(Error::Persistence { source }) if Arc::ptr_eq(&source, &failure)));
        {
            let mut state = storage.inner.state.lock().unwrap();
            assert!(Arc::ptr_eq(state.status.failure.as_ref().unwrap(), &failure));
            assert_eq!(state.retry, 41);
            assert!(state.data_request.is_none());
            state.status.closing = true;
        }
        assert!(matches!(storage.wait_saved_after(&cursor).await, Err(Error::Closing { .. })));
        {
            let mut state = storage.inner.state.lock().unwrap();
            state.status.accepted.sequence = 7;
            state.status.saved.sequence = 7;
        }
        assert_eq!(storage.wait_saved_after(&cursor).await.unwrap().sequence, 7);
        let mut foreign = cursor;
        foreign.history.store = crate::Identifier::new("foreign").unwrap();
        assert!(matches!(storage.wait_saved_after(&foreign).await, Err(Error::Domain(harmos_domain::Error::DifferentHistory))));
    }
}
