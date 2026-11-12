/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Pinned Historical Images for Admitted Reads

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use super::backend::{BackendSnapshot, ScanPage};
use super::capacity::{Attempt, Refusal};
use super::config::SNAPSHOT_BYTES;
use super::manager::{Inner, State, encoded_size};
use super::selection::Entry;
use super::{FileReference, ObjectEntry, ObjectPoint, StorageCursor, StorageManager};
use crate::{Error, Result};

/* === Constants === */

static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Snapshot {
    inner: Arc<Inner>,
    id: u64,
    backend: BackendSnapshot,
}

pub(super) struct SnapshotState {
    pub(super) context: crate::execution::Context,
    pub(super) queued: Option<tracing::Span>,
    at: StorageCursor,
    opened: Option<std::result::Result<BackendSnapshot, Arc<Error>>>,
    opening: bool,
    released: bool,
}

/* === Enums === */

pub(super) enum SnapshotAction {
    Open(u64, StorageCursor),
    Release(u64, Option<BackendSnapshot>),
}

/* === Traits === */

/* === Implementations === */

impl Drop for SnapshotState {
    fn drop(&mut self) {
        self.context.sync(|| drop(self.queued.take()));
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.snapshot.snapshot", level = "trace", skip_all, fields(outcome = "cancelled"))]
    pub async fn snapshot(&self, at: &StorageCursor) -> Result<Snapshot> {
        let result = async {
            self.inner.primary.get_history().check(&at.history)?;
            let id = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
            {
                let mut state = self.inner.state.lock().unwrap();
                super::capacity::admitting(&state, false)?;
                if !state.status.saved.covers(at)? {
                    return Err(Error::InvalidInput {
                        field: "query cursor",
                        reason: "cursor exceeds the durable storage frontier",
                    });
                }
                state.snapshots.insert(
                    id,
                    SnapshotState {
                        context: crate::execution::Context::current(),
                        queued: Some(tracing::debug_span!("storage.snapshot.queue", snapshot = id)),
                        at: at.clone(),
                        opened: None,
                        opening: false,
                        released: false,
                    },
                );
            }
            // The Guard Exists Before Waiting, So Cancellation Releases Registration.
            let mut guard = Snapshot {
                inner: Arc::clone(&self.inner),
                id,
                backend: BackendSnapshot { at: at.clone(), token: Vec::new() },
            };
            self.inner.notify();
            let opened = self
                .inner
                .wait(None, |state| match state.snapshots.get(&id) {
                    None => Attempt::Refuse(Refusal::Failed(Error::Closing { domain: "storage" })),
                    Some(held) if held.released => Attempt::Refuse(Refusal::Failed(Error::Closing { domain: "storage" })),
                    Some(SnapshotState { opened: Some(result), .. }) => match result {
                        Ok(snapshot) => Attempt::Ready(snapshot.clone()),
                        Err(source) => Attempt::Refuse(Refusal::Failed(Error::Persistence { source: source.clone() })),
                    },
                    Some(_) if !state.status.running => Attempt::Refuse(Refusal::Failed(Error::NotReady { domain: "storage worker" })),
                    Some(_) => Attempt::Wait,
                })
                .await
                .map_err(Refusal::into_error)?;
            guard.backend = opened;
            Ok(guard)
        }
        .await;
        tracing::Span::current().record("outcome", if result.is_ok() { "completed" } else { "failed" });
        result
    }

    #[tracing::instrument(name = "storage.snapshot.release_snapshots", level = "trace", skip_all)]
    pub(super) async fn release_snapshots(&self) -> Result<()> {
        {
            let mut state = self.inner.state.lock().unwrap();
            for snapshot in state.snapshots.values_mut() {
                snapshot.released = true;
            }
        }
        self.inner.notify();
        self.inner
            .wait(None, |state| {
                if state.snapshots.is_empty() {
                    Attempt::Ready(())
                } else if !state.status.running {
                    Attempt::Refuse(Refusal::Failed(Error::NotReady { domain: "storage worker" }))
                } else {
                    Attempt::Wait
                }
            })
            .await
            .map_err(Refusal::into_error)
    }
}

impl Snapshot {
    pub fn at(&self) -> &StorageCursor {
        &self.backend.at
    }

