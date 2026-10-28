/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::Instrument;

use super::TrackedState;
use super::entry::{At, Entry, Link, Position, PrincipalId};
use super::envelope::{Envelope, Recorded};
use super::transaction::dispatch::Dispatch;
use super::transaction::{DecodeError, Inverting};
use crate::Application;
use crate::error::Error;
use crate::supervision::executor::InContext;

const SHARED_PAGE_BYTES: usize = 8 * 1024 * 1024;

type Predicate<A> = Arc<dyn Fn(&<A as Application>::Transaction) -> bool + Send + Sync>;

type Step<A, X> = Pin<Box<dyn Future<Output = (Parts<A>, Result<Vec<Entry<X>>, Error>)> + Send>>;

pub struct Origin<S> {
    pub value: S,
    pub position: Position,
}

pub(crate) struct Reads<A: Application> {
    sender: mpsc::Sender<ReadRequest<A>>,
}

pub struct Focus<A: Application> {
    predicate: Option<Predicate<A>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reversals {
    pub undoable: bool,
    pub redoable: bool,
}

pub(crate) struct Undoable<A: Application> {
    pub(crate) principal: PrincipalId,
    pub(crate) forward: crate::operations::Response<A::Transaction>,
    pub(crate) inverse: Option<crate::operations::Response<A::Transaction>>,
    pub(crate) undone: Option<Position>,
}

pub(crate) struct Undos<A: Application> {
    pub(crate) entries: BTreeMap<Position, Undoable<A>>,
}

pub(crate) struct Tail<A: Application> {
    pub(crate) entries: VecDeque<Arc<Entry<Recorded<A>>>>,
    capacity: usize,
    floor: Position,
}

pub(crate) struct SharedPage<A: Application> {
    pub(crate) entries: Vec<Arc<Entry<Recorded<A>>>>,
    pub(crate) reaches: Position,
}

struct Parts<A: Application> {
    trace: crate::supervision::executor::SpanContext,
    tracked: Arc<TrackedState<A>>,
    frontier: Position,
    doorbell: watch::Receiver<Position>,
}

struct Watching<A: Application, X> {
    parts: Option<Parts<A>>,
    step: Option<Step<A, X>>,
    pending: VecDeque<Entry<X>>,
    done: bool,
}

pub(crate) enum ReadRequest<A: Application> {
    Entries {
        trace: crate::supervision::executor::SpanContext,
        after: Position,
        through: Position,
        reply: oneshot::Sender<Result<Vec<Entry<Recorded<A>>>, Error>>,
    },
    State {
        trace: crate::supervision::executor::SpanContext,
        origin: Origin<A::State>,
        at: Position,
        reply: oneshot::Sender<Result<At<A::State>, Error>>,
    },
}

pub(crate) trait ReplayPayload<A: Application>: Sized {
    fn prepared(&self) -> Option<crate::state::Prepared> {
        None
    }

    fn forward(&self) -> Option<crate::operations::Response<A::Transaction>>;
    fn restore(
        envelope: Envelope<Self>, catalog: Option<&Dispatch<A, Self>>, state: &mut A::State,
    ) -> Result<Option<Recorded<A>>, DecodeError>;
}

impl<S> Origin<S> {
    pub fn initial(value: S) -> Self {
        Self { value, position: Position::ORIGIN }
    }

    pub fn at(value: S, position: Position) -> Self {
        Self { value, position }
    }
}

impl<S> From<S> for Origin<S> {
    fn from(value: S) -> Self {
        Self::initial(value)
    }
}

impl<S> From<At<S>> for Origin<S> {
    fn from(origin: At<S>) -> Self {
        Self::at(origin.value, origin.position)
    }
}

impl<A: Application> Reads<A> {
    pub(crate) fn channel(capacity: usize) -> (Self, mpsc::Receiver<ReadRequest<A>>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));

