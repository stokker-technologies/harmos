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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::manager::{
    Domain, ENTRY_OVERHEAD, Inner, MemoryReservation, ReservationKind, State, encoded_size,
};
use super::selection::Entry;
use super::*;
use crate::supervision::executor;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(super) type BorrowedDecode<R, T> = dyn Fn(&R, NonZeroUsize) -> Result<T> + Send + Sync;
type Fetch<T> = fn(&Lease, &ReadSelection, ReadLimits) -> Result<LivePage<T>>;

/* === Structs === */
pub struct QueryFollower<R, T> {
    source: StorageFollower<R>,
    pending: Option<StoragePage<R>>,
    decoded: Option<QueryPage<T>>,
    decode: Box<BorrowedDecode<R, T>>,
    page: ReadLimits,
    decoded_bytes: NonZeroUsize,
    remaining: Duration,
}

pub struct QueryReplay<R, T> {
    inner: Option<QueryFollower<R, T>>,
    boundary: StorageCursor,
}

pub(crate) struct StorageFollower<T> {
    lease: Lease,
    history: Option<StorageReader<T>>,
    selection: ReadSelection,
    saved: StorageCursor,
    accepted: StorageCursor,
    changed: watch::Receiver<u64>,
    fetch: Fetch<T>,
}

pub(super) struct Lease {
    pub(super) inner: Arc<Inner>,
    pub(super) id: u64,
}

pub(super) struct Following {
    pub(super) after: u64,
    through: Option<u64>,
    pub(super) bytes: usize,
    pub(super) active: usize,
    pub(super) deadline: tokio::time::Instant,
    failure: Option<Failure>,
}

pub(super) struct Activity {
    inner: Arc<Inner>,
    id: u64,
}

struct LivePage<T> {
    entries: Vec<T>,
    reservation: Option<MemoryReservation>,
    advanced: bool,
    done: bool,
}

/* === Enums === */
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Live,
    Replay,
}

#[derive(Clone, Copy)]
enum Failure {
    Gap { requested: u64, available: u64 },
    Expired,
    Closed,
}

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub(super) async fn follow_raw<T: Entry>(
        &self,
        mut selection: ReadSelection,
        mode: Mode,
        execution: QueryLimits,
    ) -> Result<StorageFollower<T>> {
        if selection.kind != T::KIND
            || selection.order != ReadOrder::Ascending
            || selection.history != super::ReadHistory::Complete
            || selection.object_versions != super::ObjectVersions::Revisions
            || (matches!(mode, Mode::Live) && selection.through.is_some())
        {
            return Err(Error::InvalidInput {
                field: "storage follow",
                reason: "requires complete ascending history and matching kind; live following must be open-ended",
            });
        }
        let (view, lease) = tokio::time::timeout(
            execution.timeout,
            self.acquire_view(Some((&selection, mode)), Some(execution)),
        )
        .await
        .map_err(|_| super::query::execution_timeout())??;
        let lease = lease.expect("follow acquisition registers retention atomically");
        let saved = view.saved()?;
        let accepted = view.accepted()?;
        if matches!(mode, Mode::Replay) {
            selection.through = Some(selection.through.unwrap_or(accepted.sequence));
        }
        let history_through = selection
            .through
            .unwrap_or(saved.sequence)
            .min(saved.sequence);
        let mut historical = selection.clone();
        historical.after = historical.after.min(history_through);
        historical.through = Some(history_through);
        let reader = view.reader::<T>(historical)?;
        let history = (selection.after < history_through).then_some(reader);
        let changed = self.inner.changed.subscribe();
        Ok(StorageFollower {
            lease,
            history,
            selection,
            saved,
            accepted,
            changed,
            fetch: fetch::<T>,
        })
    }
}

impl<C> StorageManager<C> {
    pub async fn follow<Q: super::query::FollowQuery<C>>(
        &self,
        query: Q,
    ) -> Result<QueryFollower<Q::Raw, Q::Value>> {
        query.open_follow(self, false).await
    }

