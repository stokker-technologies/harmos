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
use std::fmt;
use std::future::Future;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::manager::{ENTRY_OVERHEAD, Inner, MemoryReservation, State, encoded_size};
use super::selection::Entry;
use super::*;
use crate::supervision::executor;
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */
type Fetch<T> = for<'a> fn(
    &'a StorageView,
    &'a ReadSelection,
    Option<&'a ReadCursor>,
    ReadLimits,
) -> StorageFuture<'a, Fetched<T>>;

/* === Structs === */
#[derive(Clone)]
pub struct StorageView {
    lease: Arc<Lease>,
}

pub struct StorageReader<T> {
    view: Option<StorageView>,
    selection: ReadSelection,
    cursor: Option<ReadCursor>,
    staged_cursor: Option<Option<ReadCursor>>,
    fetch: Fetch<T>,
}

pub struct StoragePage<T> {
    entries: Vec<T>,
    reservation: MemoryReservation,
}

struct Fetched<T> {
    page: ReadPage<T>,
    reservation: MemoryReservation,
}

struct Lease {
    manager: StorageManager<super::manager::ErasedCatalog>,
    id: u64,
}

pub(super) struct Activity {
    inner: Arc<Inner>,
    id: u64,
}

pub(super) struct Pin {
    inner: Arc<Inner>,
    id: u64,
}

#[derive(Clone)]
pub struct StorageViewComputation {
    owner: Arc<ComputationOwner>,
}

struct ComputationOwner {
    view: StorageView,
    started: tokio::time::Instant,
    timeout: Duration,
    _pin: Pin,
    _reservation: super::manager::MemoryReservation,
}

#[derive(Clone, Debug)]
pub(super) struct ComputationLink {
    inner: Weak<Inner>,
    id: u64,
}

pub(super) struct ViewState {
    pub(super) accepted: StorageCursor,
    pub(super) saved: StorageCursor,
    pub(super) result: Option<std::result::Result<Arc<ReadView>, Arc<Error>>>,
    pub(super) orphan: Option<BackendSnapshot>,
    pub(super) opening: bool,
    pub(super) expired: bool,
    pub(super) busy: usize,
    active: usize,
    pub(super) deadline: tokio::time::Instant,
    pub(super) bytes: usize,
    execution: Option<QueryLimits>,
    remaining: Option<Duration>,
    computing: bool,
    computation: Weak<ComputationOwner>,
}