        (Self { sender }, receiver)
    }

    #[tracing::instrument(target = "harmos::state", name = "state.entries", level = "debug", skip_all)]
    pub(crate) async fn entries(&self, after: Position, through: Position) -> Result<Vec<Entry<Recorded<A>>>, Error> {
        let (reply, answer) = oneshot::channel();
        self.sender
            .send(ReadRequest::Entries {
                after,
                through,
                reply,
                trace: crate::supervision::executor::SpanContext::new(tracing::Span::current(), tracing::Span::none()),
            })
            .await
            .map_err(|_| Error::BeyondTail)?;

        answer.await.map_err(|_| Error::Unavailable)?
    }

    #[tracing::instrument(target = "harmos::state", name = "state.state", level = "debug", skip_all)]
    pub(crate) async fn state(&self, origin: Origin<A::State>, at: Position) -> Result<At<A::State>, Error> {
        if origin.position > at {
            return Err(Error::OriginAhead);
        }
        if origin.position == at {
            return Ok(At { value: origin.value, position: at });
        }

        let (reply, answer) = oneshot::channel();
        self.sender
            .send(ReadRequest::State {
                origin,
                at,
                reply,
                trace: crate::supervision::executor::SpanContext::new(tracing::Span::current(), tracing::Span::none()),
            })
            .await
            .map_err(|_| Error::BeyondTail)?;

        answer.await.map_err(|_| Error::Unavailable)?
    }
}

impl<A: Application> Clone for Reads<A> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}

impl<A: Application> Focus<A> {
    pub fn all() -> Self {
        Self { predicate: None }
    }

    pub fn matching(predicate: impl Fn(&A::Transaction) -> bool + Send + Sync + 'static) -> Self {
        Self { predicate: Some(Arc::new(predicate)) }
    }

    fn admits(&self, transaction: &A::Transaction) -> bool {
        self.predicate.as_ref().is_none_or(|predicate| predicate(transaction))
    }
}

impl<A: Application> Clone for Focus<A> {
    fn clone(&self) -> Self {
        Self { predicate: self.predicate.clone() }
    }
}

impl<A: Application> fmt::Debug for Focus<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.predicate {
            Some(_) => "Focus::matching(..)",
            None => "Focus::all()",
        })
    }
}

impl<A: Application> Undos<A> {
    pub(crate) fn record(
        &mut self, position: Position, principal: Option<&PrincipalId>, link: Option<Link>,
        forward: &crate::operations::Response<A::Transaction>, inverse: Option<A::Transaction>,
    ) {
        match link {
            Some(Link::UndoOf(original)) => {
                if let Some(entry) = self.entries.get_mut(&original) {
                    entry.undone = Some(position);
                }
            }
            Some(Link::RedoOf(original)) => {
                if let Some(entry) = self.entries.get_mut(&original) {
                    entry.undone = None;
                    // The redo re-applied the change, so the inverse captured
                    // against the state it just left is the one that undoes it
                    // next time.
                    entry.inverse = inverse.map(crate::operations::Response::new);
                }
            }
            None => {
                let Some(principal) = principal else {
                    return;
                };
                // A new forward commit truncates this principal's redo branch:
                // the present has diverged from the future those entries would
                // restore, so redoing onto it would be a lie. The truncation is
                // personal — undo is per principal, and another principal's
                // redo is still guarded by the inverse's re-check on commit.
                self.entries.retain(|_, entry| entry.principal != *principal || entry.undone.is_none());
                self.entries.insert(
                    position,
                    Undoable {
                        principal: principal.clone(),
                        forward: forward.clone(),
                        inverse: inverse.map(crate::operations::Response::new),
                        undone: None,
                    },
                );
            }
        }
    }

    pub(crate) fn undoable(
        &self, principal: &PrincipalId, focus: &Focus<A>,
    ) -> Option<(Position, crate::operations::Response<A::Transaction>)> {
        for (position, entry) in self.entries.iter().rev() {
            if entry.principal != *principal {
                continue;
            }
            if entry.undone.is_some() || !focus.admits(&entry.forward) {
                continue;
            }
            let inverse = entry.inverse.as_ref()?;
            return Some((*position, inverse.clone()));
        }
        None
    }