    pub async fn replay<Q: super::query::FollowQuery<C>>(
        &self,
        query: Q,
    ) -> Result<QueryReplay<Q::Raw, Q::Value>> {
        let follower = query.open_follow(self, true).await?;
        let boundary = StorageCursor {
            history: follower.source.accepted.history.clone(),
            sequence: follower
                .source
                .selection
                .through
                .expect("replay fixes a frontier"),
        };
        let inner = (!follower.source.finished()).then_some(follower);
        Ok(QueryReplay { inner, boundary })
    }
}

impl<R, T: MemorySize> QueryFollower<R, T> {
    pub(super) fn new(
        source: StorageFollower<R>,
        decode: Box<BorrowedDecode<R, T>>,
        page: ReadLimits,
        decoded_bytes: NonZeroUsize,
        remaining: Duration,
    ) -> Self {
        Self {
            source,
            pending: None,
            decoded: None,
            decode,
            page,
            decoded_bytes,
            remaining,
        }
    }

    pub fn saved_boundary(&self) -> &StorageCursor {
        self.source.saved_boundary()
    }
    pub fn accepted_boundary(&self) -> &StorageCursor {
        self.source.accepted_boundary()
    }

    pub async fn next_page(&mut self) -> Result<QueryPage<T>> {
        self.read_page().await?.ok_or(Error::Closing {
            domain: "live following",
        })
    }

    pub async fn next_page_projected<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<QueryValue<U>> {
        self.project_page_with(max_bytes, |page, bytes| page.try_project(bytes, map))
            .await?
            .ok_or(Error::Closing {
                domain: "live following",
            })
    }

    pub async fn next_page_projected_shared<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<SharedQueryValue<U>> {
        self.project_page_with(max_bytes, |page, bytes| {
            page.try_project(bytes, map)?.try_into_shared()
        })
        .await?
        .ok_or(Error::Closing {
            domain: "live following",
        })
    }

    async fn project_page_with<U>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&QueryPage<T>, NonZeroUsize) -> Result<U>,
    ) -> Result<Option<U>> {
        if max_bytes > self.decoded_bytes {
            return Err(super::query::decoded_limit());
        }
        let output = self.read_page_with(|page| map(page, max_bytes)).await?;
        if output.is_some() {
            self.decoded.take();
        }
        Ok(output)
    }

    async fn read_page(&mut self) -> Result<Option<QueryPage<T>>> {
        match self.read_page_with(|_| Ok(())).await? {
            Some(()) => Ok(self.decoded.take()),
            None => Ok(None),
        }
    }

    async fn read_page_with<U>(
        &mut self,
        map: impl FnOnce(&QueryPage<T>) -> Result<U>,
    ) -> Result<Option<U>> {
        let _live = self.source.lease.activity()?;
        if self.remaining.is_zero() {
            return Err(super::query::execution_timeout());
        }
        if self.decoded.is_none() && self.pending.is_none() {
            self.pending = self
                .source
                .read_page(self.page, Some(&mut self.remaining))
                .await?;
        }
        if self.decoded.is_none() && self.pending.is_none() {
            return Ok(None);
        }
        let timeout = self.remaining;
        let started = tokio::time::Instant::now();
        let _active = super::query::ActiveBudget {
            remaining: &mut self.remaining,
            started,
        };
        if self.decoded.is_none() {
            let pending = self.pending.as_ref().expect("original raw page is pending");
            let decode = &self.decode;
            let page = pending.try_project(self.decoded_bytes, |entries, bytes| {
                let inline = entries
                    .len()
                    .checked_mul(std::mem::size_of::<T>())
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<T>>()))
                    .ok_or_else(super::query::decoded_limit)?;
                if inline > bytes.get() {
                    return Err(super::query::decoded_limit());
                }
                let mut output = Vec::with_capacity(entries.len());
                let mut retained = std::mem::size_of::<Vec<T>>()
                    .saturating_add(output.capacity().saturating_mul(std::mem::size_of::<T>()));
                for entry in entries {
                    if started.elapsed() >= timeout {
                        return Err(super::query::execution_timeout());
                    }
                    let allowance = NonZeroUsize::new(bytes.get().saturating_sub(retained))
                        .ok_or_else(super::query::decoded_limit)?;
                    let value = decode(entry, allowance)?;
                    retained = retained.saturating_add(
                        value.memory_size().saturating_sub(std::mem::size_of::<T>()),
                    );
                    if retained > bytes.get() {
                        return Err(super::query::decoded_limit());
                    }
                    output.push(value);
                }
                if started.elapsed() >= timeout {
                    return Err(super::query::execution_timeout());
                }
                Ok(output)
            })?;
            self.decoded = Some(page);
            self.pending.take();
        }
        if started.elapsed() >= timeout {
            return Err(super::query::execution_timeout());
        }
        let page = self
            .decoded
            .as_ref()
            .expect("original decoded page is pending");
        let output = map(page)?;
        if started.elapsed() >= timeout {
            return Err(super::query::execution_timeout());
        }
        self.source.lease.check()?;
        Ok(Some(output))
    }
}