/* === Enums === */
pub(super) enum ViewAction {
    Open(u64, StorageCursor, Option<QueryLimits>),
    Release(u64, BackendSnapshot),
}

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub(super) async fn view(&self) -> Result<StorageView> {
        self.acquire_view(None, None).await.map(|(view, _)| view)
    }

    pub async fn view_with_limits(&self, limits: QueryLimits) -> Result<StorageView> {
        let started = tokio::time::Instant::now();
        limits.check_ceiling(self.inner.settings.query_execution_ceiling)?;
        self.inner.primary.validate_query_limits(limits)?;
        let timeout = super::query::remaining_budget(limits.timeout, started)?;
        let (view, _) = tokio::time::timeout(timeout, self.acquire_view(None, Some(limits)))
            .await
            .map_err(|_| Error::ResourceExhausted {
                resource: "query computation time",
            })??;
        if started.elapsed() >= limits.timeout {
            return Err(Error::ResourceExhausted {
                resource: "query computation time",
            });
        }
        view.limit_computation(limits.timeout.saturating_sub(started.elapsed()))?;
        Ok(view)
    }

    pub(super) async fn acquire_view(
        &self,
        follow: Option<(&ReadSelection, super::follow::Mode)>,
        execution: Option<QueryLimits>,
    ) -> Result<(StorageView, Option<super::follow::Lease>)> {
        // Cover retained frontiers plus temporary acquisition/release copies.
        let bytes = encoded_size(self.inner.primary.history())?
            .checked_mul(8)
            .and_then(|n| n.checked_add(self.inner.settings.snapshot_bytes.get().checked_mul(4)?))
            .and_then(|n| n.checked_add(ENTRY_OVERHEAD * 2))
            .ok_or(Error::InvalidInput {
                field: "read view",
                reason: "size overflow",
            })?;
        let history_bytes = encoded_size(self.inner.primary.history())?;
        let follower_bytes = follow
            .map(|(selection, _)| {
                encoded_size(selection)?
                    .checked_mul(3)
                    .and_then(|bytes| bytes.checked_add(history_bytes.checked_mul(4)?))
                    .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD * 4))
                    .ok_or(Error::ResourceExhausted {
                        resource: "storage follower metadata",
                    })
            })
            .transpose()?
            .unwrap_or(0);
        let bytes = bytes
            .checked_add(execution.map_or(0, |limits| limits.memory_bytes.get()))
            .ok_or(Error::ResourceExhausted {
                resource: "storage query memory",
            })?;
        let total_bytes = bytes
            .checked_add(follower_bytes)
            .ok_or(Error::ResourceExhausted {
                resource: "storage read memory",
            })?;
        if total_bytes > self.inner.settings.read_memory_bytes.get() {
            return Err(Error::InvalidInput {
                field: "read view",
                reason: "view exceeds read memory budget",
            });
        }
        let mut changed = self.inner.changed.subscribe();
        let id = {
            let mut state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if !state.ready || !state.status.running {
                return Err(Error::NotReady { domain: "storage" });
            }
            state.collect();
            if state.views.len() >= self.inner.settings.max_views.get()
                || state.status.read_bytes
                    > self.inner.settings.read_memory_bytes.get() - total_bytes
            {
                return Err(Error::ResourceExhausted {
                    resource: "storage views",
                });
            }
            if follow.is_some() && state.followers.len() >= self.inner.settings.max_followers.get()
            {
                return Err(Error::ResourceExhausted {
                    resource: "storage followers",
                });
            }
            if follow.is_some_and(|(selection, mode)| {
                let through = match mode {
                    super::follow::Mode::Live => state.status.accepted.sequence,
                    super::follow::Mode::Replay => {
                        selection.through.unwrap_or(state.status.accepted.sequence)
                    }
                };
                selection.after > through || through > state.status.accepted.sequence
            }) {
                return Err(Error::InvalidInput {
                    field: "storage follow",
                    reason: "range lies outside accepted data",
                });
            }
            let id = NEXT_VIEW
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| Error::InvalidInput {
                    field: "read view",
                    reason: "identity exhausted",
                })?;
            let accepted = state.status.accepted.clone();
            let saved = state.status.saved.clone();
            if let Some((selection, mode)) = follow {
                state.followers.insert(
                    id,
                    super::follow::Following::new(
                        saved.sequence.max(selection.after),
                        match mode {
                            super::follow::Mode::Live => None,
                            super::follow::Mode::Replay => {
                                Some(selection.through.unwrap_or(accepted.sequence))
                            }
                        },
                        follower_bytes,
                        self.inner.settings.view_idle_timeout,
                    ),
                );
            }
            state.views.insert(
                id,
                ViewState {
                    accepted,
                    saved,
                    result: None,
                    orphan: None,
                    opening: false,
                    expired: false,
                    busy: 0,
                    active: 0,
                    deadline: executor::now() + self.inner.settings.view_idle_timeout,
                    bytes,
                    execution,
                    remaining: execution.map(|limits| limits.timeout),
                    computing: false,
                    computation: Weak::new(),
                },
            );
            state.collect();
            id
        };
        let view = StorageView {
            lease: Arc::new(Lease {
                manager: self.erased(),
                id,
            }),
        };
        let follower = follow.map(|_| super::follow::Lease {
            inner: self.inner.clone(),
            id,
        });
        let _following = follower
            .as_ref()
            .map(|lease| lease.activity())
            .transpose()?;
        self.inner.notify();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                let held = state.views.get(&id).ok_or(Error::ViewExpired)?;
                if held.expired || held.idle_expired(executor::now()) {
                    return Err(Error::ViewExpired);
                }
                if let Some(result) = &held.result {
                    result.as_ref().map_err(|source| Error::Persistence {
                        source: source.clone(),
                    })?;
                    return Ok((view, follower));
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
            }
            changed.changed().await.map_err(|_| Error::ViewExpired)?;
        }
    }

    pub(super) async fn release_views(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        self.inner.notify();
        loop {
            {
                let mut state = self.inner.state.lock().unwrap();
                for view in state.views.values_mut() {
                    view.expired = true;
                }
                state.collect();
                if state.views.is_empty() {
                    return Ok(());
                }
                if let Some(error) = &state.status.failure {
                    return Err(Error::Persistence {
                        source: error.clone(),
                    });
                }
                if !state.status.running {
                    return Err(Error::NotReady {
                        domain: "storage worker",
                    });
                }
            }
            changed.changed().await.map_err(|_| Error::ViewExpired)?;
        }
    }
}

