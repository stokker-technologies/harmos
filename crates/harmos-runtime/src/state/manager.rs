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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
// Third Party Imports
use futures_core::Stream;
use tokio::sync::{RwLock, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::Instrument;
// Local Imports
use super::ManagedState;
use super::transaction::TransactionError;
use super::writer::{Admission, Direction, Refusal, Request};
use super::{
    At, CausationId, CorrelationId, DecodeError, DefinitionId, Entry, Error, Focus, Inverting, Position, PreparationError, Prepared,
    PrincipalId, ReadRequest, Reads, Receipt, Recorded, RequestKey, Reversals, Tail, Transaction, Undos, capture, dispatch, history,
    writer,
};
use crate::Application;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct TrackedState<A: Application> {
    pub(crate) state: RwLock<At<A::State>>,
    pub(crate) tail: RwLock<Tail<A>>,
    pub(super) applied: watch::Sender<Position>,
    pub(super) stored: watch::Receiver<Position>,
    pub(super) poisoned: AtomicBool,
    pub(crate) sealing: watch::Sender<bool>,
    pub(super) capture_failed: Arc<AtomicBool>,
    pub(super) capture_progress: Option<watch::Receiver<Result<Position, crate::state::CaptureError>>>,
    pub(super) catalog: Arc<dispatch::Dispatch<A, crate::storage::Payload<A>>>,
    pub(super) reads: Reads<A>,
    pub(super) record_preparation: Arc<tokio::sync::Semaphore>,
    pub(super) clock: fn() -> super::Timestamp,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Context {
    pub(super) principal: Option<PrincipalId>,
    pub(super) correlation: Option<CorrelationId>,
    pub(super) causation: Option<CausationId>,
}

pub struct StateManager<A: Application> {
    handles: Arc<()>,
    record_schemas: Arc<crate::records::Registry>,
    writer: mpsc::Sender<writer::QueuedRequest<A>>,
    context: Context,
    pub(super) tracked: Arc<TrackedState<A>>,
}

pub(crate) struct Feed<A: Application> {
    pub(super) tracked: Arc<TrackedState<A>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> TrackedState<A> {
    pub(super) fn unavailable<E>(&self) -> Error<E> {
        if self.capture_failed.load(Ordering::Acquire) { Error::CaptureFailed } else { Error::Unavailable }
    }

    pub(super) fn poison(&self) {
        self.poisoned.store(true, Ordering::Release);
    }

    pub(crate) fn poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire)
    }
}

impl<A: Application> StateManager<A> {
    pub fn state(&self) -> ManagedState<A> {
        ManagedState::new(Arc::clone(&self.tracked))
    }

    pub(crate) fn handle_probe(&self) -> Weak<()> {
        Arc::downgrade(&self.handles)
    }

    pub(crate) fn record_schemas(&self) -> Arc<crate::records::Registry> {
        Arc::clone(&self.record_schemas)
    }

    // The private boot boundary receives already-owned journal components.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        origin: At<A::State>, inverses: Inverting<A>, undos: Undos<A>, requests: usize, tail: Tail<A>,
        durability: Option<watch::Receiver<Position>>, catalog: Arc<dispatch::Dispatch<A, crate::storage::Payload<A>>>,
        capture: Option<capture::Declaration<A>>, storage: Option<crate::storage::StorageManager<crate::storage::ErasedCatalog>>,
        clock: fn() -> super::Timestamp,
    ) -> (Self, JoinHandle<()>, mpsc::Receiver<ReadRequest<A>>) {
        let position = origin.position;
        let (sender, receiver) = mpsc::channel(requests.max(1));
        let (reads, history) = Reads::channel(requests);
        let (applied, _) = watch::channel(position);
        let (durable, bridged) = watch::channel(position);
        let stored = durability.unwrap_or(bridged);
        let capture_failed = Arc::new(AtomicBool::new(false));
        let capture = capture.map(|capture| capture.start(capture_failed.clone(), position));
        let tracked = Arc::new(TrackedState {
            state: RwLock::new(origin),
            tail: RwLock::new(tail),
            applied,
            stored,
            poisoned: AtomicBool::new(false),
            sealing: watch::channel(false).0,
            capture_failed,
            capture_progress: capture.as_ref().map(capture::Capture::progress),
            record_preparation: Arc::new(tokio::sync::Semaphore::new(1)),
            catalog,
            reads,
            clock,
        });
        let running = tokio::spawn(writer::run(receiver, Arc::clone(&tracked), durable, inverses, undos, capture));

        (
            Self {
                handles: Arc::new(()),
                record_schemas: Arc::new(crate::records::Registry::new(storage)),
                writer: sender,
                context: Context::default(),
                tracked,
            },
            running,
            history,
        )
    }

    #[tracing::instrument(target = "harmos::state", name = "state.commit", level = "debug", skip_all)]
    pub async fn commit<T>(&self, transaction: T) -> Result<Receipt, Error<T::Error>>
    where
        T: Transaction<A::State> + Into<A::Transaction>,
        T::Error: From<TransactionError<A>>,
    {
        self.request(None, transaction.into()).await
    }

    #[tracing::instrument(target = "harmos::state", name = "state.commit_keyed", level = "debug", skip_all)]
    pub async fn commit_keyed<T>(&self, key: RequestKey, transaction: T) -> Result<Receipt, Error<T::Error>>
    where
        T: Transaction<A::State> + Into<A::Transaction>,
        T::Error: From<TransactionError<A>>,
    {
        self.request(Some(key), transaction.into()).await
    }

    pub(crate) fn record_resources(&self) -> (Arc<dispatch::Dispatch<A, crate::storage::Payload<A>>>, Arc<tokio::sync::Semaphore>) {
        (Arc::clone(&self.tracked.catalog), Arc::clone(&self.tracked.record_preparation))
    }

    #[tracing::instrument(target = "harmos::state", name = "state.append_prepared", level = "debug", skip_all)]
    pub(crate) async fn append_prepared(
        &self, value: Recorded<A>, prepared: Prepared, permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<Receipt, Error> {
        let (reply, answer) = oneshot::channel();
        let request = Request::Record { value, prepared, preparation: permit, context: self.context.clone(), reply };

        let waiting = self.writer.reserve().instrument(tracing::debug_span!(target: "harmos::state", "state.admission_wait"));
        let permit = match waiting.await {
            Ok(permit) => permit,
            Err(_) => return Err(self.unavailable()),
        };
        let queued = tracing::debug_span!(target: "harmos::state", "state.queue");
        permit.send((request, crate::supervision::executor::SpanContext::new(tracing::Span::current(), queued)));
        answer.await.map_err(|_| self.unavailable())?.map_err(preparation)
    }

    pub fn decode_transaction(&self, id: &DefinitionId, version: u32, bytes: &[u8]) -> Result<A::Transaction, DecodeError> {
        self.tracked.catalog.decode_transaction(id, version, bytes)
    }

    pub async fn query<X>(&self, after: Position) -> impl Iterator<Item = Result<Entry<X>, Error>>
    where
        X: Clone + Send + 'static,
    {
        self.query_through(after, self.applied()).await
    }

    #[tracing::instrument(target = "harmos::state", name = "state.query_through", level = "debug", skip_all)]
    pub async fn query_through<X>(&self, after: Position, through: Position) -> impl Iterator<Item = Result<Entry<X>, Error>>
    where
        X: Clone + Send + 'static,
    {
        if self.tracked.poisoned() {
            return vec![Err(self.unavailable())].into_iter();
        }

        if through > self.applied() {
            return vec![Err(Error::FrontierAhead)].into_iter();
        }
        let scanned = {
            let tail = self.tracked.tail.read().await;
            tail.scan_through::<X>(after, through)
        };

        match scanned {
            Ok((found, _)) => found.into_iter().map(Ok).collect::<Vec<_>>().into_iter(),
            Err(Error::BeyondTail) => match self.tracked.reads.entries(after, through).await {
                Ok(entries) => entries
                    .into_iter()
                    .filter_map(|entry| {
                        entry
                            .value
                            .select::<X>()
                            .map(|value| Ok(Entry { prepared: None, position: entry.position, metadata: entry.metadata, value }))
                    })
                    .collect::<Vec<_>>()
                    .into_iter(),
                Err(refusal) => vec![Err(refusal)].into_iter(),
            },
            Err(refusal) => vec![Err(refusal)].into_iter(),
        }
    }

    #[tracing::instrument(target = "harmos::state", name = "state.watch", level = "debug", skip_all)]
    pub fn watch<X>(&self, after: Position) -> impl Stream<Item = Result<Entry<X>, Error>>
    where
        X: Clone + Send + 'static,
    {
        history::watching(Arc::clone(&self.tracked), after)
    }

    pub fn applied(&self) -> Position {
        *self.tracked.applied.borrow()
    }

    pub(crate) fn progress(&self) -> watch::Receiver<Position> {
        self.tracked.applied.subscribe()
    }

    pub(crate) fn seal(&self) {
        self.tracked.sealing.send_replace(true);
    }

    pub(crate) fn feed(&self) -> Feed<A> {
        Feed { tracked: Arc::clone(&self.tracked) }
    }

    pub fn stored(&self) -> Position {
        *self.tracked.stored.borrow()
    }

    #[tracing::instrument(target = "harmos::state", name = "state.wait_until_stored", level = "debug", skip_all)]
    pub async fn wait_until_stored(&self, at: Position) {
        let mut stored = self.tracked.stored.clone();

        while *stored.borrow_and_update() < at {
            if stored.changed().await.is_err() {
                return;
            }
        }
    }

    #[tracing::instrument(target = "harmos::state", name = "state.undo", level = "debug", skip_all)]
    pub async fn undo(&self, principal: PrincipalId, focus: Focus<A>) -> Result<Receipt, Error<TransactionError<A>>> {
        self.revert(principal, focus, Direction::Undo).await
    }

    #[tracing::instrument(target = "harmos::state", name = "state.redo", level = "debug", skip_all)]
    pub async fn redo(&self, principal: PrincipalId, focus: Focus<A>) -> Result<Receipt, Error<TransactionError<A>>> {
        self.revert(principal, focus, Direction::Redo).await
    }

    #[tracing::instrument(target = "harmos::state", name = "state.reversals", level = "debug", skip_all)]
    pub async fn reversals(&self, principal: PrincipalId, focus: Focus<A>) -> Result<At<Reversals>, Error> {
        if self.tracked.poisoned() {
            return Err(self.unavailable());
        }

        let (reply, answer) = oneshot::channel();
        let request = Request::Reversals { principal, focus, reply };

        let waiting = self.writer.reserve().instrument(tracing::debug_span!(target: "harmos::state", "state.admission_wait"));
        let permit = match waiting.await {
            Ok(permit) => permit,
            Err(_) => return Err(self.unavailable()),
        };
        let queued = tracing::debug_span!(target: "harmos::state", "state.queue");
        permit.send((request, crate::supervision::executor::SpanContext::new(tracing::Span::current(), queued)));

        answer.await.map_err(|_| self.unavailable())
    }

    pub fn as_principal(&self, principal: PrincipalId) -> Self {
        let mut scoped = self.clone();
        scoped.context.principal = Some(principal);
        scoped
    }

    pub fn caused_by(&self, cause: Position) -> Self {
        let mut scoped = self.clone();
        scoped.context.correlation = Some(CorrelationId(cause));
        scoped.context.causation = Some(CausationId(cause));
        scoped
    }

    #[tracing::instrument(target = "harmos::state", name = "state.revert", level = "debug", skip_all)]
    async fn revert(&self, principal: PrincipalId, focus: Focus<A>, direction: Direction) -> Result<Receipt, Error<TransactionError<A>>> {
        let (reply, answer) = oneshot::channel();
        let request = Request::Revert { principal, focus, direction, context: self.context.clone(), reply };

        let waiting = self.writer.reserve().instrument(tracing::debug_span!(target: "harmos::state", "state.admission_wait"));
        let permit = match waiting.await {
            Ok(permit) => permit,
            Err(_) => return Err(self.unavailable()),
        };
        let queued = tracing::debug_span!(target: "harmos::state", "state.queue");
        permit.send((request, crate::supervision::executor::SpanContext::new(tracing::Span::current(), queued)));
        match answer.await {
            Ok(Ok(receipt)) => Ok(receipt),
            Ok(Err(Refusal::Preparation(refusal))) => Err(preparation(refusal)),
            Ok(Err(Refusal::Conflict(refusal))) => Err(match direction {
                Direction::Undo => Error::UndoConflict(refusal),
                Direction::Redo => Error::RedoConflict(refusal),
            }),
            Ok(Err(Refusal::Nothing)) => Err(match direction {
                Direction::Undo => Error::NothingToUndo,
                Direction::Redo => Error::NothingToRedo,
            }),
            Err(_) => Err(self.unavailable()),
        }
    }

    #[tracing::instrument(target = "harmos::state", name = "state.wait_until_captured", level = "debug", skip_all)]
    pub async fn wait_until_captured(&self, through: Position) -> Result<(), crate::state::CaptureError> {
        let Some(mut progress) = self.tracked.capture_progress.clone() else {
            return Ok(());
        };
        loop {
            if (*progress.borrow_and_update())? >= through {
                return Ok(());
            }
            if progress.changed().await.is_err() {
                return Err(crate::state::CaptureError::Failed);
            }
        }
    }

    pub fn capture_status(&self) -> Result<(), crate::state::CaptureError> {
        if self.tracked.capture_failed.load(Ordering::Acquire) { Err(crate::state::CaptureError::Failed) } else { Ok(()) }
    }

    fn unavailable<E>(&self) -> Error<E> {
        if self.capture_status().is_err() { Error::CaptureFailed } else { Error::Unavailable }
    }

    async fn request<E>(&self, key: Option<RequestKey>, transaction: A::Transaction) -> Result<Receipt, Error<E>>
    where
        E: From<TransactionError<A>>,
    {
        let (reply, answer) = oneshot::channel();
        let request = Request::Commit { transaction, key, context: self.context.clone(), reply };

        let waiting = self.writer.reserve().instrument(tracing::debug_span!(target: "harmos::state", "state.admission_wait"));
        let permit = match waiting.await {
            Ok(permit) => permit,
            Err(_) => return Err(self.unavailable()),
        };
        let queued = tracing::debug_span!(target: "harmos::state", "state.queue");
        permit.send((request, crate::supervision::executor::SpanContext::new(tracing::Span::current(), queued)));
        match answer.await {
            Ok(Ok(receipt)) => Ok(receipt),
            Ok(Err(Admission::Rejected(refusal))) => Err(Error::Rejected(E::from(refusal))),
            Ok(Err(Admission::Preparation(refusal))) => Err(preparation(refusal)),
            Err(_) => Err(self.unavailable()),
        }
    }
}

impl<A: Application> Feed<A> {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    #[tracing::instrument(target = "harmos::state", name = "state.shared_page", level = "debug", skip_all)]
    pub(crate) async fn shared_page(
        &self, after: Position, through: Position, max_rows: std::num::NonZeroUsize,
    ) -> Result<history::SharedPage<A>, Error> {
        self.tracked.tail.read().await.shared_page(after, through, max_rows)
    }

    #[tracing::instrument(target = "harmos::state", name = "state.since", level = "debug", skip_all)]
    pub(crate) async fn since(&self, after: Position) -> Result<Vec<Arc<Entry<Recorded<A>>>>, Error> {
        let tail = self.tracked.tail.read().await;

        tail.since(after)
    }
}

impl<A: Application> Clone for StateManager<A> {
    fn clone(&self) -> Self {
        Self {
            handles: Arc::clone(&self.handles),
            record_schemas: Arc::clone(&self.record_schemas),
            writer: self.writer.clone(),
            context: self.context.clone(),
            tracked: Arc::clone(&self.tracked),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn preparation<E>(error: PreparationError) -> Error<E> {
    match error {
        PreparationError::CaptureFailed => Error::CaptureFailed,
        error => Error::Preparation(error),
    }
}

/* === Tests === */