impl<R, T: MemorySize> QueryReplay<R, T> {
    pub fn boundary(&self) -> &StorageCursor {
        &self.boundary
    }

    pub async fn next_page(&mut self) -> Result<Option<QueryPage<T>>> {
        let Some(inner) = &mut self.inner else {
            return Ok(None);
        };
        let page = inner.read_page().await?;
        if inner.pending.is_none() && inner.decoded.is_none() && inner.source.finished() {
            self.inner = None;
        }
        Ok(page)
    }

    pub async fn next_page_projected<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<Option<QueryValue<U>>> {
        let Some(inner) = &mut self.inner else {
            return Ok(None);
        };
        let output = inner
            .project_page_with(max_bytes, |page, bytes| page.try_project(bytes, map))
            .await?;
        if inner.pending.is_none() && inner.decoded.is_none() && inner.source.finished() {
            self.inner = None;
        }
        Ok(output)
    }

    pub async fn next_page_projected_shared<U: MemorySize>(
        &mut self,
        max_bytes: NonZeroUsize,
        map: impl FnOnce(&[T], NonZeroUsize) -> Result<U>,
    ) -> Result<Option<SharedQueryValue<U>>> {
        let Some(inner) = &mut self.inner else {
            return Ok(None);
        };
        let output = inner
            .project_page_with(max_bytes, |page, bytes| {
                page.try_project(bytes, map)?.try_into_shared()
            })
            .await?;
        if inner.pending.is_none() && inner.decoded.is_none() && inner.source.finished() {
            self.inner = None;
        }
        Ok(output)
    }
}

impl<T> StorageFollower<T> {
    pub fn saved_boundary(&self) -> &StorageCursor {
        &self.saved
    }
    pub fn accepted_boundary(&self) -> &StorageCursor {
        &self.accepted
    }

    async fn read_page(
        &mut self,
        limits: ReadLimits,
        mut remaining: Option<&mut Duration>,
    ) -> Result<Option<StoragePage<T>>> {
        let _activity = self.lease.activity()?;
        if let Some(history) = &mut self.history {
            let page = if let Some(remaining) = remaining.as_deref_mut() {
                if remaining.is_zero() {
                    return Err(super::query::execution_timeout());
                }
                let timeout = *remaining;
                let _active = super::query::ActiveBudget {
                    remaining,
                    started: tokio::time::Instant::now(),
                };
                let computation = history
                    .view()
                    .map(|view| view.computation_with(Some(timeout), Duration::ZERO))
                    .transpose()?
                    .flatten();
                let timeout = computation
                    .as_ref()
                    .map_or(timeout, |active| active.timeout().min(timeout));
                let page = tokio::time::timeout(timeout, history.next_page(limits))
                    .await
                    .map_err(|_| super::query::execution_timeout())??;
                if let Some(active) = computation {
                    active.finish(page)?
                } else {
                    page
                }
            } else if let Some(view) = history.view().cloned() {
                view.compute(history.next_page(limits)).await?
            } else {
                history.next_page(limits).await?
            };
            self.lease.check()?;
            if let Some(page) = page {
                return Ok(Some(page));
            }
            self.history = None;
        }
        loop {
            self.changed.borrow_and_update();
            let page = if let Some(remaining) = remaining.as_deref_mut() {
                if remaining.is_zero() {
                    return Err(super::query::execution_timeout());
                }
                let _active = super::query::ActiveBudget {
                    remaining,
                    started: tokio::time::Instant::now(),
                };
                (self.fetch)(&self.lease, &self.selection, limits)?
            } else {
                (self.fetch)(&self.lease, &self.selection, limits)?
            };
            if !page.entries.is_empty() {
                return Ok(Some(StoragePage::new(
                    page.entries,
                    page.reservation
                        .expect("nonempty live page owns its read reservation"),
                )));
            }
            if remaining.as_deref().is_some_and(Duration::is_zero) {
                return Err(super::query::execution_timeout());
            }
            if page.done {
                return Ok(None);
            }
            if page.advanced {
                executor::yield_now().await;
            } else {
                self.changed
                    .changed()
                    .await
                    .map_err(|_| Error::ViewExpired)?;
            }
        }
    }
    fn finished(&self) -> bool {
        self.history.as_ref().is_none_or(StorageReader::exhausted)
            && self
                .lease
                .inner
                .state
                .lock()
                .unwrap()
                .followers
                .get(&self.lease.id)
                .is_some_and(|held| held.through.is_some_and(|through| held.after >= through))
    }
}