impl StorageView {
    pub fn begin_computation(&self) -> Result<StorageViewComputation> {
        self.computation()?.ok_or(Error::InvalidInput {
            field: "storage view computation",
            reason: "original view has no bounded computation profile",
        })
    }

    pub(super) fn manager(&self) -> &StorageManager<super::manager::ErasedCatalog> {
        &self.lease.manager
    }

    pub fn accepted(&self) -> Result<StorageCursor> {
        let (_pin, view) = self.pin()?;
        Ok(view.accepted().clone())
    }

    pub fn saved(&self) -> Result<StorageCursor> {
        let (_pin, view) = self.pin()?;
        Ok(view.saved().clone())
    }

    pub(super) fn read_objects(
        &self,
        selection: ReadSelection,
    ) -> Result<StorageReader<ObjectEntry>> {
        self.reader::<ObjectEntry>(selection)
    }

    // Open Original Raw Saved Paging Under This View And Its Execution Limits.
    // ReadSelection::through Selects A Fixed Cut Within The Original Snapshot.
    pub fn read_records(&self, selection: ReadSelection) -> Result<StorageReader<StoredRecord>> {
        self.reader::<StoredRecord>(selection)
    }

    pub(super) fn read_stream(&self, selection: ReadSelection) -> Result<StorageReader<StoredRow>> {
        self.reader::<StoredRow>(selection)
    }

    pub fn execution_limits(&self) -> Result<Option<QueryLimits>> {
        let state = self.lease.manager.inner.state.lock().unwrap();
        let view = state.views.get(&self.lease.id).ok_or(Error::ViewExpired)?;
        if view.expired || view.idle_expired(executor::now()) {
            return Err(Error::ViewExpired);
        }
        Ok(view.execution)
    }

    pub fn decoded_bytes(&self) -> Result<NonZeroUsize> {
        self.execution_limits()?;
        Ok(self.manager().inner.settings.query_decoded_bytes)
    }

    pub fn prepare_work(&self) -> Result<super::StorageReadWorkPreparation> {
        super::StorageReadWorkPreparation::saved(self.clone(), self.decoded_bytes()?)
    }

    pub fn project_owned<T: MemorySize, U: MemorySize>(
        &self,
        input: QueryValue<T>,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        input.check_origin(self.manager())?;
        self.project_value(max_bytes, || input.try_map(max_bytes, map))
    }

    pub fn project_shared<T: MemorySize, U: MemorySize>(
        &self,
        input: &SharedQueryValue<T>,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&T, NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        input.check_origin(self.manager())?;
        self.project_value(max_bytes, || input.try_project(max_bytes, map))
    }

    fn project_value<T>(
        &self,
        max_bytes: NonZeroUsize,
        project: impl FnOnce() -> Result<QueryValue<T>>,
    ) -> Result<QueryValue<T>> {
        if max_bytes > self.manager().inner.settings.query_decoded_bytes {
            return Err(super::query::decoded_limit());
        }
        let computation = self.computation()?.ok_or(Error::InvalidInput {
            field: "saved view projection",
            reason: "view has no bounded computation profile",
        })?;
        let _live = self.pin()?;
        computation.check()?;
        let output = project()?;
        computation.finish(output)
    }

    pub(super) fn limit_computation(&self, ceiling: Duration) -> Result<()> {
        let inner = &self.lease.manager.inner;
        let mut state = inner.state.lock().unwrap();
        let view = state
            .views
            .get_mut(&self.lease.id)
            .ok_or(Error::ViewExpired)?;
        if view.expired || view.idle_expired(executor::now()) {
            return Err(Error::ViewExpired);
        }
        if view.computing {
            return Err(Error::NotReady {
                domain: "saved view computation",
            });
        }
        let remaining = view.remaining.as_mut().ok_or(Error::InvalidInput {
            field: "query computation",
            reason: "view has no execution allowance",
        })?;
        *remaining = (*remaining).min(ceiling);
        if remaining.is_zero() {
            return Err(super::query::execution_timeout());
        }
        Ok(())
    }

    pub(super) fn computation(&self) -> Result<Option<StorageViewComputation>> {
        self.computation_with(None, Duration::ZERO)
    }

