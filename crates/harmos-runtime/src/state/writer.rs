/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, watch};
use tracing::Instrument;

use super::entry::{CausationId, CorrelationId, Entry, Link, Metadata, Position, PrincipalId, Receipt, RequestKey, Timestamp};
use super::envelope::Recorded;
use super::history::{Focus, Reversals, Undos};
use super::record::{PreparationError, Prepared, PreparedCommit};
use super::transaction::Inverting;
use super::transaction::{Transaction, TransactionError};
use super::{Context, TrackedState};
use crate::supervision::executor::InContext;
use crate::{Application, state::At};

pub(crate) type QueuedRequest<A> = (Request<A>, crate::supervision::executor::SpanContext);

#[derive(Clone)]
struct Unsealed {
    principal: Option<PrincipalId>,
    correlation: Option<CorrelationId>,
    causation: Option<CausationId>,
    request: Option<RequestKey>,
    link: Option<Link>,
    at: Timestamp,
}

pub(crate) enum Request<A: Application> {
    Commit {
        transaction: crate::operations::Response<A::Transaction>,
        key: Option<RequestKey>,
        context: Context,
        reply: oneshot::Sender<Result<Receipt, Admission<A>>>,
    },
    Record {
        value: Recorded<A>,
        prepared: Prepared,
        preparation: tokio::sync::OwnedSemaphorePermit,
        context: Context,
        reply: oneshot::Sender<Result<Receipt, PreparationError>>,
    },
    Revert {
        principal: PrincipalId,
        focus: Focus<A>,
        direction: Direction,
        context: Context,
        reply: oneshot::Sender<Result<Receipt, Refusal<A>>>,
    },
    Reversals {
        principal: PrincipalId,
        focus: Focus<A>,
        reply: oneshot::Sender<At<Reversals>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Undo,
    Redo,
}

pub(crate) enum Admission<A: Application> {
    Rejected(TransactionError<A>),
    Preparation(PreparationError),
}

pub(crate) enum Refusal<A: Application> {
    Preparation(PreparationError),
    Conflict(TransactionError<A>),
    Nothing,
}

impl Direction {
    fn link(self, original: Position) -> Link {
        match self {
            Self::Undo => Link::UndoOf(original),
            Self::Redo => Link::RedoOf(original),
        }
    }
}

impl Unsealed {
    fn mint(context: &Context, request: Option<RequestKey>, link: Option<Link>, clock: fn() -> Timestamp) -> Self {
        Self {
            principal: context.principal.clone(),
            correlation: context.correlation,
            causation: context.causation,
            request,
            link,
            at: witnessed_at(clock()),
        }
    }

