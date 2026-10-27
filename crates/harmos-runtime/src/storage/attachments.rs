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
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use crate::state::dispatch::{Decode, Dispatch, Restored};
use crate::state::{Feed, ReplayPayload};
use crate::storage::consumer::{Attached, JournalConsumer, Recording, Series, Serving, StreamConsumer};
use crate::{
    Application, DefinitionId, Error, runtime::Incompatible, state::DecodeError, state::Entry, state::Envelope, state::Kind,
    state::Position, state::Recorded, state::Transaction,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Payload<A: Application> {
    pub prepared: Option<crate::state::Prepared>,
    pub id: DefinitionId,
    pub version: u32,
    pub value: Recorded<A>,
}

pub struct Recovery<A: Application> {
    pub commits: Vec<(Position, Position)>,
    pub frontier: Position,
    pub entries: Vec<Envelope<Payload<A>>>,
    pub encoded: Vec<(Position, Arc<Vec<u8>>)>,
}

pub(crate) struct Attachments<A: Application> {
    pub(crate) capture: Option<crate::state::capture::Declaration<A>>,
    attached: Vec<Box<dyn Attached<A>>>,
    streams: Vec<Box<dyn Recording<A>>>,
    series: Option<Box<dyn Serving<A>>>,
    recovery: Option<Box<dyn Recovering<A>>>,
    snapshot: Option<Box<dyn Attached<A>>>,
    retention: Option<Horizon>,
    tail: usize,
}

pub(crate) struct Consumers<A: Application> {
    attached: Vec<Box<dyn Attached<A>>>,
    streams: Vec<Box<dyn Recording<A>>>,
    series: Option<Box<dyn Serving<A>>>,
    recovery: Option<Box<dyn Recovering<A>>>,
    snapshot: Option<Box<dyn Attached<A>>>,
    retention: Option<Horizon>,
    durable: Option<watch::Sender<Position>>,
    recovered: Vec<Arc<Entry<Recorded<A>>>>,
    stored: Vec<(Position, Arc<Vec<u8>>)>,
    frontier: Position,
    source_frontier: Position,
    snapshot_frontier: Position,
    pruned: Position,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horizon {
    BeneathSnapshot,
}

/* === Traits === */

pub trait Source<A: Application>: JournalConsumer<A> {
    fn load(&mut self) -> impl Future<Output = Result<Recovery<A>, Incompatible>> + Send;

    fn read(&mut self, _after: Position, _through: Position) -> impl Future<Output = Result<Recovery<A>, Error>> + Send {
        async { Err(Error::BeyondTail) }
    }

    fn prune(&mut self, _beneath: Position) -> impl Future<Output = ()> + Send {
        async {}
    }
}

pub(crate) trait Recovering<A: Application>: Attached<A> {
    fn load(&mut self) -> Pin<Box<dyn Future<Output = Result<Recovery<A>, Incompatible>> + Send + '_>>;

    fn read(&mut self, after: Position, through: Position) -> Pin<Box<dyn Future<Output = Result<Recovery<A>, Error>> + Send + '_>>;

    fn prune(&mut self, beneath: Position) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/* === Implementations === */

impl<A: Application> Default for Recovery<A> {
    fn default() -> Self {
        Self {
            commits: Vec::new(),
            frontier: Position::ORIGIN,
            entries: Vec::new(),
            encoded: Vec::new(),
        }
    }
}

impl<A: Application, S: Source<A>> Recovering<A> for S {
    fn load(&mut self) -> Pin<Box<dyn Future<Output = Result<Recovery<A>, Incompatible>> + Send + '_>> {
        Box::pin(Source::load(self))
    }

    fn read(&mut self, after: Position, through: Position) -> Pin<Box<dyn Future<Output = Result<Recovery<A>, Error>> + Send + '_>> {
        Box::pin(Source::read(self, after, through))
    }

    fn prune(&mut self, beneath: Position) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(Source::prune(self, beneath))
    }
}

impl<A: Application> ReplayPayload<A> for Payload<A> {
    fn prepared(&self) -> Option<crate::state::Prepared> {
        self.prepared.clone()
    }

    fn forward(&self) -> Option<Arc<A::Transaction>> {
        match &self.value {
            Recorded::Transaction(transaction) => Some(Arc::clone(transaction)),
            Recorded::Record(_) | Recorded::System(_) => None,
        }
    }