    pub(super) fn computation_with(
        &self,
        ceiling: Option<Duration>,
        charged: Duration,
    ) -> Result<Option<StorageViewComputation>> {
        if self.execution_limits()?.is_none() {
            return Ok(None);
        }
        let bytes = std::mem::size_of::<ComputationOwner>() + 2 * std::mem::size_of::<usize>();
        let reservation = self.manager().reserve_read_now(bytes, 1)?;
        let inner = &self.lease.manager.inner;
        let mut state = inner.state.lock().unwrap();
        let view = state
            .views
            .get_mut(&self.lease.id)
            .ok_or(Error::ViewExpired)?;
        if view.expired || view.idle_expired(executor::now()) {
            return Err(Error::ViewExpired);
        }
        if view.remaining.is_none() {
            return Ok(None);
        }
        if view.computing {
            return Err(Error::NotReady {
                domain: "saved view computation",
            });
        }
        let remaining = view
            .remaining
            .as_mut()
            .expect("bounded view owns allowance");
        *remaining = ceiling
            .map_or(*remaining, |ceiling| (*remaining).min(ceiling))
            .saturating_sub(charged);
        let timeout = *remaining;
        if timeout.is_zero() {
            return Err(super::query::execution_timeout());
        }
        view.computing = true;
        view.active += 1;
        view.busy += 1;
        let owner = Arc::new(ComputationOwner {
            view: self.clone(),
            started: tokio::time::Instant::now(),
            timeout,
            _pin: Pin {
                inner: inner.clone(),
                id: self.lease.id,
            },
            _reservation: reservation,
        });
        view.computation = Arc::downgrade(&owner);
        Ok(Some(StorageViewComputation { owner }))
    }

    pub(super) async fn compute<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let computation = self.computation()?;
        let result = match &computation {
            Some(active) => tokio::time::timeout(active.timeout(), work)
                .await
                .map_err(|_| super::query::execution_timeout())?,
            None => work.await,
        };
        if let Some(active) = &computation {
            active.check()?;
        }
        result
    }

    pub(super) fn reader<T: Entry>(&self, selection: ReadSelection) -> Result<StorageReader<T>> {
        let (_, view) = self.pin()?;
        view.check_kind(T::KIND, &selection, None)?;
        if selection.record_filter.is_some() {
            let limits = view.execution_limits().ok_or(Error::InvalidInput {
                field: "record filter",
                reason: "requires bounded saved-query limits",
            })?;
            self.manager()
                .inner
                .primary
                .validate_record_filter(&selection, limits)?;
        }
        if T::KIND == ReadKind::Objects {
            self.manager()
                .inner
                .primary
                .validate_object_versions(selection.object_versions)?;
        }
        Ok(StorageReader {
            view: Some(self.clone()),
            selection,
            cursor: None,
            staged_cursor: None,
            fetch: fetch::<T>,
        })
    }

    pub(super) fn activity(&self) -> Result<Activity> {
        let inner = &self.lease.manager.inner;
        let mut state = inner.state.lock().unwrap();
        let view = state
            .views
            .get_mut(&self.lease.id)
            .ok_or(Error::ViewExpired)?;
        if view.expired || view.idle_expired(executor::now()) {
            return Err(Error::ViewExpired);
        }
        view.active += 1;
        Ok(Activity {
            inner: inner.clone(),
            id: self.lease.id,
        })
    }

    pub(super) fn pin(&self) -> Result<(Pin, Arc<ReadView>)> {
        let inner = &self.lease.manager.inner;
        let mut state = inner.state.lock().unwrap();
        let view = state
            .views
            .get_mut(&self.lease.id)
            .ok_or(Error::ViewExpired)?;
        if view.expired || view.idle_expired(executor::now()) {
            return Err(Error::ViewExpired);
        }
        let result = view.result.as_ref().ok_or(Error::NotReady {
            domain: "read view",
        })?;
        let metadata = result
            .as_ref()
            .map_err(|source| Error::Persistence {
                source: source.clone(),
            })?
            .clone();
        view.busy += 1;
        Ok((
            Pin {
                inner: inner.clone(),
                id: self.lease.id,
            },
            metadata,
        ))
    }
}

impl<T> StorageReader<T> {
    pub fn view(&self) -> Option<&StorageView> {
        self.view.as_ref()
    }