    pub(crate) fn redoable(
        &self, principal: &PrincipalId, focus: &Focus<A>,
    ) -> Option<(Position, crate::operations::Response<A::Transaction>)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.principal == *principal && focus.admits(&entry.forward))
            .filter_map(|(position, entry)| entry.undone.map(|at| (at, *position, entry.forward.clone())))
            .max_by_key(|(at, _position, _forward)| *at)
            .map(|(_at, position, forward)| (position, forward))
    }

    pub(crate) fn reversals(&self, principal: &PrincipalId, focus: &Focus<A>) -> Reversals {
        Reversals {
            undoable: self.undoable(principal, focus).is_some(),
            redoable: self.redoable(principal, focus).is_some(),
        }
    }
}

impl<A: Application> Default for Undos<A> {
    fn default() -> Self {
        Self { entries: BTreeMap::new() }
    }
}

impl<A: Application> Tail<A> {
    pub(crate) fn new(origin: Position, capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
            floor: origin,
        }
    }

    pub(crate) fn recovered(frontier: Position, capacity: usize, entries: Vec<Arc<Entry<Recorded<A>>>>) -> Self {
        if entries.is_empty() {
            return Self::new(frontier, capacity);
        }
        let floor = entries.first().map_or(frontier, |entry| Position(entry.position.0.saturating_sub(1)));

        Self {
            entries: entries.into(),
            capacity: capacity.max(1),
            floor,
        }
    }

    pub(crate) fn push_group(&mut self, entries: Vec<Entry<Recorded<A>>>) -> Vec<Arc<Entry<Recorded<A>>>> {
        while self.entries.len() + entries.len() > self.capacity && !self.entries.is_empty() {
            let evicted = self.entries.pop_front().expect("nonempty tail");
            let frontier = evicted.metadata.commit.map_or(evicted.position, |(_, end)| end);
            self.floor = frontier;
            while self.entries.front().is_some_and(|entry| entry.position <= frontier) {
                self.entries.pop_front();
            }
        }
        let entries: Vec<_> = entries.into_iter().map(Arc::new).collect();
        self.entries.extend(entries.iter().cloned());
        entries
    }

    pub(crate) fn splits_commit(&self, position: Position) -> bool {
        self.entries.iter().any(|entry| entry.metadata.commit.is_some_and(|(first, end)| first <= position && position < end))
    }

    pub(crate) fn since(&self, after: Position) -> Result<Vec<Arc<Entry<Recorded<A>>>>, Error> {
        if after < self.floor {
            return Err(Error::BeyondTail);
        }

        Ok(self.entries.iter().filter(|entry| entry.position > after).map(Arc::clone).collect())
    }

    pub(crate) fn scan<X: Clone + 'static>(&self, after: Position) -> Result<(Vec<Entry<X>>, Position), Error> {
        self.scan_through(after, Position(u64::MAX))
    }

    pub(crate) fn scan_through<X: Clone + 'static>(&self, after: Position, through: Position) -> Result<(Vec<Entry<X>>, Position), Error> {
        let mut frontier = after;
        let mut found = Vec::new();
        loop {
            let page = self.shared_page(frontier, through, NonZeroUsize::new(1024).unwrap())?;
            if page.entries.is_empty() {
                break;
            }
            frontier = page.reaches;
            for entry in page.entries {
                if let Some(value) = entry.value.select::<X>() {
                    found.push(Entry {
                        prepared: None,
                        position: entry.position,
                        metadata: entry.metadata.clone(),
                        value,
                    });
                }
            }
        }

        Ok((found, frontier))
    }

    #[tracing::instrument(target = "harmos::state", name = "state.shared_page", level = "debug", skip_all)]
    pub(crate) fn shared_page(&self, after: Position, through: Position, max_rows: NonZeroUsize) -> Result<SharedPage<A>, Error> {
        self.shared_page_sized(after, through, max_rows, |_| 0)
    }

    pub(crate) fn shared_page_sized(
        &self, after: Position, through: Position, max_rows: NonZeroUsize, size: impl Fn(&A::Record) -> usize,
    ) -> Result<SharedPage<A>, Error> {
        if after < self.floor {
            return Err(Error::BeyondTail);
        }
        let start = self.entries.partition_point(|entry| entry.position <= after);
        let count = max_rows.get().min(self.entries.len() - start);
        let mut entries = Vec::with_capacity(count);
        let mut reaches = after;
        let mut bytes = 0usize;
        for entry in self.entries.iter().skip(start).take(count) {
            if entry.position > through {
                break;
            }
            if let Recorded::Record(record) = &entry.value {
                let count = size(record);
                if !entries.is_empty() && count > SHARED_PAGE_BYTES.saturating_sub(bytes) {
                    break;
                }
                bytes = bytes.saturating_add(count);
            }
            reaches = entry.position;
            entries.push(Arc::clone(entry));
        }
        Ok(SharedPage { entries, reaches })
    }
}