impl Following {
    pub(super) fn new(
        after: u64,
        through: Option<u64>,
        bytes: usize,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            after,
            through,
            bytes,
            active: 0,
            deadline: executor::now() + timeout,
            failure: None,
        }
    }
    fn retains(&self) -> bool {
        self.failure.is_none() && self.through.is_none_or(|through| self.after < through)
    }
    fn check(&self) -> Result<()> {
        match self.failure {
            Some(Failure::Gap {
                requested,
                available,
            }) => Err(Error::Gap {
                requested,
                available,
            }),
            Some(Failure::Expired) => Err(Error::ViewExpired),
            Some(Failure::Closed) => Err(Error::Closing { domain: "storage" }),
            None => Ok(()),
        }
    }
    pub(super) fn expire(&mut self, closing: bool, now: tokio::time::Instant) -> bool {
        if self.failure.is_some() {
            return false;
        }
        if closing {
            self.failure = Some(Failure::Closed);
        } else if self.active == 0 && now >= self.deadline {
            self.failure = Some(Failure::Expired);
        }
        self.failure.is_some()
    }
    pub(super) fn idle_deadline(&self) -> Option<tokio::time::Instant> {
        (self.failure.is_none() && self.active == 0).then_some(self.deadline)
    }
}

impl Lease {
    pub(super) fn activity(&self) -> Result<Activity> {
        let mut state = self.inner.state.lock().unwrap();
        state.collect();
        let held = state
            .followers
            .get_mut(&self.id)
            .ok_or(Error::ViewExpired)?;
        held.check()?;
        held.active += 1;
        Ok(Activity {
            inner: self.inner.clone(),
            id: self.id,
        })
    }
    fn check(&self) -> Result<()> {
        let mut state = self.inner.state.lock().unwrap();
        state.collect();
        state
            .followers
            .get(&self.id)
            .ok_or(Error::ViewExpired)?
            .check()
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.followers.remove(&self.id);
        state.retire_data();
        state.collect();
        self.inner.notify();
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(held) = state.followers.get_mut(&self.id) {
            held.active -= 1;
            held.deadline = executor::now() + self.inner.settings.view_idle_timeout;
        }
        state.collect();
        self.inner.notify();
    }
}

impl State {
    pub(super) fn retire_data(&mut self) {
        let through = self
            .followers
            .values()
            .filter(|held| held.retains())
            .map(|held| held.after)
            .min()
            .unwrap_or(u64::MAX);
        self.queue.retire_data(self.data_flushed, through);
    }