    pub(super) fn reserve_output(
        &self,
        bytes: std::num::NonZeroUsize,
    ) -> Result<MemoryReservation> {
        let view = self.view.as_ref().ok_or(Error::ResourceExhausted {
            resource: "exhausted query",
        })?;
        let _live = view.pin()?;
        view.manager().reserve_read_now(bytes.get(), 1)
    }

    pub(super) fn exhausted(&self) -> bool {
        self.view.is_none()
    }

    pub async fn next_page(&mut self, limits: ReadLimits) -> Result<Option<StoragePage<T>>> {
        let Some(view) = self.view.clone() else {
            return Ok(None);
        };
        let result = view
            .compute(self.stage_page_reserved(limits, |entries, reservation| {
                Ok(StoragePage::new(entries, reservation))
            }))
            .await;
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                self.staged_cursor = None;
                return Err(error);
            }
        };
        let Some((entries, empty)) = page else {
            return Ok(None);
        };
        self.commit_staged_page();
        if empty { Ok(None) } else { Ok(Some(entries)) }
    }

    // The Internal Caller Already Owns The Original View Computation.
    pub(super) async fn next_page_in_computation(
        &mut self,
        limits: ReadLimits,
    ) -> Result<Option<StoragePage<T>>> {
        let Some((entries, empty)) = self
            .stage_page_reserved(limits, |entries, reservation| {
                Ok(StoragePage::new(entries, reservation))
            })
            .await?
        else {
            return Ok(None);
        };
        self.commit_staged_page();
        if empty { Ok(None) } else { Ok(Some(entries)) }
    }

    pub(super) async fn stage_page_mapped<U>(
        &mut self,
        limits: ReadLimits,
        map: impl FnOnce(Vec<T>) -> Result<U>,
    ) -> Result<Option<(U, bool)>> {
        self.stage_page_reserved(limits, |entries, reservation| {
            let entries = map(entries)?;
            drop(reservation);
            Ok(entries)
        })
        .await
    }

    pub(super) fn commit_staged_page(&mut self) {
        self.cursor = self
            .staged_cursor
            .take()
            .expect("original staged page owns its next cursor");
        if self.cursor.is_none() {
            self.view = None;
        }
    }

    async fn stage_page_reserved<U>(
        &mut self,
        limits: ReadLimits,
        map: impl FnOnce(Vec<T>, MemoryReservation) -> Result<U>,
    ) -> Result<Option<(U, bool)>> {
        if self.staged_cursor.is_some() {
            return Err(Error::NotReady {
                domain: "original staged query page",
            });
        }
        let Some(view) = &self.view else {
            return Ok(None);
        };
        view.manager()
            .inner
            .settings
            .query_page_ceiling
            .check(limits.rows.get(), limits.bytes.get())?;
        let fetched = (self.fetch)(view, &self.selection, self.cursor.as_ref(), limits).await?;
        let Fetched { page, reservation } = fetched;
        // A failed typed decoder must leave the same page available for retry.
        let empty = page.entries.is_empty();
        let entries = map(page.entries, reservation)?;
        self.staged_cursor = Some(page.next);
        Ok(Some((entries, empty)))
    }
}

impl<T> StoragePage<T> {
    pub(super) fn new(entries: Vec<T>, reservation: MemoryReservation) -> Self {
        Self {
            entries,
            reservation,
        }
    }

    pub fn try_project<U: crate::MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<Vec<U>>,
    ) -> Result<QueryPage<U>> {
        let reservation = self.reservation.projection(max_bytes)?;
        let entries = map(&self.entries, max_bytes)?;
        if entries.memory_size() > max_bytes.get() {
            return Err(super::query::decoded_limit());
        }
        QueryPage::retain(entries, reservation)
    }
}

impl<T> Deref for StoragePage<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.entries
    }
}

impl<T: fmt::Debug> fmt::Debug for StoragePage<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.entries.fmt(formatter)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let inner = &self.manager.inner;
        let mut state = inner.state.lock().unwrap();
        if let Some(view) = state.views.get_mut(&self.id) {
            view.expired = true;
        }
        state.collect();
        inner.notify();
    }
}

impl StorageViewComputation {
    pub fn deadline(&self) -> tokio::time::Instant {
        self.owner.started + self.owner.timeout
    }

    pub(super) fn timeout(&self) -> Duration {
        self.owner.timeout
    }