    pub async fn list_column_rows(
        &self, read: &crate::query::columnar::ArrowScan, cancel: &crate::query::columnar::Cancellation,
    ) -> Result<crate::query::columnar::ArrowPage> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let future = crate::query::columnar::list_column_rows(self.inner.primary.as_ref(), &self.backend, read, cancel);
        let page = self.inner.bounded("primary.list_column_rows", Box::pin(future)).await?;
        self.check()?;
        Ok(page)
    }

    pub async fn list_index_entries(
        &self, read: &crate::query::index::IndexRead, cancel: &crate::query::columnar::Cancellation,
    ) -> Result<crate::query::index::IndexPage> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let future = crate::query::index::list_index_entries(self.inner.primary.as_ref(), &self.backend, read, cancel);
        let page = self.inner.bounded("primary.list_index_entries", Box::pin(future)).await?;
        self.check()?;
        Ok(page)
    }

    pub async fn advance_projection(
        &self, name: &crate::Identifier, expected_revision: Option<u64>, work: super::ReadLimits,
        cancel: &crate::query::columnar::Cancellation,
    ) -> Result<u64> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let revision =
            crate::query::projection::advance_projection(self.inner.primary.as_ref(), &self.backend, name, expected_revision, work, cancel)
                .await?;
        self.check()?;
        Ok(revision)
    }

    pub(super) async fn get_record_position(&self, selection: &super::query::RecordSelection, offset: u64) -> Result<Option<u64>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let position = self
            .inner
            .bounded("primary.get_record_position", self.inner.primary.get_record_position(&self.backend, selection, offset))
            .await?;
        self.check()?;
        if position.is_some_and(|position| position > self.backend.at.sequence) {
            return Err(invalid("backend seek is outside the saved cut"));
        }
        Ok(position)
    }

    pub(super) async fn record_views(
        &self, selection: &super::query::RecordSelection, page: ScanPage,
    ) -> Result<Option<Vec<crate::RecordView>>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let views =
            self.inner.bounded("primary.list_record_views", self.inner.primary.list_record_views(&self.backend, selection, page)).await?;
        self.check()?;
        let Some(views) = views else {
            return Ok(None);
        };
        let mut previous = page.after;
        let mut bytes = 0usize;
        for view in &views {
            let record = view.record();
            self.backend.at.history.check(&record.position.history)?;
            if record.position.sequence <= previous || record.position.sequence > self.backend.at.sequence || !record.matches(selection) {
                return Err(invalid("backend returned an entry outside the ordered selection"));
            }
            previous = record.position.sequence;
            bytes = bytes.saturating_add(view.encoded_bytes());
        }
        if views.len() > page.rows.get() || (views.len() > 1 && bytes > page.bytes.get()) {
            return Err(invalid("backend page exceeds its row or byte bound"));
        }
        Ok(Some(views))
    }

    #[tracing::instrument(name = "storage.snapshot.scan", level = "trace", skip_all, fields(snapshot = self.id, after = page.after, rows = page.rows.get(), bytes = page.bytes.get()))]
    pub(super) async fn scan<T: Entry>(&self, selection: &T::Selection, page: ScanPage) -> Result<(Vec<T>, usize)> {
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        self.check()?;
        let entries = self.inner.bounded("primary.scan", T::scan(&*self.inner.primary, &self.backend, selection, page)).await?;
        self.check()?;
        let mut previous = page.after;
        let mut bytes = 0usize;
        for entry in &entries {
            let position = entry.position();
            self.backend.at.history.check(&position.history)?;
            if position.sequence <= previous || position.sequence > self.backend.at.sequence || !entry.matches(selection) {
                return Err(invalid("backend returned an entry outside the ordered selection"));
            }
            previous = position.sequence;
            bytes = bytes.saturating_add(encoded_size(entry)?);
        }
        if entries.len() > page.rows.get() || (entries.len() > 1 && bytes > page.bytes.get()) {
            return Err(invalid("backend page exceeds its row or byte bound"));
        }
        Ok((entries, bytes))
    }

    #[tracing::instrument(name = "storage.snapshot.get_object", level = "trace", skip_all)]
    pub(super) async fn get_object(&self, point: &ObjectPoint) -> Result<Option<ObjectEntry>> {
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        self.check()?;
        let object = self.inner.bounded("primary.get_object", self.inner.primary.get_object(&self.backend, point)).await?;
        self.check()?;
        if let Some(object) = &object {
            object.version.check_position(&object.position)?;
            if object.position.sequence == 0
                || !self.backend.at.covers(&object.position)?
                || object.value.schema.id != point.schema
                || object.value.item != point.item
            {
                return Err(invalid("backend returned a different object revision"));
            }
        }
        Ok(object)
    }

    #[tracing::instrument(name = "storage.snapshot.read_file", level = "trace", skip_all)]
    pub(super) async fn read_file(&self, file: &FileReference, offset: u64, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        let _lifecycle = tracing::Instrument::instrument(self.inner.lifecycle.read(), tracing::trace_span!("storage.lock.wait")).await;
        self.check()?;
        let bytes =
            self.inner.bounded("primary.get_file_bytes", self.inner.primary.get_file_bytes(&self.backend, file, offset, max_bytes)).await?;
        self.check()?;
        Ok(bytes)
    }

    fn check(&self) -> Result<()> {
        let state = self.inner.state.lock().unwrap();
        match state.snapshots.get(&self.id) {
            Some(held) if !held.released && !state.status.closing => Ok(()),
            _ => Err(Error::Closing { domain: "storage" }),
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(held) = state.snapshots.get_mut(&self.id) {
            held.released = true;
            held.context.sync(|| drop(held.queued.take()));
            held.queued = Some(held.context.sync(|| tracing::debug_span!("storage.snapshot.release.queue", snapshot = self.id)));
        }
        drop(state);
        self.inner.notify();
    }
}

