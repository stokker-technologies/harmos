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
use std::any::Any;
use std::collections::VecDeque;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Bound;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::demand::Holding;
use crate::{state::At, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Row = Arc<dyn Any + Send + Sync>;

type Step = Pin<Box<dyn Future<Output = (Parts, Turn)> + Send>>;

/* === Structs === */
pub(crate) struct Window {
    ring: RwLock<Ring>,
    pub(super) visibility: Arc<RwLock<()>>,
    sequence: Arc<AtomicU64>,
    doorbell: watch::Sender<u64>,
    pump: watch::Sender<()>,
    demand: watch::Sender<usize>,
}

struct Ring {
    batches: Option<super::batches::State>,
    rows: VecDeque<(u64, u64, Position, Row)>,
    capacity: usize,
    next: u64,
    last: u64,
    // Latest overwritten delivery cursor; independent of the durable floor.
    evicted: u64,
    // Sidecar loss has no recoverable Position or row identity.
    missed: u64,
    floor: Position,
    standing: bool,
}

#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
pub(super) struct ListenerCandidate {
    pub(super) cursor: u64,
    pub(super) sequence: u64,
    pub(super) source: usize,
}

pub(crate) struct Retained {
    pub(crate) rows: Vec<(Position, Row)>,
    pub(crate) floor: Position,
    pub(crate) beneath: bool,
}

pub(crate) struct Advanced {
    pub(crate) missed: u64,
    pub(crate) rows: Vec<(Position, Row)>,
    pub(crate) frontier: u64,
}

struct Parts {
    window: Arc<Window>,
    doorbell: watch::Receiver<u64>,
    frontier: u64,
    closed: Arc<AtomicBool>,
    #[allow(
        dead_code,
        reason = "held for its drop, which releases the demand count"
    )]
    held: Holding,
}