    pub(super) fn finish<T>(&self, value: T) -> Result<T> {
        self.check()?;
        Ok(value)
    }

    pub fn check(&self) -> Result<()> {
        if self.owner.started.elapsed() >= self.owner.timeout {
            return Err(super::query::execution_timeout());
        }
        Ok(())
    }
}

impl Drop for ComputationOwner {
    fn drop(&mut self) {
        let inner = &self.view.lease.manager.inner;
        let mut state = inner.state.lock().unwrap();
        if let Some(view) = state.views.get_mut(&self.view.lease.id) {
            if let Some(remaining) = &mut view.remaining {
                *remaining = remaining.saturating_sub(self.started.elapsed());
            }
            view.computing = false;
            view.computation = Weak::new();
            view.active -= 1;
            view.deadline = executor::now() + inner.settings.view_idle_timeout;
        }
        state.collect();
        inner.notify();
    }
}

impl ComputationLink {
    pub(super) fn new(inner: Weak<Inner>, id: u64) -> Self {
        Self { inner, id }
    }

    pub(super) fn retain(&self) -> Result<StorageViewComputation> {
        let inner = self.inner.upgrade().ok_or(Error::ViewExpired)?;
        let state = inner.state.lock().unwrap();
        let view = state.views.get(&self.id).ok_or(Error::ViewExpired)?;
        let owner = view.computation.upgrade().ok_or(Error::NotReady {
            domain: "original saved view computation",
        })?;
        Ok(StorageViewComputation { owner })
    }
}

impl PartialEq for ComputationLink {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Weak::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for ComputationLink {}

impl Drop for Activity {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(view) = state.views.get_mut(&self.id) {
            view.active -= 1;
            if view.active == 0 && !view.expired {
                view.deadline = executor::now() + self.inner.settings.view_idle_timeout;
            }
        }
        state.collect();
        self.inner.notify();
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(view) = state.views.get_mut(&self.id) {
            view.busy -= 1;
            if view.busy == 0 && !view.expired {
                view.deadline = executor::now() + self.inner.settings.view_idle_timeout;
            }
        }
        state.collect();
        self.inner.notify();
    }
}

impl ViewState {
    fn idle_expired(&self, now: tokio::time::Instant) -> bool {
        self.idle_deadline().is_some_and(|deadline| now >= deadline)
    }

    fn idle_deadline(&self) -> Option<tokio::time::Instant> {
        (self.result.is_some() && !self.opening && self.busy == 0 && self.active == 0)
            .then_some(self.deadline)
    }

    fn backend(&self) -> Option<&BackendSnapshot> {
        self.orphan.as_ref().or_else(|| {
            self.result
                .as_ref()?
                .as_ref()
                .ok()
                .map(|view| view.snapshot())
        })
    }
}

impl State {
    pub(super) fn collect(&mut self) {
        let now = executor::now();
        self.receipts.purge(now);
        self.status.mutation_bytes = self.receipts.snapshot_bytes();
        for view in self.views.values_mut() {
            view.expired |= view.idle_expired(now) || self.status.closing;
        }
        self.views.retain(|_, view| {
            !view.expired || view.busy != 0 || view.opening || view.backend().is_some()
        });
        let mut expired = false;
        for held in self.followers.values_mut() {
            expired |= held.expire(self.status.closing, now);
        }
        if expired {
            self.retire_data();
        }
        self.status.pending_batches = self.reserved_batches
            + self.queue.len()
            + self
                .files
                .values()
                .filter(|file| file.pending_batch())
                .count();
        self.status.pending_bytes = self.reserved
            + self.scratch_bytes
            + self.replica_base_bytes
            + self.queue.bytes()
            + self.files.values().map(|file| file.bytes()).sum::<usize>();
        self.status.read_bytes = self.read_bytes
            + self
                .deletions
                .values()
                .map(|deletion| deletion.bytes)
                .sum::<usize>()
            + self.saves.values().map(|save| save.bytes).sum::<usize>()
            + self.views.values().map(|view| view.bytes).sum::<usize>()
            + self
                .followers
                .values()
                .map(|held| held.bytes)
                .sum::<usize>();
    }