    fn seal(self, position: Position) -> Metadata {
        Metadata {
            commit: None,
            principal: self.principal,
            correlation: self.correlation.unwrap_or(CorrelationId(position)),
            causation: self.causation,
            request: self.request,
            link: self.link,
            at: self.at,
        }
    }
}

pub(crate) async fn run<A: Application>(
    mut requests: mpsc::Receiver<QueuedRequest<A>>, tracked: Arc<TrackedState<A>>, stored: watch::Sender<Position>, inverses: Inverting<A>,
    mut undos: Undos<A>, mut capture: Option<super::capture::Capture<A>>,
) {
    let mut receipts = capture.as_mut().map(|capture| capture.take_receipts()).unwrap_or_default();
    run_admissions(&mut requests, &tracked, &stored, inverses, &mut undos, capture.as_ref(), &mut receipts).await;
    if let Some(capture) = capture {
        capture.snapshot(&tracked, &undos, &receipts, true).await;
        capture.finish().await;
    }
}

async fn run_admissions<A: Application>(
    requests: &mut mpsc::Receiver<QueuedRequest<A>>, tracked: &Arc<TrackedState<A>>, stored: &watch::Sender<Position>,
    inverses: Inverting<A>, undos: &mut Undos<A>, capture: Option<&super::capture::Capture<A>>,
    receipts: &mut HashMap<RequestKey, Receipt>,
) {
    let mut sealing = tracked.sealing.subscribe();

    while let Some((request, mut trace)) = next_request(requests, &mut sealing, capture).await {
        let span = tracing::dispatcher::with_default(
            &trace.dispatcher,
            || tracing::debug_span!(target: "harmos::state", parent: &trace.span, "state.write", outcome = "pending", position = tracing::field::Empty, frontier = tracing::field::Empty),
        );
        span.follows_from(trace.predecessor());
        trace.dequeued();
        let proceed = async {
            match request {
                Request::Commit { transaction, key, context, reply } => {
                    if let Some(seen) = key.as_ref().and_then(|key| receipts.get(key)) {
                        let _ = reply.send(Ok(*seen));
                        return true;
                    }

                    let draft = Unsealed::mint(&context, key.clone(), None, tracked.clock);

                    // Prepare against the pre-state, release it while reserving capture
                    // capacity, then lock through apply and publication. Only this
                    // task mutates state, so the prepared pre-state cannot change.
                    let sealed = {
                        let state = tracked.state.read().instrument(tracing::debug_span!(target: "harmos::state", "state.read_lock")).await;

                        if let Err(refusal) =
                            tracing::debug_span!(target: "harmos::state", "state.validate").in_scope(|| transaction.check(&state.value))
                        {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Admission::Rejected(refusal)));
                            return true;
                        }

                        if !context_fits(&context, key.as_ref()) {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Admission::Preparation(PreparationError::Limit)));
                            return true;
                        }
                        let prepared = match tracing::debug_span!(target: "harmos::state", "state.prepare")
                            .in_scope(|| super::record::prepare::<A>(&transaction, &state.value, &tracked.catalog))
                        {
                            Ok(prepared) => prepared,
                            Err(refusal) => {
                                tracing::Span::current().record("outcome", "refused");
                                let _ = reply.send(Err(Admission::Preparation(refusal)));
                                return true;
                            }
                        };
                        let Some(frontier) = state.position.0.checked_add(1 + prepared.records.len() as u64) else {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Admission::Preparation(PreparationError::Limit)));
                            return true;
                        };
                        // Inverse capture belongs here, between check and apply,
                        // because an inverse is a function of the state before it.
                        drop(state);
                        let permit = match reserve(
                            capture,
                            &prepared.transaction,
                            prepared.records.iter().map(|(_, prepared)| prepared),
                            &draft,
                            (Position(frontier - prepared.records.len() as u64), crate::state::Kind::Transaction, true),
                        )
                        .await
                        {
                            Ok(permit) => permit,
                            Err(error) => {
                                let failed = error == PreparationError::CaptureFailed;
                                tracing::Span::current().record("outcome", "refused");
                                let _ = reply.send(Err(Admission::Preparation(error)));
                                if failed {
                                    return false;
                                }
                                return true;
                            }
                        };
                        let mut state =
                            tracked.state.write().instrument(tracing::debug_span!(target: "harmos::state", "state.write_lock")).await;
                        if let Some(capture) = capture
                            && let Err(error) = capture.check()
                        {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Admission::Preparation(error)));
                            return false;
                        }
                        let inverse = inverses(&transaction, &state.value);
                        let applied = tracing::debug_span!(target: "harmos::state", "state.apply")
                            .in_scope(|| catch_unwind(AssertUnwindSafe(|| transaction.apply(&mut state.value))));
                        if applied.is_err() {
                            tracked.poison();
                            return false;
                        }

                        let position = Position(state.position.0 + 1);
                        state.position = Position(frontier);
                        (state, position, inverse, prepared, permit)
                    };

                    let (state, position, inverse, prepared, permit) = sealed;
                    let receipt = publish_group(tracked, stored, position, draft, transaction.clone(), prepared, capture.zip(permit)).await;
                    drop(state);

                    undos.record(position, context.principal.as_ref(), None, &transaction, inverse);
                    if let Some(key) = key {
                        receipts.insert(key, receipt);
                    }
                    tracing::Span::current().record("outcome", "applied");
                    tracing::Span::current().record("position", receipt.position.0);
                    tracing::Span::current().record("frontier", receipt.frontier.0);
                    let _ = reply.send(Ok(receipt));
                }
                Request::Revert {
                    principal,
                    focus,
                    direction,
                    mut context,
                    reply,
                } => {
                    let chosen = match direction {
                        Direction::Undo => undos.undoable(&principal, &focus),
                        Direction::Redo => undos.redoable(&principal, &focus),
                    };
                    let Some((original, answer)) = chosen else {
                        tracing::Span::current().record("outcome", "refused");
                        let _ = reply.send(Err(Refusal::Nothing));
                        return true;
                    };

                    // The principal doing the undoing is the one witnessed on the
                    // entry it appends, whichever handle carried the request.
                    context.principal = Some(principal);
                    let link = direction.link(original);
                    let draft = Unsealed::mint(&context, None, Some(link), tracked.clock);

                    // Prepare against the pre-state, release it while reserving capture
                    // capacity, then lock through apply and publication. Only this
                    // task mutates state, so the prepared pre-state cannot change.
                    let sealed = {
                        let state = tracked.state.read().instrument(tracing::debug_span!(target: "harmos::state", "state.read_lock")).await;

                        // The inverse is an ordinary transaction, so it runs its
                        // ordinary check against the *current* state. A refusal is
                        // the world having moved on, and it appends nothing.
                        if let Err(refusal) = answer.check(&state.value) {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Refusal::Conflict(refusal)));
                            return true;
                        }

                        if !context_fits(&context, None) {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Refusal::Preparation(PreparationError::Limit)));
                            return true;
                        }
                        let prepared = match super::record::prepare::<A>(&answer, &state.value, &tracked.catalog) {
                            Ok(prepared) => prepared,
                            Err(refusal) => {
                                tracing::Span::current().record("outcome", "refused");
                                let _ = reply.send(Err(Refusal::Preparation(refusal)));
                                return true;
                            }
                        };
                        let Some(frontier) = state.position.0.checked_add(1 + prepared.records.len() as u64) else {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Refusal::Preparation(PreparationError::Limit)));
                            return true;
                        };
                        drop(state);
                        let permit = match reserve(
                            capture,
                            &prepared.transaction,
                            prepared.records.iter().map(|(_, prepared)| prepared),
                            &draft,
                            (Position(frontier - prepared.records.len() as u64), crate::state::Kind::Transaction, true),
                        )
                        .await
                        {
                            Ok(permit) => permit,
                            Err(error) => {
                                let failed = error == PreparationError::CaptureFailed;
                                tracing::Span::current().record("outcome", "refused");
                                let _ = reply.send(Err(Refusal::Preparation(error)));
                                if failed {
                                    return false;
                                }
                                return true;
                            }
                        };
                        let mut state =
                            tracked.state.write().instrument(tracing::debug_span!(target: "harmos::state", "state.write_lock")).await;
                        if let Some(capture) = capture
                            && let Err(error) = capture.check()
                        {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(Refusal::Preparation(error)));
                            return false;
                        }
                        let inverse = inverses(&answer, &state.value);
                        let applied = catch_unwind(AssertUnwindSafe(|| answer.apply(&mut state.value)));
                        if applied.is_err() {
                            tracked.poison();
                            return false;
                        }

                        let position = Position(state.position.0 + 1);
                        state.position = Position(frontier);
                        (state, position, inverse, prepared, permit)
                    };

                    let (state, position, inverse, prepared, permit) = sealed;
                    let receipt = publish_group(tracked, stored, position, draft, answer.clone(), prepared, capture.zip(permit)).await;
                    drop(state);

                    undos.record(position, context.principal.as_ref(), Some(link), &answer, inverse);
                    tracing::Span::current().record("outcome", "applied");
                    tracing::Span::current().record("position", receipt.position.0);
                    tracing::Span::current().record("frontier", receipt.frontier.0);
                    let _ = reply.send(Ok(receipt));
                }
                Request::Record {
                    value,
                    prepared,
                    preparation: _preparation,
                    context,
                    reply,
                } => {
                    if !context_fits(&context, None) {
                        tracing::Span::current().record("outcome", "refused");
                        let _ = reply.send(Err(PreparationError::Limit));
                        return true;
                    }
                    let draft = Unsealed::mint(&context, None, None, tracked.clock);
                    let Some(next) = tracked
                        .state
                        .read()
                        .instrument(tracing::debug_span!(target: "harmos::state", "state.read_lock"))
                        .await
                        .position
                        .0
                        .checked_add(1)
                    else {
                        tracing::Span::current().record("outcome", "refused");
                        let _ = reply.send(Err(PreparationError::Limit));
                        return true;
                    };
                    let permit = match reserve(capture, &prepared, std::iter::empty(), &draft, (Position(next), value.kind(), false)).await
                    {
                        Ok(permit) => permit,
                        Err(error) => {
                            let failed = error == PreparationError::CaptureFailed;
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(error));
                            if failed {
                                return false;
                            }
                            return true;
                        }
                    };

                    // Keep state locked until the tail and applied frontier publish.
                    // Every At returned by a reader must already name queryable history.
                    let sealed = {
                        let mut state =
                            tracked.state.write().instrument(tracing::debug_span!(target: "harmos::state", "state.write_lock")).await;
                        if let Some(capture) = capture
                            && let Err(error) = capture.check()
                        {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(error));
                            return false;
                        }
                        let Some(next) = state.position.0.checked_add(1) else {
                            tracing::Span::current().record("outcome", "refused");
                            let _ = reply.send(Err(PreparationError::Limit));
                            return true;
                        };
                        let position = Position(next);
                        state.position = position;
                        (state, position)
                    };

                    let (state, position) = sealed;
                    let receipt = publish(tracked, stored, position, draft, value, prepared, capture.zip(permit)).await;
                    drop(state);

                    tracing::Span::current().record("outcome", "applied");
                    tracing::Span::current().record("position", receipt.position.0);
                    tracing::Span::current().record("frontier", receipt.frontier.0);
                    let _ = reply.send(Ok(receipt));
                }
                Request::Reversals { principal, focus, reply } => {
                    // Stamped from the same state a `read` is stamped from, because
                    // a view compares the two: an answer a caller cannot place in
                    // history is one it cannot tell from a stale one.
                    let position =
                        tracked.state.read().instrument(tracing::debug_span!(target: "harmos::state", "state.read_lock")).await.position;

                    let _ = reply.send(At {
                        value: undos.reversals(&principal, &focus),
                        position,
                    });
                }
            }
            if let Some(capture) = capture {
                capture.snapshot(tracked, undos, receipts, false).await;
            }
            true
        }
        .instrument(span)
        .in_context(trace.dispatcher.clone())
        .await;
        if !proceed {
            return;
        }
    }
}