    fn restore(
        envelope: Envelope<Self>, catalog: Option<&Dispatch<A, Self>>, state: &mut A::State,
    ) -> Result<Option<Recorded<A>>, DecodeError> {
        let system = match &envelope.payload.value {
            Recorded::System(fact) => Some(fact.clone()),
            Recorded::Transaction(_) | Recorded::Record(_) => None,
        };
        if let Some(fact) = system {
            return Ok(Some(Recorded::System(fact)));
        }
        let forward = match &envelope.payload.value {
            Recorded::Transaction(transaction) => Some(Arc::clone(transaction)),
            Recorded::Record(_) | Recorded::System(_) => None,
        };

        let Some(catalog) = catalog else {
            let Payload { value, .. } = envelope.payload;
            return Ok(match value {
                Recorded::Transaction(transaction) => {
                    transaction.apply(state);
                    Some(Recorded::Transaction(transaction))
                }
                Recorded::Record(record) => Some(Recorded::Record(record)),
                Recorded::System(fact) => Some(Recorded::System(fact)),
            });
        };

        Ok(match catalog.restore(envelope, state)? {
            Restored::Applied => forward.map(Recorded::Transaction),
            Restored::Recorded(record) => Some(Recorded::Record(record)),
            Restored::Skipped => None,
        })
    }
}

impl<A: Application> Attachments<A> {
    pub(crate) fn has_recovery_owner(&self) -> bool {
        self.capture.is_some() || self.recovery.is_some() || self.snapshot.is_some() || self.retention.is_some()
    }

    pub(crate) fn new() -> Self {
        Self {
            capture: None,
            attached: Vec::new(),
            streams: Vec::new(),
            series: None,
            recovery: None,
            snapshot: None,
            retention: None,
            tail: 1024,
        }
    }

    pub(crate) fn require_capture(
        &mut self, limits: crate::state::CaptureLimits, consumer: impl crate::state::JournalCapture<A>,
    ) -> Result<(), crate::state::CaptureError> {
        if limits.pending_bytes == 0
            || limits.max_group_bytes == 0
            || limits.max_group_bytes > limits.pending_bytes
            || limits.pending_bytes as usize > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(crate::state::CaptureError::InvalidLimits);
        }
        self.capture = Some(crate::state::capture::Declaration::Callback(crate::state::capture::CallbackDeclaration {
            limits,
            consumer: Box::new(consumer),
        }));

        Ok(())
    }

    pub(crate) fn attach(&mut self, consumer: impl JournalConsumer<A>) {
        self.attached.push(Box::new(consumer));
    }

    pub(crate) fn attach_streams(&mut self, consumer: impl StreamConsumer<A>) {
        self.streams.push(Box::new(consumer));
    }

    pub(crate) fn series(&mut self, series: impl Series<A>) {
        if let Some(demoted) = self.series.replace(Box::new(series)) {
            self.streams.push(demoted);
        }
    }

    pub(crate) fn recover(&mut self, source: impl Source<A>) {
        if let Some(demoted) = self.recovery.replace(Box::new(source)) {
            self.attached.push(demoted);
        }
    }

    pub(crate) fn snapshot(&mut self, snapshot: impl JournalConsumer<A>) {
        if let Some(demoted) = self.snapshot.replace(Box::new(snapshot)) {
            self.attached.push(demoted);
        }
    }

    pub(crate) fn retain(&mut self, horizon: Horizon) {
        self.retention = Some(horizon);
    }

    pub(crate) fn tail(&mut self, entries: usize) {
        self.tail = entries.max(1);
    }

    pub(crate) fn tail_window(&self) -> Result<usize, Incompatible> {
        if self.retention == Some(Horizon::BeneathSnapshot) && self.snapshot.is_none() {
            return Err(Incompatible::unreadable("retention beneath a snapshot requires a snapshot declaration"));
        }

        Ok(self.tail)
    }

    pub(crate) fn supervised(&mut self) -> Consumers<A> {
        let snapshot_frontier = self.snapshot.as_ref().map_or(Position::ORIGIN, |snapshot| snapshot.resume());

        Consumers {
            attached: std::mem::take(&mut self.attached),
            streams: std::mem::take(&mut self.streams),
            series: self.series.take(),
            recovery: self.recovery.take(),
            snapshot: self.snapshot.take(),
            retention: self.retention,
            durable: None,
            recovered: Vec::new(),
            stored: Vec::new(),
            frontier: Position::ORIGIN,
            source_frontier: Position::ORIGIN,
            snapshot_frontier,
            pruned: Position::ORIGIN,
        }
    }
}