    pub(super) fn view_action(&mut self, allow_release: bool) -> Option<ViewAction> {
        self.collect();
        for (&id, view) in &mut self.views {
            if view.expired && view.busy == 0 && allow_release {
                if let Some(snapshot) = view.backend() {
                    return Some(ViewAction::Release(id, snapshot.clone()));
                }
            } else if !view.expired && !view.opening && view.result.is_none() {
                view.opening = true;
                return Some(ViewAction::Open(id, view.saved.clone(), view.execution));
            }
        }
        None
    }

    pub(super) fn opened(
        &mut self,
        id: u64,
        inner: Weak<Inner>,
        snapshot: Result<BackendSnapshot>,
        max_bytes: usize,
        idle_timeout: std::time::Duration,
    ) {
        if let Some(view) = self.views.get_mut(&id) {
            view.opening = false;
            view.deadline = executor::now() + idle_timeout;
            view.result = Some(match snapshot {
                Err(error) => Err(Arc::new(error)),
                Ok(snapshot) => {
                    let result = if snapshot.token.len() > max_bytes {
                        Err(Error::InvalidInput {
                            field: "backend snapshot",
                            reason: "token exceeds memory bound",
                        })
                    } else {
                        ReadView::new(
                            Identifier::new(format!("storage-view-{id}")).unwrap(),
                            view.accepted.clone(),
                            view.saved.clone(),
                            snapshot.clone(),
                        )
                    };
                    match result {
                        Ok(result) => Ok(Arc::new(
                            result
                                .with_execution(view.execution)
                                .with_computation(ComputationLink::new(inner, id)),
                        )),
                        Err(error) => {
                            view.orphan = Some(snapshot);
                            Err(Arc::new(error))
                        }
                    }
                }
            });
        }
        self.collect();
    }

    pub(super) fn view_deadline(&self) -> Option<tokio::time::Instant> {
        self.views
            .values()
            .filter(|view| !view.expired)
            .filter_map(ViewState::idle_deadline)
            .chain(
                self.followers
                    .values()
                    .filter_map(super::follow::Following::idle_deadline),
            )
            .min()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn fetch<'a, T: Entry>(
    owned: &'a StorageView,
    selection: &'a ReadSelection,
    cursor: Option<&'a ReadCursor>,
    limits: ReadLimits,
) -> StorageFuture<'a, Fetched<T>> {
    Box::pin(async move {
        let _activity = owned.activity()?;
        let manager = &owned.lease.manager;
        let limits = T::read_limits(limits);
        let _lifecycle = manager.inner.lifecycle.lock().await;
        // Reserve the bounded backend page and cursor/filter metadata before I/O.
        // Identifiers are unbounded; fixed per-row overhead cannot cover them.
        let cursor_bytes = encoded_size(selection)?
            .checked_add(encoded_size(manager.inner.primary.history())?)
            .and_then(|bytes| bytes.checked_add(ENTRY_OVERHEAD))
            .and_then(|bytes| bytes.checked_mul(3))
            .ok_or(Error::InvalidInput {
                field: "read page",
                reason: "size overflow",
            })?;
        let page_bytes = if T::KIND == super::ReadKind::Schemas {
            limits
                .bytes
                .get()
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(T::decoding_overhead(limits)?))
                .ok_or(Error::InvalidInput {
                    field: "read page",
                    reason: "schema decode bound overflow",
                })?
        } else {
            limits.bytes.get()
        };
        let bytes = page_bytes
            .checked_add(cursor_bytes)
            .ok_or(Error::InvalidInput {
                field: "read page",
                reason: "size overflow",
            })?;
        let reservation = manager.reserve_read(bytes, limits.rows.get()).await?;
        let (_pin, view) = owned.pin()?;
        view.check_kind(T::KIND, selection, cursor)?;
        let page = manager
            .inner
            .bounded(T::fetch(
                &*manager.inner.primary,
                &view,
                selection,
                cursor,
                limits,
            ))
            .await?;
        page.check_positions(&view, selection, cursor, T::position)?;
        page.check(limits, |entry| encoded_size(entry).unwrap_or(usize::MAX))?;
        if page.entries.iter().any(|entry| !entry.matches(selection)) {
            return Err(Error::InvalidInput {
                field: "read page",
                reason: "backend returned data outside saved selection",
            });
        }
        for entry in &page.entries {
            entry.validate_read(&manager.inner, limits)?;
        }
        // Explicit expiry during backend I/O must still fail, never rebase.
        let _still_live = owned.pin()?;
        Ok(Fetched { page, reservation })
    })
}

/* === Tests === */