#[tracing::instrument(target = "harmos::state", name = "state.publish", level = "debug", skip_all, fields(position = position.0))]
async fn publish<A: Application>(
    tracked: &TrackedState<A>, stored: &watch::Sender<Position>, position: Position, draft: Unsealed, value: Recorded<A>,
    prepared: Prepared, capture: Option<(&super::capture::Capture<A>, super::capture::Permit)>,
) -> Receipt {
    let metadata = draft.seal(position);
    let correlation = metadata.correlation;

    {
        let mut tail = tracked.tail.write().await;
        let entries = tail.push_group(vec![Entry {
            prepared: Some(prepared),
            position,
            metadata,
            value,
        }]);
        // A storage worker may flush on another thread as soon as ownership
        // transfers. Publish the applied frontier before it can acknowledge save.
        tracked.applied.send_replace(position);
        if let Some((capture, permit)) = capture {
            capture.submit(entries, position, permit);
        }
    }

    // Memory-only durability: absent a recovery source, `stored` tracks
    // `applied` on this same code path, so `wait_until_stored` needs no `cfg`.
    stored.send_replace(position);

    Receipt {
        frontier: position,
        position,
        correlation,
    }
}

#[tracing::instrument(target = "harmos::state", name = "state.publish", level = "debug", skip_all, fields(position = position.0))]
async fn publish_group<A: Application>(
    tracked: &TrackedState<A>, stored: &watch::Sender<Position>, position: Position, draft: Unsealed,
    transaction: crate::operations::Response<A::Transaction>, prepared: PreparedCommit<A>,
    capture: Option<(&super::capture::Capture<A>, super::capture::Permit)>,
) -> Receipt {
    let PreparedCommit { transaction: prepared, records } = prepared;
    let frontier = Position(position.0 + records.len() as u64);
    let mut metadata = draft.seal(position);
    metadata.commit = Some((position, frontier));
    let correlation = metadata.correlation;
    let mut entries = Vec::with_capacity(records.len() + 1);
    entries.push(Entry {
        position,
        metadata: metadata.clone(),
        prepared: Some(prepared),
        value: Recorded::Transaction(transaction),
    });
    metadata.causation = Some(CausationId(position));
    metadata.request = None;
    metadata.link = None;
    for (index, (record, prepared)) in records.into_iter().enumerate() {
        entries.push(Entry {
            position: Position(position.0 + index as u64 + 1),
            metadata: metadata.clone(),
            prepared: Some(prepared),
            value: record,
        });
    }
    let entries = tracked.tail.write().await.push_group(entries);
    tracked.applied.send_replace(frontier);
    if let Some((capture, permit)) = capture {
        capture.submit(entries, frontier, permit);
    }
    stored.send_replace(frontier);
    Receipt { position, frontier, correlation }
}

