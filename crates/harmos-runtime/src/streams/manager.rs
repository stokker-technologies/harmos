/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::TypeId;
use std::fmt;
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::ops::{Bound, RangeBounds};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use harmos_domain::Aggregate;
use tokio::sync::watch;

use super::window::Window;
use super::{
    Aggregated, Axial, Batched, Batches, BeneathWindow, Bucket, Definition, Demand, Holding, Item, Prefix, Stream, Table, Tier, Tracked,
    Wired, demand, window,
};
use crate::{Application, state::At, state::Position, storage::Batch, storage::Cursor};

pub struct StreamManager<A: Application> {
    pub(super) schema_settings: Arc<crate::config::SchemaSettings>,
    pub(super) windows: Arc<Table>,
    pub(super) applied: watch::Receiver<Position>,
    pub(super) tier: Tier,
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

pub(super) struct Partition<S> {
    pub(super) labels: Vec<String>,
    pub(super) matches: Box<dyn Fn(&S) -> bool + Send>,
}

pub struct Query<S> {
    pub(super) window: Arc<Window>,
    pub(super) definition: Definition,
    pub(super) tier: Tier,
    pub(super) start: Bound<Position>,
    pub(super) end: Bound<Position>,
    pub(super) as_of: Option<Position>,
    pub(super) partition: Option<Partition<S>>,
    pub(super) retained_only: bool,
    pub(super) marker: PhantomData<fn() -> S>,
}

pub struct Survey<S> {
    pub(super) definition: Definition,
    pub(super) tier: Tier,
    pub(super) start: Bound<u64>,
    pub(super) end: Bound<u64>,
    pub(super) track: Vec<String>,
    pub(super) marker: PhantomData<fn() -> S>,
}

pub struct Seal<S> {
    pub(super) definition: Definition,
    pub(super) tier: Tier,
    pub(super) track: Vec<String>,
    pub(super) marker: PhantomData<fn() -> S>,
}

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
    pub(crate) fn wired(
        windows: Table, applied: watch::Receiver<Position>, tier: Tier, schema_settings: Arc<crate::config::SchemaSettings>,
    ) -> Self {
        Self {
            schema_settings,
            windows: Arc::new(windows),
            applied,
            tier,
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
            definition: wired.definition.clone(),
            tier: self.tier.clone(),
            start: range.start_bound().cloned(),
            end: range.end_bound().cloned(),
            as_of: None,
            partition: None,
            retained_only: false,
            marker: PhantomData,
        }
    }

    pub fn survey<S: Stream<A> + Axial>(&self, range: impl RangeBounds<u64>) -> Survey<S> {
        Survey {
            definition: self.stream::<S>().definition.clone(),
            tier: self.tier.clone(),
            start: range.start_bound().cloned(),
            end: range.end_bound().cloned(),
            track: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn seal<S: Stream<A>>(&self) -> Seal<S> {
        Seal {
            definition: self.stream::<S>().definition.clone(),
            tier: self.tier.clone(),
            track: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn watch<S: Stream<A>>(&self) -> impl futures_core::Stream<Item = Item<S>> + Send + Unpin + use<S, A> {
        let window = Arc::clone(&self.stream::<S>().window);
        let held = demand::Holding::engage(Arc::clone(&window));

        window::following(window, Arc::clone(&self.closed), held)
    }

    pub fn batches<S: Stream<A> + Batched>(&self) -> Batches<S> {
        let window = &self.stream::<S>().window;
        window.batches::<S>(Holding::engage(Arc::clone(window)))
    }

    pub fn demand<S: Stream<A>>(&self) -> Demand {
        Demand::of(&self.stream::<S>().window)
    }

    pub(crate) fn close(&self) {
        for wired in self.windows.declared() {
            wired.window.close_batches();
        }
        self.closed.store(true, Ordering::Release);
        self.windows.wake();
        self.windows.ingest.wake();
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

    pub(crate) fn declared(&self) -> Vec<Arc<Wired>> {
        self.windows.declared()
    }

    fn stream<S: Stream<A>>(&self) -> Arc<Wired> {
        self.windows
            .wired(S::ID, S::VERSION)
            .filter(|wired| wired.definition.row() == TypeId::of::<S>() && wired.definition.version() == S::VERSION)
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
            tier: self.tier.clone(),
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
        let Self {
            window,
            definition,
            tier,
            start,
            end,
            as_of,
            partition,
            retained_only,
            ..
        } = self;
        let retained = if retained_only { window.snapshot_scope(start, end, as_of, false) } else { window.snapshot(start, end, as_of) };
        let mut rows = Vec::new();

        if retained.beneath && !retained_only {
            let served =
                tier.rows(definition, narrowed(partition.as_ref()), start, beneath(beneath_of(end, as_of), retained.floor)).await?;

            rows.extend(served.into_iter().map(|At { value, position }| (position, value)));
        }
        rows.extend(retained.rows);

        Ok(rows
            .into_iter()
            .map(|(position, row)| At {
                value: super::row_value::<S>(row.as_ref()).expect("a stream's tiers hold only its own rows").clone(),
                position,
            })
            .filter(|At { value, .. }| partition.as_ref().is_none_or(|partition| (partition.matches)(value)))
            .collect::<Vec<_>>()
            .into_iter())
    }
}

impl<S: Clone + Send + Tracked + 'static> Query<S> {
    pub fn track<K: Prefix<S::Track>>(mut self, track: K) -> Self {
        self.partition = Some(Partition {
            labels: track.labels(),
            matches: Box::new(move |row: &S| track.selects(&row.track())),
        });
        self
    }
}

impl<S: Clone + Send + 'static> IntoFuture for Query<S> {
    type Output = Result<std::vec::IntoIter<At<S>>, BeneathWindow>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.answer())
    }
}

impl<S> Survey<S> {
    pub async fn budget(self, points: usize) -> Result<Vec<Bucket>, BeneathWindow> {
        self.tier.buckets(self.definition, self.track, self.start, self.end, points).await
    }

    pub async fn fold(self, kind: Aggregate) -> Result<Vec<Aggregated>, BeneathWindow> {
        self.tier.aggregate(self.definition, self.track, self.start, self.end, checked(kind)).await
    }
}

impl<S: Tracked> Survey<S> {
    pub fn track<K: Prefix<S::Track>>(mut self, track: K) -> Self {
        self.track = track.labels();
        self
    }
}

impl<S: Tracked> Seal<S> {
    pub fn track<K: Prefix<S::Track>>(mut self, track: K) -> Self {
        self.track = track.labels();
        self
    }
}

impl<S> IntoFuture for Seal<S> {
    type Output = Position;
    type IntoFuture = Pin<Box<dyn Future<Output = Position> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.tier.seal(self.definition, self.track).await })
    }
}

fn checked(kind: Aggregate) -> Aggregate {
    assert!(kind.answerable(), "a percentile rank is a percentage, and {kind:?} names none");
    kind
}

fn narrowed<S>(partition: Option<&Partition<S>>) -> Vec<String> {
    partition.map(|partition| partition.labels.clone()).unwrap_or_default()
}

fn beneath(end: Bound<Position>, ceiling: Position) -> Bound<Position> {
    match end {
        Bound::Included(at) | Bound::Excluded(at) if at <= ceiling => end,
        _ => Bound::Included(ceiling),
    }
}

fn beneath_of(end: Bound<Position>, as_of: Option<Position>) -> Bound<Position> {
    match as_of {
        Some(cutoff) => beneath(end, cutoff),
        None => end,
    }
}