impl State {
    pub(super) fn snapshot_opening(&self) -> bool {
        self.snapshots.values().any(|held| !held.released && held.opened.is_none())
    }

    pub(super) fn snapshot_action(&mut self, allow_release: bool) -> Option<SnapshotAction> {
        for (&id, held) in &mut self.snapshots {
            if held.opening {
                continue;
            }
            if held.released {
                if !allow_release {
                    continue;
                }
                let snapshot = match &held.opened {
                    Some(Ok(snapshot)) => Some(snapshot.clone()),
                    _ => None,
                };
                return Some(SnapshotAction::Release(id, snapshot));
            }
            if held.opened.is_none() {
                held.opening = true;
                return Some(SnapshotAction::Open(id, held.at.clone()));
            }
        }
        None
    }

    pub(super) fn snapshot_opened(&mut self, id: u64, result: Result<BackendSnapshot>) {
        let Some(held) = self.snapshots.get_mut(&id) else {
            return;
        };
        held.opening = false;
        held.opened = Some(match result {
            Ok(snapshot) if snapshot.at != held.at || snapshot.token.len() > SNAPSHOT_BYTES => {
                Err(Arc::new(invalid("backend snapshot differs from its requested image")))
            }
            Ok(snapshot) => Ok(snapshot),
            Err(error) => Err(Arc::new(error)),
        });
    }

    pub(super) fn snapshot_removed(&mut self, id: u64) {
        self.snapshots.remove(&id);
        // A Released Image Lets The Worker Resume Pruning From The Start.
        self.objects_pruned = 0;
        self.objects_prune_owed = true;
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "read snapshot", reason }
}

/* === Tests === */