fn witnessed_at(now: Timestamp) -> Timestamp {
    let rounded = match now.duration_since(Timestamp::UNIX_EPOCH) {
        Ok(since) => {
            u64::try_from(since.as_micros()).ok().and_then(|micros| Timestamp::UNIX_EPOCH.checked_add(Duration::from_micros(micros)))
        }
        Err(before) => u64::try_from(before.duration().as_micros())
            .ok()
            .and_then(|micros| Timestamp::UNIX_EPOCH.checked_sub(Duration::from_micros(micros))),
    };

    rounded.unwrap_or(now)
}

async fn next_request<A: Application>(
    requests: &mut mpsc::Receiver<QueuedRequest<A>>, sealing: &mut watch::Receiver<bool>, capture: Option<&super::capture::Capture<A>>,
) -> Option<QueuedRequest<A>> {
    if *sealing.borrow_and_update() {
        requests.close();
    }

    let mut failure = capture.map(super::capture::Capture::failure);
    if failure.as_ref().is_some_and(|failure| *failure.borrow()) {
        return None;
    }

    let mut sealed = Box::pin(sealing.changed());
    let mut done = false;
    let mut failed = failure.as_mut().map(|failure| Box::pin(failure.changed()));

    std::future::poll_fn(|cx| {
        if let Some(failed) = failed.as_mut()
            && failed.as_mut().poll(cx).is_ready()
        {
            return std::task::Poll::Ready(None);
        }
        if !done && sealed.as_mut().poll(cx).is_ready() {
            done = true;
            requests.close();
        }

        requests.poll_recv(cx)
    })
    .await
}

