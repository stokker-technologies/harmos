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
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
// Third Party Imports
use harmos_domain::Aggregate;
use harmos_storage::signal::Summary as Running;
use tokio::sync::watch;
// Local Imports
use super::demand::Holding;
use crate::aggregate;
use crate::streams::BatchDefinition;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Next = Pin<Box<dyn Future<Output = (Cursor, Option<BatchItem>)> + Send>>;

/* === Structs === */

// One measured field of a batch. `statistics` is the field's running
// summary, absent when no finite value arrived or a statistic overflowed.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub field: &'static str,
    pub statistics: Option<Running>,
    pub invalid: u64,
    pub overflow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub count: u64,
    pub missed: u64,
    pub partial: bool,
    pub fields: Vec<Measurement>,
}

pub struct Batches<S> {
    cursor: Option<Cursor>,
    next: Option<Next>,
    marker: PhantomData<fn() -> S>,
    _held: Holding,
}

struct Cursor {
    feed: Arc<Feed>,
    bell: watch::Receiver<()>,
    next: u64,
}

pub(crate) struct Feed {
    ring: Mutex<Retained>,
    bell: watch::Sender<()>,
}

struct Retained {
    rows: VecDeque<Arc<Summary>>,
    next: u64,
    closed: bool,
}

pub(crate) struct State {
    definition: BatchDefinition,
    count: u64,
    missed: u64,
    fields: Vec<Accumulator>,
    pub(crate) feed: Arc<Feed>,
    closed: bool,
}

#[derive(Clone, Default)]
struct Accumulator {
    running: Running,
    invalid: u64,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq)]
pub enum BatchItem {
    Summary(Arc<Summary>),
    Gap { missed: u64 },
}

/* === Traits === */

/* === Implementations === */

impl Summary {
    pub fn complete(&self) -> bool {
        !self.partial && self.missed == 0
    }
}

impl<S> Batches<S> {
    pub async fn next(&mut self) -> Option<BatchItem> {
        std::future::poll_fn(|cx| futures_core::Stream::poll_next(Pin::new(&mut *self), cx)).await
    }
}

impl Cursor {
    async fn advance(mut self) -> (Self, Option<BatchItem>) {
        loop {
            let (item, closed) = {
                let ring = self.feed.ring.lock().expect("summary ring is healthy");
                let floor = ring.next - ring.rows.len() as u64;
                let item = if self.next < floor {
                    let missed = floor - self.next;
                    self.next = floor;
                    Some(BatchItem::Gap { missed })
                } else if self.next < ring.next {
                    let row = Arc::clone(&ring.rows[(self.next - floor) as usize]);
                    self.next += 1;
                    Some(BatchItem::Summary(row))
                } else {
                    None
                };
                (item, ring.closed)
            };
            if item.is_some() || closed {
                return (self, item);
            }
            if self.bell.changed().await.is_err() {
                return (self, None);
            }
        }
    }
}

impl<S> futures_core::Stream for Batches<S> {
    type Item = BatchItem;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.next.is_none() {
            let Some(cursor) = this.cursor.take() else {
                return Poll::Ready(None);
            };
            this.next = Some(Box::pin(cursor.advance()));
        }
        match this.next.as_mut().expect("advance installed").as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready((cursor, item)) => {
                this.next = None;
                if item.is_some() {
                    this.cursor = Some(cursor);
                }
                Poll::Ready(item)
            }
        }
    }
}

impl Feed {
    pub(crate) fn following<S>(self: &Arc<Self>, held: Holding) -> Batches<S> {
        let bell = self.bell.subscribe();
        let next = self.ring.lock().expect("summary ring is healthy").next;
        Batches { cursor: Some(Cursor { feed: Arc::clone(self), bell, next }), next: None, marker: PhantomData, _held: held }
    }
}

impl State {
    pub(crate) fn new(definition: BatchDefinition) -> Self {
        let (bell, _) = watch::channel(());
        Self {
            definition,
            count: 0,
            missed: 0,
            fields: vec![Accumulator::default(); definition.fields().len()],
            feed: Arc::new(Feed { ring: Mutex::new(Retained { rows: VecDeque::with_capacity(64), next: 0, closed: false }), bell }),
            closed: false,
        }
    }

    pub(crate) fn publish(&mut self, row: &dyn Any) {
        if self.closed {
            return;
        }
        let mut fields = self.fields.iter_mut();
        self.definition.measure(row, &mut |value| fields.next().expect("one accumulator per declared measure").push(value));
        self.count += 1;
        if self.count == self.definition.samples() {
            self.flush(false);
        }
    }

    pub(crate) fn miss(&mut self, count: u64) {
        if !self.closed {
            self.missed = self.missed.saturating_add(count);
        }
    }

    fn flush(&mut self, partial: bool) {
        let summary = Summary {
            count: self.count,
            missed: self.missed,
            partial,
            fields: self
                .fields
                .iter_mut()
                .zip(self.definition.fields())
                .map(|(accumulator, field)| {
                    let result = accumulator.finish(field);
                    *accumulator = Accumulator::default();
                    result
                })
                .collect(),
        };
        self.count = 0;
        self.missed = 0;
        let mut ring = self.feed.ring.lock().expect("summary ring is healthy");
        if ring.rows.len() == 64 {
            ring.rows.pop_front();
        }
        ring.rows.push_back(Arc::new(summary));
        ring.next += 1;
        drop(ring);
        self.feed.bell.send_replace(());
    }

    pub(crate) fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if self.count > 0 || self.missed > 0 {
            self.flush(true);
        }
        self.feed.ring.lock().expect("summary ring is healthy").closed = true;
        self.feed.bell.send_replace(());
    }
}

impl Measurement {
    // Mergeable members only, as `Running` answers them; a batch keeps no
    // values, so median and percentiles answer `None`.
    pub fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_summary(self.statistics.as_ref()?, kind)
    }
}

impl Accumulator {
    fn push(&mut self, value: f64) {
        if value.is_finite() {
            self.running.push(value);
        } else {
            self.invalid += 1;
        }
    }

    fn finish(&self, field: &'static str) -> Measurement {
        let running = self.running;
        let overflow = !running.is_empty()
            && [Some(running.sum()), running.mean(), running.variance(), running.rms()]
                .into_iter()
                .any(|statistic| !statistic.is_some_and(f64::is_finite));
        let statistics = (!running.is_empty() && !overflow).then_some(running);
        Measurement { field, statistics, invalid: self.invalid, overflow }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