impl<A: Application, X> Unpin for Watching<A, X> {}

impl<A, X> Stream for Watching<A, X>
where
    A: Application,
    X: Clone + Send + 'static,
{
    type Item = Result<Entry<X>, Error>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let watching = self.get_mut();

        if let Some(entry) = watching.pending.pop_front() {
            return Poll::Ready(Some(Ok(entry)));
        }
        if watching.done {
            return Poll::Ready(None);
        }

        let mut step = match watching.step.take() {
            Some(step) => step,
            None => match watching.parts.take() {
                Some(parts) => {
                    let dispatcher = parts.trace.dispatcher.clone();
                    let span = tracing::dispatcher::with_default(
                        &dispatcher,
                        || tracing::trace_span!(target: "harmos::state", parent: &parts.trace.span, "state.watch_batch", after = parts.frontier.0),
                    );
                    Box::pin(next(parts).instrument(span).in_context(dispatcher))
                }
                None => {
                    watching.done = true;
                    return Poll::Ready(None);
                }
            },
        };

        match step.as_mut().poll(context) {
            Poll::Pending => {
                watching.step = Some(step);
                Poll::Pending
            }
            Poll::Ready((parts, Ok(found))) => {
                watching.parts = Some(parts);
                watching.pending.extend(found);
                match watching.pending.pop_front() {
                    Some(entry) => Poll::Ready(Some(Ok(entry))),
                    None => {
                        watching.done = true;
                        Poll::Ready(None)
                    }
                }
            }
            Poll::Ready((parts, Err(refusal))) => {
                watching.parts = Some(parts);
                watching.done = true;
                Poll::Ready(Some(Err(refusal)))
            }
        }
    }
}

#[tracing::instrument(target = "harmos::state", name = "state.replay", level = "debug", skip_all)]
pub(crate) fn replay<A, W>(
    catalog: Option<&Dispatch<A, W>>, recorded: Vec<Envelope<W>>, state: &mut At<A::State>, undos: &mut Undos<A>, inverses: Inverting<A>,
    tail: usize,
) -> Result<Vec<Arc<Entry<Recorded<A>>>>, DecodeError>
where
    A: Application,
    W: ReplayPayload<A>,
{
    let mut redelivered = VecDeque::new();

    for envelope in recorded {
        let prepared = envelope.payload.prepared();
        let position = envelope.position;
        let metadata = envelope.metadata.clone();
        let link = metadata.link;
        let principal = metadata.principal.clone();
        let forward = envelope.payload.forward();
        let inverse = forward.as_ref().and_then(|change| inverses(change, &state.value));
        let recovered = W::restore(envelope, catalog, &mut state.value)?;

        if let Some(forward) = &forward {
            undos.record(position, principal.as_ref(), link, forward, inverse);
        }
        if let Some(value) = recovered
            && tail != 0
        {
            // Keep a complete newest group, even when it exceeds the target window.
            let first = metadata.commit.map_or(position, |(first, _)| first);
            while redelivered.len() >= tail && redelivered.front().is_some_and(|entry: &Arc<Entry<Recorded<A>>>| entry.position < first) {
                let end = redelivered.front().map(|entry| entry.metadata.commit.map_or(entry.position, |(_, end)| end)).expect("nonempty");
                while redelivered.front().is_some_and(|entry| entry.position <= end) {
                    redelivered.pop_front();
                }
            }
            redelivered.push_back(Arc::new(Entry { prepared, position, metadata, value }));
        }
        state.position = position;
    }

    Ok(redelivered.into())
}

