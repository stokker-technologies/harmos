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
use std::any::TypeId;
use std::fmt;
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::ops::{Bound, RangeBounds};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::window::Window;
use super::{BeneathWindow, Demand, Item, Stream, Table, Wired, demand, window};
use crate::{Application, state::At, state::Position, storage::Batch, storage::Cursor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StreamManager<A: Application> {
    pub(super) schema_settings: Arc<crate::config::SchemaSettings>,
    pub(super) windows: Arc<Table>,
    pub(super) applied: watch::Receiver<Position>,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) marker: PhantomData<fn() -> A>,
}

// A live feed paces plugin publication: the window never evicts a row this
// feed has not taken in. Asking for the next batch takes in the previous one.
pub struct StreamFeed<A: Application> {
    pub(super) streams: StreamManager<A>,
    pub(super) changed: watch::Receiver<()>,
    pub(super) cursor: Cursor,
    pub(super) intake: super::ingest::Intake,
}

pub struct Query<S> {
    pub(super) window: Arc<Window>,
    pub(super) start: Bound<Position>,
    pub(super) end: Bound<Position>,
    pub(super) as_of: Option<Position>,
    pub(super) retained_only: bool,
    pub(super) marker: PhantomData<fn() -> S>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> StreamFeed<A> {
    pub async fn next(&mut self) -> Option<Batch<A>> {
        self.intake.took(self.cursor.0);
        loop {
            self.changed.borrow_and_update();
            let batch = self.streams.batch(self.cursor);
            if batch.reaches() > self.cursor {
                self.cursor = batch.reaches();
                return Some(batch);
            }
            if self.streams.closed.load(Ordering::Acquire) {
                return None;
            }
            if self.changed.changed().await.is_err() {
                return None;
            }
        }
    }
}

impl<A: Application> StreamManager<A> {
    pub(crate) fn wired(windows: Table, applied: watch::Receiver<Position>, schema_settings: Arc<crate::config::SchemaSettings>) -> Self {
        Self {
            schema_settings,
            windows: Arc::new(windows),
            applied,
            closed: Arc::new(AtomicBool::new(false)),
            marker: PhantomData,
        }
    }

    pub fn follow(&self, resume: Cursor) -> StreamFeed<A> {
        StreamFeed {
            streams: self.clone(),
            changed: self.delivery(),
            cursor: resume,
            intake: self.windows.ingest.engage(resume.0),
        }
    }

    pub fn frontier(&self) -> Cursor {
        self.windows.frontier()
    }

    pub fn publish<S: Stream<A>>(&self, row: S) {
        let at = *self.applied.borrow();
        self.stream::<S>().window.publish(at, Arc::new(row));
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn miss<S: Stream<A>>(&self, count: u64) {
        self.stream::<S>().window.miss(count);
    }

    pub fn query<S: Stream<A>>(&self, range: impl RangeBounds<Position>) -> Query<S> {
        let wired = self.stream::<S>();

        Query {
            window: Arc::clone(&wired.window),
            start: range.start_bound().cloned(),
            end: range.end_bound().cloned(),
            as_of: None,
            retained_only: false,
            marker: PhantomData,
        }
    }

    pub fn watch<S: Stream<A>>(&self) -> impl futures_core::Stream<Item = Item<S>> + Send + Unpin + use<S, A> {
        let window = Arc::clone(&self.stream::<S>().window);
        let held = demand::Holding::engage(Arc::clone(&window));

        window::following(window, Arc::clone(&self.closed), held)
    }

    pub fn demand<S: Stream<A>>(&self) -> Demand {
        Demand::of(&self.stream::<S>().window)
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.windows.wake();
        self.windows.ingest.wake();
    }

    pub(crate) fn drained(&self, cursor: u64) {
        self.windows.drained(cursor);
    }

    pub(crate) fn delivery(&self) -> watch::Receiver<()> {
        self.windows.delivery()
    }

    pub(crate) fn batch(&self, resume: Cursor) -> Batch<A> {
        let delivery = self.windows.recorded_after(resume);

        Batch {
            rows: delivery.rows,
            incomplete: delivery.incomplete,
            reaches: delivery.reaches,
            application: PhantomData,
        }
    }

    fn stream<S: Stream<A>>(&self) -> Arc<Wired> {
        self.windows
            .current(S::ID)
            .filter(|wired| wired.definition.row() == TypeId::of::<S>() && wired.definition.owner().is_none())
            .unwrap_or_else(|| {
                panic!(
                    "the stream \"{}\" is not in the registered catalog; \
                     register its definition with builder.register(catalog)",
                    S::ID
                )
            })
    }
}

impl<A: Application> Clone for StreamManager<A> {
    fn clone(&self) -> Self {
        Self {
            schema_settings: Arc::clone(&self.schema_settings),
            windows: Arc::clone(&self.windows),
            applied: self.applied.clone(),
            closed: Arc::clone(&self.closed),
            marker: PhantomData,
        }
    }
}

impl<A: Application> fmt::Debug for StreamManager<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("StreamManager").finish_non_exhaustive()
    }
}

impl<S: Clone + Send + 'static> Query<S> {
    pub fn retained(mut self) -> Self {
        self.retained_only = true;
        self
    }

    pub fn as_of(mut self, at: Position) -> Self {
        self.as_of = Some(at);
        self
    }

    async fn answer(self) -> Result<std::vec::IntoIter<At<S>>, BeneathWindow> {
        let retained = if self.retained_only {
            self.window.snapshot_scope(self.start, self.end, self.as_of, false)
        } else {
            self.window.snapshot(self.start, self.end, self.as_of)
        };
        if retained.beneath && !self.retained_only {
            return Err(BeneathWindow::Floor { floor: retained.floor });
        }

        Ok(retained
            .rows
            .into_iter()
            .map(|(position, row)| At {
                value: super::row_value::<S>(row.as_ref()).expect("a stream window holds only its own rows").clone(),
                position,
            })
            .collect::<Vec<_>>()
            .into_iter())
    }
}

impl<S: Clone + Send + 'static> IntoFuture for Query<S> {
    type Output = Result<std::vec::IntoIter<At<S>>, BeneathWindow>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.answer())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