    pub(super) fn lag_followers(&mut self) -> bool {
        let Some(through) = self
            .queue
            .iter()
            .filter(|entry| {
                entry.operation.domain() == Domain::Data && entry.index <= self.data_flushed
            })
            .map(|entry| entry.operation.sequence())
            .max()
        else {
            return false;
        };
        let mut changed = false;
        for held in self.followers.values_mut() {
            if held.retains() && held.after < through {
                held.failure = Some(Failure::Gap {
                    requested: held.after.saturating_add(1),
                    available: through.saturating_add(1),
                });
                changed = true;
            }
        }
        if changed {
            self.retire_data();
            self.collect();
        }
        changed
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn fetch<T: Entry>(
    lease: &Lease,
    selection: &ReadSelection,
    limits: ReadLimits,
) -> Result<LivePage<T>> {
    let mut reservation = None;
    let result = (|| {
        let mut state = lease.inner.state.lock().unwrap();
        state.collect();
        let held = state.followers.get(&lease.id).ok_or(Error::ViewExpired)?;
        held.check()?;
        let after = held.after;
        let through = held.through;
        let mut cursor = after;
        let mut entries = Vec::new();
        let mut bytes = 0usize;
        let mut scanned = 0usize;
        let mut reserved = false;
        let scan_limit = limits.rows.get().min(1024);
        // Reserve only when a matching item exists. Empty scans allocate nothing
        // and emit no spurious wakeup, so a parked follower cannot wake itself.
        let charge = limits
            .bytes
            .get()
            .checked_add(limits.rows.get().checked_mul(ENTRY_OVERHEAD).ok_or(
                Error::ResourceExhausted {
                    resource: "storage read memory",
                },
            )?)
            .ok_or(Error::ResourceExhausted {
                resource: "storage read memory",
            })?;
        if charge > lease.inner.settings.read_memory_bytes.get() {
            return Err(Error::ResourceExhausted {
                resource: "storage read memory",
            });
        }
        let available = lease
            .inner
            .settings
            .read_memory_bytes
            .get()
            .saturating_sub(state.status.read_bytes);
        'queue: for pending in state
            .queue
            .iter()
            .filter(|entry| entry.operation.domain() == Domain::Data)
        {
            if through.is_some_and(|through| cursor >= through) {
                break;
            }
            if pending.operation.sequence() <= cursor {
                continue;
            }
            let values = T::pending(&pending.operation);
            if values.is_empty() {
                cursor = pending
                    .operation
                    .sequence()
                    .min(through.unwrap_or(u64::MAX));
                scanned += 1;
            } else {
                let start = values.partition_point(|entry| entry.position().sequence <= cursor);
                for entry in &values[start..] {
                    if scanned >= scan_limit
                        || through.is_some_and(|through| entry.position().sequence > through)
                    {
                        break 'queue;
                    }
                    if entry.matches(selection) {
                        let size = encoded_size(entry)?;
                        if size > limits.bytes.get() {
                            return Err(Error::InvalidInput {
                                field: "read page",
                                reason: "one live entry exceeds the byte bound",
                            });
                        }
                        if entries.len() >= limits.rows.get() || size > limits.bytes.get() - bytes {
                            break 'queue;
                        }
                        if !reserved {
                            if charge > available {
                                return Err(Error::ResourceExhausted {
                                    resource: "storage read memory",
                                });
                            }
                            // The state lock serializes this reservation until it
                            // is recorded below; pending references stay borrowed.
                            reserved = true;
                        }
                        entry.validate_read(&lease.inner, limits)?;
                        bytes += size;
                        entries.push(entry.clone());
                    }
                    cursor = entry.position().sequence;
                    scanned += 1;
                }
            }
            if scanned >= scan_limit {
                break;
            }
        }
        let done = through.is_some_and(|through| cursor >= through);
        if entries.is_empty() && cursor == after && !done {
            if !state.ready || !state.status.running {
                return Err(Error::NotReady {
                    domain: "storage worker",
                });
            }
            if let Some(source) = &state.status.failure {
                return Err(Error::Persistence {
                    source: source.clone(),
                });
            }
        }
        if reserved {
            state.read_bytes += charge;
            reservation = Some(MemoryReservation {
                inner: lease.inner.clone(),
                bytes: charge,
                kind: ReservationKind::Read,
            });
        }
        state.followers.get_mut(&lease.id).unwrap().after = cursor;
        let advanced = cursor > after;
        if advanced {
            state.retire_data();
        }
        state.collect();
        if advanced {
            lease.inner.notify();
        }
        Ok(LivePage {
            entries,
            reservation: reservation.take(),
            advanced,
            done,
        })
    })();
    drop(reservation);
    result
}

/* === Tests === */