struct Following<S> {
    parts: Option<Parts>,
    step: Option<Step>,
    gap: u64,
    pending: VecDeque<(Position, Row)>,
    done: bool,
    marker: PhantomData<fn() -> S>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item<S> {
    Row(At<S>),
    Gap { missed: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BeneathWindow {
    #[error("no series adapter serves beneath the retained window")]
    Unserved,
    #[error("the served history begins after position {}", floor.0)]
    Floor { floor: Position },
    #[error("the series adapter could not read its own history: {0}")]
    Unreadable(String),
}

enum Turn {
    Advanced(Advanced),
    Ended,
}

/* === Traits === */

/* === Implementations === */
impl Window {
    pub(crate) fn wired(
        capacity: usize,
        floor: Position,
        sequence: Arc<AtomicU64>,
        pump: watch::Sender<()>,
    ) -> Self {
        let (doorbell, _) = watch::channel(0);
        let (demand, _) = watch::channel(0);

        Self {
            visibility: Arc::new(RwLock::new(())),
            ring: RwLock::new(Ring {
                batches: None,
                rows: VecDeque::with_capacity(capacity),
                capacity,
                next: 0,
                last: 0,
                evicted: 0,
                missed: 0,
                floor,
                standing: false,
            }),
            sequence,
            doorbell,
            pump,
            demand,
        }
    }

    pub(super) fn visible_with(mut self, visibility: Arc<RwLock<()>>) -> Self {
        self.visibility = visibility;
        self
    }

    pub(crate) fn batching(self, definition: Option<super::BatchDefinition>) -> Self {
        self.ring.write().expect("ring is healthy").batches =
            definition.map(super::batches::State::new);
        self
    }

    pub(crate) fn batches<S>(&self, held: Holding) -> super::Batches<S> {
        let ring = self.ring.read().expect("ring is healthy");
        ring.batches
            .as_ref()
            .expect("stream declared batching")
            .feed
            .following(held)
    }

    pub(crate) fn close_batches(&self) {
        if let Some(batches) = &mut self.ring.write().expect("ring is healthy").batches {
            batches.close();
        }
    }

    pub(crate) fn publish(&self, at: Position, row: Row) {
        self.append(at, row, 0)
            .expect("native stream cursor exhausted");
    }

    pub(super) fn append(
        &self,
        at: Position,
        row: Row,
        missed: u64,
    ) -> Result<(), super::StreamPublicationError> {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let evicted = self.append_locked(at, row, missed)?;
        drop(_visibility);
        drop(evicted);
        self.knock();
        self.pump.send_modify(|()| {});
        Ok(())
    }

    // Caller holds the facility visibility fence. Notifications follow scope commit.
    pub(super) fn append_locked(
        &self,
        at: Position,
        row: Row,
        missed: u64,
    ) -> Result<Option<Row>, super::StreamPublicationError> {
        let evicted;
        {
            let mut ring = self.ring.write().expect("the ring is never poisoned");
            let advance = missed
                .checked_add(1)
                .ok_or(super::StreamPublicationError::Sequence)?;
            let next = ring
                .next
                .checked_add(advance)
                .ok_or(super::StreamPublicationError::Sequence)?;
            let previous = self
                .sequence
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |cursor| {
                    cursor.checked_add(advance)
                })
                .map_err(|_| super::StreamPublicationError::Sequence)?;
            let cursor = previous + advance;
            if missed > 0 {
                ring.missed = cursor - 1;
                if let Some(batches) = &mut ring.batches {
                    batches.miss(missed);
                }
            }
            evicted = if ring.rows.len() == ring.capacity {
                let (lost, _, stamp, row) = ring.rows.pop_front().expect("full ring");
                ring.evicted = lost;
                ring.floor = stamp;
                ring.standing = true;
                Some(row)
            } else {
                None
            };
            if let Some(batches) = &mut ring.batches {
                batches.publish(row.as_ref());
            }
            if let Some(publication) = row.downcast_ref::<super::ManagedPublication>() {
                publication.assign_sequence(next);
            }
            ring.rows.push_back((cursor, next, at, row));
            ring.next = next;
            ring.last = cursor;
        }
        Ok(evicted)
    }

    // Pool admission is serialized, and the caller owns facility visibility.
    // Preflight the whole prefix so a refused row changes no retention or gaps.
    pub(super) fn can_advance(&self, count: u64) -> bool {
        self.sequence
            .load(Ordering::Relaxed)
            .checked_add(count)
            .is_some()
            && self
                .ring
                .read()
                .expect("the ring is never poisoned")
                .next
                .checked_add(count)
                .is_some()
    }

    // Rows past `taken` are not in every feed's hands yet and stay retained.
    pub(super) fn reclaim(
        &self,
        needed_bytes: usize,
        needed_rows: usize,
        taken: u64,
    ) -> Option<()> {
        let mut ring = self.ring.write().expect("the ring is never poisoned");
        let mut bytes = 0usize;
        let mut rows = 0usize;
        let mut prefix = 0usize;
        for (cursor, _, _, row) in &ring.rows {
            if *cursor > taken {
                break;
            }
            let publication = row.downcast_ref::<super::ManagedPublication>()?;
            prefix += 1;
            if Arc::strong_count(row) == 1
                && let Some(charge) = publication.reclaimable_bytes()
            {
                bytes = bytes.saturating_add(charge);
                rows += 1;
            }
            if bytes >= needed_bytes && rows >= needed_rows {
                break;
            }
        }
        if bytes < needed_bytes || rows < needed_rows {
            return None;
        }
        for _ in 0..prefix {
            let (cursor, _, position, row) =
                ring.rows.pop_front().expect("validated retention prefix");
            ring.evicted = cursor;
            ring.floor = position;
            ring.standing = true;
            // Only unshared ManagedPublication owners release credit here;
            // no application destructor or pending receipt sender is invoked.
            drop(row);
        }
        Some(())
    }

    // Appending one row would evict a row some live feed has not taken in.
    pub(super) fn evicts_unread(&self, taken: u64) -> bool {
        let ring = self.ring.read().expect("the ring is never poisoned");
        ring.rows.len() == ring.capacity
            && ring
                .rows
                .front()
                .is_some_and(|(cursor, ..)| *cursor > taken)
    }

    // Rows the window still takes without evicting one past `taken`.
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn room(&self, taken: u64) -> usize {
        let ring = self.ring.read().expect("the ring is never poisoned");
        let read = ring.rows.partition_point(|(cursor, ..)| *cursor <= taken);
        ring.capacity - (ring.rows.len() - read)
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn miss(&self, count: u64) {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        if count == 0 {
            return;
        }

        let published;
        {
            let mut ring = self.ring.write().expect("the ring is never poisoned");
            let next = ring
                .next
                .checked_add(count)
                .expect("native stream cursor exhausted");
            let previous = self
                .sequence
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |cursor| {
                    cursor.checked_add(count)
                })
                .expect("native stream cursor exhausted");
            let cursor = previous + count;
            if let Some(batches) = &mut ring.batches {
                batches.miss(count);
            }
            ring.missed = cursor;
            ring.next = next;
            ring.last = cursor;
            published = ring.next;
        }
        drop(_visibility);
        self.doorbell.send_replace(published);
        self.pump.send_modify(|()| {});
    }

    pub(crate) fn stands_on(&self, floor: Position) {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let mut ring = self.ring.write().expect("the ring is never poisoned");

        ring.floor = floor;
        ring.standing = true;
    }

    pub(crate) fn snapshot(
        &self,
        start: Bound<Position>,
        end: Bound<Position>,
        as_of: Option<Position>,
    ) -> Retained {
        self.snapshot_scope(start, end, as_of, true)
    }

    pub(crate) fn snapshot_scope(
        &self,
        start: Bound<Position>,
        end: Bound<Position>,
        as_of: Option<Position>,
        history: bool,
    ) -> Retained {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let ring = self.ring.read().expect("the ring is never poisoned");
        let beneath = history && ring.standing && reaches_beneath(start, ring.floor);

        Retained {
            rows: ring
                .rows
                .iter()
                .filter(|(_, _, stamp, _)| !beneath || *stamp > ring.floor)
                .filter(|(_, _, stamp, _)| admitted(start, end, *stamp))
                .filter(|(_, _, stamp, _)| as_of.is_none_or(|cutoff| *stamp <= cutoff))
                .map(|(_, _, stamp, row)| (*stamp, Arc::clone(row)))
                .collect(),
            floor: ring.floor,
            beneath,
        }
    }

    pub(crate) fn reader<S: serde::Serialize + 'static>(
        &self,
        start: Bound<Position>,
        end: Bound<Position>,
        limits: super::ReadLimits,
    ) -> Result<Box<dyn super::RowSource>, super::ReadError> {
        struct Frozen {
            cut: super::ReadCut,
            rows: std::collections::vec_deque::IntoIter<(Position, Row)>,
        }
        impl super::RowSource for Frozen {
            fn cut(&self) -> super::ReadCut {
                self.cut
            }
            fn next(
                &mut self,
                cancelled: &AtomicBool,
            ) -> Result<Option<crate::state::At<Row>>, super::ReadError> {
                if cancelled.load(Ordering::Acquire) {
                    return Err(super::ReadError::Closed);
                }
                Ok(self
                    .rows
                    .next()
                    .map(|(position, value)| crate::state::At { position, value }))
            }
        }
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let ring = self.ring.read().expect("the ring is never poisoned");
        if ring.missed > 0 {
            return Err(super::ReadError::IncompleteCapture);
        }
        if ring.standing && reaches_beneath(start, ring.floor) {
            return Err(super::ReadError::Retained { floor: ring.floor });
        }
        let mut bytes = 0usize;
        let mut rows = VecDeque::new();
        for (scanned, (_, _, position, row)) in ring.rows.iter().enumerate() {
            if scanned == limits.rows {
                return Err(super::ReadError::Limit("window row scan"));
            }
            if !admitted(start, end, *position) {
                continue;
            }
            let typed = super::row_value::<S>(row.as_ref())
                .ok_or_else(|| super::ReadError::Unreadable("window row type mismatch".into()))?;
            bytes = bytes
                .checked_add(super::reader::encoded_size(typed, limits.row_bytes)?)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<(Position, Row)>()))
                .filter(|bytes| *bytes <= limits.pinned_bytes)
                .ok_or(super::ReadError::Limit("pinned bytes"))?;
            rows.push_back((*position, Arc::clone(row)));
        }
        Ok(Box::new(Frozen {
            cut: super::ReadCut::Window {
                delivered: crate::storage::Cursor(ring.last),
            },
            rows: rows.into_iter(),
        }))
    }

    pub(crate) fn since(&self, frontier: u64) -> Advanced {
        let _visibility = self.visibility.read().expect("stream visibility poisoned");
        let ring = self.ring.read().expect("the ring is never poisoned");
        let rows: Vec<_> = ring
            .rows
            .iter()
            .filter(|(_, sequence, ..)| *sequence > frontier)
            .map(|(_, _, stamp, row)| (*stamp, Arc::clone(row)))
            .collect();
        let missed = ring
            .next
            .saturating_sub(frontier)
            .saturating_sub(rows.len() as u64);
        Advanced {
            missed,
            rows,
            frontier: ring.next.max(frontier),
        }
    }

    pub(super) fn latest_locked(&self) -> u64 {
        self.ring.read().expect("the ring is never poisoned").last
    }

    pub(super) fn recorded_after_locked(
        &self,
        resume: u64,
        reaches: u64,
    ) -> (Vec<(u64, Position, Row)>, bool) {
        let ring = self.ring.read().expect("the ring is never poisoned");
        let rows = ring
            .rows
            .iter()
            .filter(|(cursor, ..)| *cursor > resume && *cursor <= reaches)
            .map(|(cursor, _, stamp, row)| (*cursor, *stamp, Arc::clone(row)))
            .collect();
        // Under this same lock: evicting a row after cloning it cannot later
        // turn this delivery into a gap. A cut overtaken before this lock is
        // conservatively incomplete rather than falsely certified.
        (rows, ring.evicted > resume || ring.missed > resume)
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn listener_candidates(
        &self,
        after: u64,
        through: u64,
        source: usize,
        limit: usize,
        selected: &mut Vec<ListenerCandidate>,
    ) {
        let ring = self.ring.read().expect("the ring is never poisoned");
        let mut lower = 0;
        let mut upper = ring.rows.len();
        while lower < upper {
            let mid = lower + (upper - lower) / 2;
            if ring.rows[mid].0 <= after {
                lower = mid + 1;
            } else {
                upper = mid;
            }
        }
        for (cursor, sequence, ..) in ring.rows.iter().skip(lower).take(limit) {
            if *cursor > through
                || selected.len() == limit && *cursor >= selected.last().unwrap().cursor
            {
                break;
            }
            let index = selected.partition_point(|candidate| candidate.cursor < *cursor);
            if selected.len() == limit {
                selected.pop();
            }
            selected.insert(
                index,
                ListenerCandidate {
                    cursor: *cursor,
                    sequence: *sequence,
                    source,
                },
            );
        }
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn listener_row(&self, cursor: u64) -> Option<Row> {
        let ring = self.ring.read().expect("the ring is never poisoned");
        let mut lower = 0;
        let mut upper = ring.rows.len();
        while lower < upper {
            let mid = lower + (upper - lower) / 2;
            if ring.rows[mid].0 < cursor {
                lower = mid + 1;
            } else {
                upper = mid;
            }
        }
        ring.rows
            .get(lower)
            .filter(|row| row.0 == cursor)
            .map(|row| Arc::clone(&row.3))
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn logical_frontier(&self) -> u64 {
        self.ring.read().expect("the ring is never poisoned").next
    }

    pub(super) fn wake_delivery(&self) {
        self.pump.send_modify(|()| {});
    }

    pub(crate) fn knock(&self) {
        self.doorbell.send_modify(|_| {});
    }

    pub(crate) fn interest(&self) -> watch::Receiver<usize> {
        self.demand.subscribe()
    }

    pub(crate) fn engaged(&self) {
        self.demand.send_modify(|count| *count += 1);
    }

    pub(crate) fn released(&self) {
        self.demand.send_modify(|count| *count -= 1);
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Window").finish_non_exhaustive()
    }
}

impl<S> Unpin for Following<S> {}

impl<S: Clone + 'static> futures_core::Stream for Following<S> {
    type Item = Item<S>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let following = self.get_mut();

        loop {
            if following.gap > 0 {
                let missed = std::mem::take(&mut following.gap);
                return Poll::Ready(Some(Item::Gap { missed }));
            }
            if let Some((position, row)) = following.pending.pop_front() {
                let value = super::row_value::<S>(row.as_ref())
                    .expect("a window holds only its own stream's rows")
                    .clone();
                return Poll::Ready(Some(Item::Row(At { value, position })));
            }
            if following.done {
                return Poll::Ready(None);
            }

            let mut step = match following.step.take() {
                Some(step) => step,
                None => match following.parts.take() {
                    Some(parts) => Box::pin(turn(parts)),
                    None => {
                        following.done = true;
                        return Poll::Ready(None);
                    }
                },
            };

            match step.as_mut().poll(context) {
                Poll::Pending => {
                    following.step = Some(step);
                    return Poll::Pending;
                }
                Poll::Ready((parts, Turn::Advanced(advanced))) => {
                    following.parts = Some(parts);
                    following.gap = advanced.missed;
                    following.pending = advanced.rows.into();
                }
                Poll::Ready((parts, Turn::Ended)) => {
                    following.parts = Some(parts);
                    following.done = true;
                }
            }
        }
    }
}