pub(crate) fn watching<A, X>(tracked: Arc<TrackedState<A>>, after: Position) -> impl Stream<Item = Result<Entry<X>, Error>>
where
    A: Application,
    X: Clone + Send + 'static,
{
    let doorbell = tracked.applied.subscribe();

    Watching {
        parts: Some(Parts {
            tracked,
            frontier: after,
            doorbell,
            trace: crate::supervision::executor::SpanContext::new(tracing::Span::current(), tracing::Span::none()),
        }),
        step: None,
        pending: VecDeque::new(),
        done: false,
    }
}

async fn next<A, X>(mut parts: Parts<A>) -> (Parts<A>, Result<Vec<Entry<X>>, Error>)
where
    A: Application,
    X: Clone + Send + 'static,
{
    loop {
        if parts.tracked.poisoned() {
            return (parts, Err(Error::Unavailable));
        }

        let scanned = {
            let tail = parts.tracked.tail.read().await;
            tail.scan::<X>(parts.frontier)
        };

        match scanned {
            Err(refusal) => return (parts, Err(refusal)),
            Ok((found, frontier)) => {
                parts.frontier = frontier;
                if !found.is_empty() {
                    return (parts, Ok(found));
                }
            }
        }

        if parts.doorbell.changed().await.is_err() {
            return (parts, Err(Error::Unavailable));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CorrelationId, Metadata};
    use serde::{Deserialize, Serialize};

    #[crate::record(id = "listener.page")]
    #[derive(Clone, Serialize, Deserialize)]
    struct PageRecord(u64);

    struct Model;

    #[crate::lanes]
    enum Lanes {
        #[default]
        Default,
    }
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = PageRecord;
        type Streams = ();
    }

    #[test]
    fn listener_pages_bound_bytes_without_losing_the_next_record() {
        let mut tail = Tail::<Model>::new(Position::ORIGIN, 600);
        for sequence in 1..=600 {
            tail.push_group(vec![Entry {
                prepared: None,
                position: Position(sequence),
                value: Recorded::Record(PageRecord(1024 * 1024)),
                metadata: Metadata {
                    commit: None,
                    principal: None,
                    correlation: CorrelationId(Position(sequence)),
                    causation: None,
                    request: None,
                    link: None,
                    at: std::time::UNIX_EPOCH,
                },
            }]);
        }
        let mut after = Position::ORIGIN;
        let mut delivered = 0;
        while after < Position(600) {
            let page = tail.shared_page_sized(after, Position(600), NonZeroUsize::new(512).unwrap(), |record| record.0 as usize).unwrap();
            assert_eq!(page.entries.len(), 8);
            for (index, entry) in page.entries.iter().enumerate() {
                assert_eq!(entry.position.0, after.0 + index as u64 + 1);
            }
            delivered += page.entries.len();
            after = page.reaches;
        }
        assert_eq!(delivered, 600);
        assert_eq!(tail.shared_page(Position::ORIGIN, Position(600), NonZeroUsize::new(512).unwrap()).unwrap().entries.len(), 512);
    }
}