#[tracing::instrument(target = "harmos::state", name = "state.reserve", level = "debug", skip_all)]
async fn reserve<'a, A: Application>(
    capture: Option<&super::capture::Capture<A>>, prepared: &'a Prepared, others: impl Iterator<Item = &'a Prepared>, draft: &Unsealed,
    group: (Position, crate::state::Kind, bool),
) -> Result<Option<super::capture::Permit>, PreparationError> {
    let Some(capture) = capture else {
        return Ok(None);
    };
    let others: Vec<_> = others.collect();
    let count = 1 + others.len();
    let metadata = serde_json::to_vec(&draft.clone().seal(Position(u64::MAX))).map_err(|_| PreparationError::Encoding)?.len();
    let mut bytes = count.checked_mul(metadata + 256).ok_or(PreparationError::Limit)?;
    for prepared in std::iter::once(prepared).chain(others.iter().copied()) {
        bytes = bytes.checked_add(prepared.bytes()?).ok_or(PreparationError::Limit)?;
    }
    let (position, kind, commit) = group;
    let mut metadata = draft.clone().seal(position);
    if commit {
        metadata.commit = Some((position, Position(position.0 + others.len() as u64)));
    }
    let mut entries = Vec::with_capacity(count);
    entries.push(crate::state::Envelope {
        position,
        kind,
        id: prepared.id.clone(),
        version: prepared.version,
        metadata: metadata.clone(),
        payload: &prepared.payload,
    });
    metadata.causation = Some(CausationId(position));
    metadata.request = None;
    metadata.link = None;
    for (index, prepared) in others.into_iter().enumerate() {
        entries.push(crate::state::Envelope {
            position: Position(position.0 + index as u64 + 1),
            kind: crate::state::Kind::Record,
            id: prepared.id.clone(),
            version: prepared.version,
            metadata: metadata.clone(),
            payload: &prepared.payload,
        });
    }
    capture.reserve(bytes, &crate::state::recovery::Group::new(entries)).await.map(Some)
}

fn context_fits(context: &Context, key: Option<&RequestKey>) -> bool {
    context.principal.as_ref().is_none_or(|id| id.as_str().len() <= 1024) && key.is_none_or(|key| key.as_str().len() <= 1024)
}