/* === Public Functions === */
pub(crate) fn following<S: Clone + 'static>(
    window: Arc<Window>,
    closed: Arc<AtomicBool>,
    held: Holding,
) -> impl futures_core::Stream<Item = Item<S>> + Send + Unpin {
    let doorbell = window.doorbell.subscribe();
    let frontier = {
        let _visibility = window
            .visibility
            .read()
            .expect("stream visibility poisoned");
        window.ring.read().expect("the ring is never poisoned").next
    };

    Following {
        parts: Some(Parts {
            window,
            doorbell,
            frontier,
            closed,
            held,
        }),
        step: None,
        gap: 0,
        pending: VecDeque::new(),
        done: false,
        marker: PhantomData,
    }
}

/* === Private Functions === */
fn reaches_beneath(start: Bound<Position>, floor: Position) -> bool {
    match start {
        Bound::Included(at) => at <= floor,
        Bound::Excluded(at) => at < floor,
        Bound::Unbounded => true,
    }
}

fn admitted(start: Bound<Position>, end: Bound<Position>, at: Position) -> bool {
    let above = match start {
        Bound::Included(from) => at >= from,
        Bound::Excluded(from) => at > from,
        Bound::Unbounded => true,
    };
    let below = match end {
        Bound::Included(to) => at <= to,
        Bound::Excluded(to) => at < to,
        Bound::Unbounded => true,
    };

    above && below
}

async fn turn(mut parts: Parts) -> (Parts, Turn) {
    loop {
        let was_closed = parts.closed.load(Ordering::Acquire);
        let advanced = parts.window.since(parts.frontier);
        parts.frontier = advanced.frontier;

        if advanced.missed > 0 || !advanced.rows.is_empty() {
            return (parts, Turn::Advanced(advanced));
        }
        if was_closed || parts.doorbell.changed().await.is_err() {
            return (parts, Turn::Ended);
        }
    }
}

/* === Tests === */