impl<A: Application> Default for Attachments<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Application> Consumers<A> {
    pub(crate) fn durability(&mut self, at: Position) -> Option<watch::Receiver<Position>> {
        let (durable, stored) = watch::channel(at);
        self.durable = self.recovery.as_ref().map(|_| durable);

        self.durable.as_ref().map(|_| stored)
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.load", level = "debug", skip_all)]
    pub(crate) async fn load(&mut self) -> Result<Recovery<A>, Incompatible> {
        let recovery = match &mut self.recovery {
            Some(source) => source.load().await,
            None => Ok(Recovery::default()),
        }?;
        self.source_frontier = recovery.frontier;

        Ok(recovery)
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.read", level = "debug", skip_all)]
    pub(crate) async fn read(&mut self, after: Position, through: Position) -> Result<Recovery<A>, Error> {
        match &mut self.recovery {
            Some(source) => source.read(after, through).await,
            None => Err(Error::BeyondTail),
        }
    }

    pub(crate) fn redelivering(
        &mut self, recovered: Vec<Arc<Entry<Recorded<A>>>>, stored: Vec<(Position, Arc<Vec<u8>>)>, frontier: Position,
    ) {
        self.recovered = recovered;
        self.stored = stored;
        self.frontier = frontier;
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.drain", level = "debug", skip_all)]
    pub(crate) async fn drain(&mut self, feed: &Feed<A>, applied: Position) -> Position {
        let mut settled = applied;

        if let Some(source) = &mut self.recovery {
            let durable = fold(source.as_mut(), &self.recovered, &self.stored, self.frontier, feed, applied).await;

            if let Some(stored) = &self.durable {
                stored.send_replace(durable);
            }
            self.source_frontier = durable;
            settled = settled.min(durable);
        }
        for consumer in &mut self.attached {
            settled = settled.min(fold(consumer.as_mut(), &self.recovered, &self.stored, self.frontier, feed, applied).await);
        }
        if let Some(snapshot) = &mut self.snapshot {
            let durable = fold(snapshot.as_mut(), &self.recovered, &self.stored, self.frontier, feed, applied).await;
            self.snapshot_frontier = durable;
            settled = settled.min(durable);
        }
        self.prune().await;
        self.forget_redelivered();

        settled
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.settle", level = "debug", skip_all)]
    pub(crate) async fn settle(&mut self, applied: Position) -> Position {
        let mut settled = applied;

        if let Some(source) = &mut self.recovery {
            let durable = source.drain(Vec::new(), applied).await;

            if let Some(stored) = &self.durable {
                stored.send_replace(durable);
            }
            self.source_frontier = durable;
            settled = settled.min(durable);
        }
        for consumer in &mut self.attached {
            settled = settled.min(consumer.drain(Vec::new(), applied).await);
        }
        if let Some(snapshot) = &mut self.snapshot {
            let durable = snapshot.drain(Vec::new(), applied).await;
            self.snapshot_frontier = durable;
            settled = settled.min(durable);
        }
        self.prune().await;

        settled
    }

    pub(crate) fn recordings(&mut self) -> impl Iterator<Item = &mut dyn Recording<A>> {
        self.streams
            .iter_mut()
            .map(|recording| recording.as_mut() as &mut dyn Recording<A>)
            .chain(self.series.iter_mut().map(|series| series.as_mut() as &mut dyn Recording<A>))
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.row_reader", level = "debug", skip_all)]
    pub(crate) async fn row_reader(
        &mut self, stream: crate::streams::Definition, track: &[String], start: std::ops::Bound<Position>, end: std::ops::Bound<Position>,
        limits: crate::streams::ReadLimits,
    ) -> Result<Box<dyn crate::streams::RowSource>, crate::streams::ReadError> {
        match &mut self.series {
            Some(series) => series.reader(stream, track, start, end, limits).await,
            None => Err(crate::streams::ReadError::Unserved),
        }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.served", level = "debug", skip_all)]
    pub(crate) async fn served(
        &mut self, stream: crate::streams::Definition, track: &[String], start: std::ops::Bound<Position>, end: std::ops::Bound<Position>,
    ) -> Result<Vec<crate::state::At<Arc<dyn std::any::Any + Send + Sync>>>, crate::streams::BeneathWindow> {
        match &mut self.series {
            Some(series) => series.rows(stream, track, start, end).await,
            None => Err(crate::streams::BeneathWindow::Unserved),
        }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.surveyed", level = "debug", skip_all)]
    pub(crate) async fn surveyed(
        &mut self, stream: crate::streams::Definition, track: &[String], start: std::ops::Bound<u64>, end: std::ops::Bound<u64>,
        budget: usize,
    ) -> Result<Vec<crate::streams::Bucket>, crate::streams::BeneathWindow> {
        match &mut self.series {
            Some(series) => series.buckets(stream, track, start, end, budget).await,
            None => Err(crate::streams::BeneathWindow::Unserved),
        }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.aggregated", level = "debug", skip_all)]
    pub(crate) async fn aggregated(
        &mut self, stream: crate::streams::Definition, track: &[String], start: std::ops::Bound<u64>, end: std::ops::Bound<u64>,
        kind: crate::Aggregate,
    ) -> Result<Vec<crate::streams::Aggregated>, crate::streams::BeneathWindow> {
        match &mut self.series {
            Some(series) => series.aggregate(stream, track, start, end, kind).await,
            None => Err(crate::streams::BeneathWindow::Unserved),
        }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.sealed", level = "debug", skip_all)]
    pub(crate) async fn sealed(&mut self, stream: crate::streams::Definition, track: &[String]) -> Position {
        match &mut self.series {
            Some(series) => series.seal(stream, track).await,
            None => Position::ORIGIN,
        }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.flush", level = "debug", skip_all)]
    pub(crate) async fn flush(&mut self) {
        if let Some(source) = &mut self.recovery {
            source.flush().await;
        }
        for consumer in &mut self.attached {
            consumer.flush().await;
        }
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.flush().await;
        }
        for consumer in self.recordings() {
            consumer.flush().await;
        }
    }

    fn forget_redelivered(&mut self) {
        let passed = self
            .recovery
            .iter()
            .map(|source| source.resume())
            .chain(self.attached.iter().map(|consumer| consumer.resume()))
            .chain(self.snapshot.iter().map(|snapshot| snapshot.resume()))
            .min()
            .is_none_or(|behind| behind >= self.frontier);

        if passed {
            self.recovered = Vec::new();
            self.stored = Vec::new();
        }
    }

    async fn prune(&mut self) {
        let Some(Horizon::BeneathSnapshot) = self.retention else {
            return;
        };
        let beneath = self.snapshot_frontier;
        if beneath <= self.pruned || self.source_frontier < beneath {
            return;
        }
        let Some(source) = &mut self.recovery else {
            return;
        };

        source.prune(beneath).await;
        self.pruned = beneath;
    }
}

/* === Public Functions === */

pub(crate) fn declare<A: Application>(
    catalog: &mut Dispatch<A, Payload<A>>, kind: Kind, id: DefinitionId, version: u32, decode: Decode<A>,
) {
    match kind {
        Kind::Transaction => catalog.set_transaction(id, version, apply, decode),
        Kind::Record => catalog.set_record(id, version, retain),
    }
}

/* === Private Functions === */

fn apply<A: Application>(payload: Payload<A>, state: &mut A::State) -> Result<(), DecodeError> {
    let Payload { prepared, id, version, value } = payload;
    match value {
        Recorded::Transaction(transaction) => {
            if let Some(prepared) = prepared {
                crate::state::TransactionCatalog::replay(&*transaction, &prepared, state)
            } else {
                transaction.apply(state);
                Ok(())
            }
        }
        Recorded::Record(_) | Recorded::System(_) => Err(mismatched(id, version)),
    }
}

fn retain<A: Application>(payload: Payload<A>) -> Result<A::Record, DecodeError> {
    match payload.value {
        Recorded::Record(record) => Ok(record),
        Recorded::Transaction(_) => Err(mismatched(payload.id, payload.version)),
        Recorded::System(_) => Err(mismatched(payload.id, payload.version)),
    }
}

fn mismatched(id: DefinitionId, version: u32) -> DecodeError {
    DecodeError::Malformed { id, version }
}

async fn fold<A: Application>(
    consumer: &mut dyn Attached<A>, recovered: &[Arc<Entry<Recorded<A>>>], stored: &[(Position, Arc<Vec<u8>>)], frontier: Position,
    feed: &Feed<A>, applied: Position,
) -> Position {
    let resume = consumer.resume();
    let Ok(live) = feed.since(resume.max(frontier)).await else {
        return consumer.drain(Vec::new(), resume).await;
    };
    // What is about to be redelivered is forwarded first in its stored form,
    // so a durable copy writes those bytes instead of re-encoding history.
    let owed = stored.partition_point(|(position, _)| *position <= resume);
    if owed < stored.len() {
        consumer.redelivered(&stored[owed..]);
    }
    let mut entries: Vec<Arc<Entry<Recorded<A>>>> = recovered.iter().filter(|entry| entry.position > resume).map(Arc::clone).collect();
    entries.extend(live);

    consumer.drain(entries, applied).await
}

/* === Tests === */
