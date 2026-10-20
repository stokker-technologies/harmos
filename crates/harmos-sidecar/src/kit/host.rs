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
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::{Semaphore, SemaphorePermit, mpsc, oneshot};
// Local Imports
use crate::Frame;
use harmos_domain::Message;
use harmos_domain::operations::instance::Id;
use harmos_sdk::operations::{CHILD_METHOD, OperationDeclaration, WireChildAction, WireChildControl};

/* === Constants === */

const PENDING_CALLS: usize = 256;
// One Batched Call Claims No More Host Preparation Than One Maximal Row.
const PUBLICATION_BATCH_BYTES: usize = 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct PendingCalls {
    state: Mutex<PendingState>,
    capacity: Semaphore,
}

struct PendingState {
    closed: bool,
    next_id: u64,
    waiters: HashMap<u64, oneshot::Sender<Result<Vec<u8>, CallError>>>,
}

struct PendingCall<'a> {
    pending: &'a PendingCalls,
    id: u64,
    _permit: SemaphorePermit<'a>,
    cancel: Option<mpsc::Permit<'a, Outgoing>>,
}

#[derive(Clone)]
pub struct Supervisor {
    outgoing: mpsc::Sender<Outgoing>,
    sequences: Arc<Mutex<BTreeMap<&'static str, u64>>>,
    pending: Arc<PendingCalls>,
    operation_context: Option<[u8; 32]>,
    generation_context: Option<[u8; 32]>,
    call: Arc<harmos_domain::operations::CallContext>,
    clock: fn() -> Option<i64>,
}

/* === Enums === */

pub(crate) enum Outgoing {
    Frame(Frame),
    Started(u64),
    Finished(u64),
    Exiting(String),
    Abandon,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    // The host's refusal carries the failure unchanged; this layer names only
    // the operation the host attributed it to.
    #[error("{}", refused_line(*.code, .failure))]
    Refused {
        code: i64,

        #[source]
        failure: harmos_domain::Failure,
    },

    #[error("the host connection is unavailable")]
    Unavailable,

    #[error("the host returned a malformed reply for `{method}`: {message}")]
    Malformed { method: &'static str, message: String },

    #[error("the request for `{method}` requires {encoded} bytes; the frame limit is {limit}")]
    Oversized { method: &'static str, encoded: usize, limit: usize },

    #[error(transparent)]
    Publication(#[from] PublicationError),

    #[error("`{method}` needs process-local resources a connection cannot carry")]
    LocalResources { method: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PublicationError {
    #[error("publication sequence exhausted for stream `{stream}`")]
    SequenceExhausted { stream: &'static str },

    #[error("the observer did not declare stream `{stream}` in streams(...)")]
    Undeclared { stream: &'static str },

    #[error("the publication requires {encoded} bytes; the frame limit is {limit}")]
    Oversized { encoded: usize, limit: usize },
}

/* === Traits === */

/* === Implementations === */

// A handler that returns a host refusal forwards its failure unchanged rather
// than flattening the call error's own line into a new cause.
impl From<CallError> for harmos_domain::Refusal {
    fn from(error: CallError) -> Self {
        match error {
            CallError::Refused { code, failure } => Self::failed(code, failure),
            error => Self::of(error),
        }
    }
}

impl harmos_sdk::plugin::Context for Supervisor {
    fn with_generation(&self, token: [u8; 32]) -> Self {
        let mut host = self.clone();
        host.generation_context = Some(token);
        host
    }
}

impl PendingCalls {
    fn new() -> Self {
        Self {
            state: Mutex::new(PendingState { closed: false, next_id: 1, waiters: HashMap::new() }),
            capacity: Semaphore::new(PENDING_CALLS),
        }
    }

    fn register(&self, waiter: oneshot::Sender<Result<Vec<u8>, CallError>>) -> Result<u64, CallError> {
        let mut state = self.state.lock().expect("the pending-call lock is healthy");
        if state.closed {
            return Err(CallError::Unavailable);
        }

        let id = state.next_id;
        state.next_id = state.next_id.checked_add(1).ok_or(CallError::Unavailable)?;
        state.waiters.insert(id, waiter);

        Ok(id)
    }

    fn answer(&self, id: u64, result: Result<Vec<u8>, CallError>) {
        let waiter = self.state.lock().expect("the pending-call lock is healthy").waiters.remove(&id);
        if let Some(waiter) = waiter {
            let _ = waiter.send(result);
        }
    }

    fn close(&self) {
        let waiters = {
            let mut state = self.state.lock().expect("the pending-call lock is healthy");
            state.closed = true;
            std::mem::take(&mut state.waiters)
        };
        self.capacity.close();
        for (_, waiter) in waiters {
            let _ = waiter.send(Err(CallError::Unavailable));
        }
    }
}

impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        let unanswered = self.pending.state.lock().expect("the pending-call lock is healthy").waiters.remove(&self.id).is_some();
        if unanswered && let Some(cancel) = self.cancel.take() {
            cancel.send(Outgoing::Frame(Frame::Cancel { id: self.id }));
        }
    }
}

impl Supervisor {
    pub(crate) fn new(outgoing: mpsc::Sender<Outgoing>, clock: fn() -> Option<i64>) -> Self {
        Self {
            outgoing,
            sequences: Arc::new(Mutex::new(BTreeMap::new())),
            pending: Arc::new(PendingCalls::new()),
            operation_context: None,
            generation_context: None,
            call: Arc::default(),
            clock,
        }
    }

    pub(crate) fn for_operation(&self, context: [u8; 32]) -> Self {
        let mut host = self.clone();
        host.operation_context = Some(context);
        host
    }

    pub(crate) fn for_attempt(
        &self, context: [u8; 32], values: Vec<harmos_domain::operations::CallValue>,
    ) -> Result<Self, harmos_domain::operations::ContextError> {
        let mut host = self.for_operation(context);
        host.call = Arc::new(harmos_domain::operations::CallContext::from_portable(values)?);
        Ok(host)
    }

    pub fn context(&self) -> &harmos_domain::operations::CallContext {
        &self.call
    }

    pub fn with_value<T: harmos_domain::operations::ContextValue>(
        &self, value: T,
    ) -> Result<Self, harmos_domain::operations::ContextError> {
        let mut child = self.clone();
        child.call = Arc::new(self.call.with_value(value)?);
        Ok(child)
    }

    // One child call on this attempt or generation; a submitted child answers
    // its instance id and stays owned by the attempt.
    pub(super) async fn child_call<O>(&self, request: &O, submit: bool) -> Result<Vec<u8>, CallError>
    where
        O: OperationDeclaration,
    {
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable);
        }
        self.exchange_with(|id, call| {
            let params = self.child::<O>(request, submit)?;
            let frame = Self::request_frame(id, CHILD_METHOD, params)?;
            call.send(Outgoing::Frame(frame));
            Ok::<(), CallError>(())
        })
        .await
    }

    // Drive a child this attempt submitted; the host resolves it only within
    // the same attempt context.
    pub(super) async fn child_control(&self, instance: Id, action: WireChildAction) -> Result<Vec<u8>, CallError> {
        let params = self.control_params(instance, action)?;
        self.exchange_with(|id, call| {
            let frame = Self::request_frame(id, CHILD_METHOD, params)?;
            call.send(Outgoing::Frame(frame));
            Ok::<(), CallError>(())
        })
        .await
    }

    // Send a control without awaiting its answer, for handles that are gone.
    pub(super) fn child_control_detached(&self, instance: Id, action: WireChildAction) {
        let Ok(params) = self.control_params(instance, action) else {
            return;
        };
        let (waiter, _) = oneshot::channel();
        let Ok(id) = self.pending.register(waiter) else {
            return;
        };
        let Ok(frame) = Self::request_frame(id, CHILD_METHOD, params) else {
            return;
        };
        let frame = match self.outgoing.try_send(Outgoing::Frame(frame)) {
            Ok(()) => return,
            Err(mpsc::error::TrySendError::Closed(_)) => return,
            Err(mpsc::error::TrySendError::Full(frame)) => frame,
        };
        // A Full Queue Still Delivers The Release Once Capacity Returns
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let outgoing = self.outgoing.clone();
            runtime.spawn(async move {
                let _ = outgoing.send(frame).await;
            });
        }
    }

    fn control_params(&self, instance: Id, action: WireChildAction) -> Result<Vec<u8>, CallError> {
        let context = self.operation_context.ok_or(CallError::Unavailable)?;
        let control = WireChildControl { context, instance: instance.value(), action };
        control.encode().map_err(|error| CallError::Malformed { method: CHILD_METHOD, message: error.to_string() })
    }

    fn child<O: OperationDeclaration>(&self, request: &O, submit: bool) -> Result<Vec<u8>, CallError> {
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable);
        }
        let context = self.operation_context.unwrap_or([0; 32]);
        let encoded = Frame::request_encoded_len::<O>(request);
        if encoded > Frame::LIMIT {
            return Err(CallError::Oversized { method: O::ID, encoded, limit: Frame::LIMIT });
        }
        let payload = harmos_domain::Message::encode_to_vec(request);
        let command = harmos_sdk::operations::WireChildCall {
            context,
            generation_context: self.generation_context,
            values: self.call.portable(),
            operation: O::ID.to_owned(),
            payload,
            submit,
        };
        command.encode().map_err(|error| CallError::Malformed { method: O::ID, message: error.to_string() })
    }

    fn request_frame(id: u64, method: &'static str, params: Vec<u8>) -> Result<Frame, CallError> {
        let mut frame = Frame::Call { id: u64::MAX, method: method.to_owned(), params };
        let encoded = frame.encoded_len();
        if encoded > Frame::LIMIT {
            return Err(CallError::Oversized { method, encoded, limit: Frame::LIMIT });
        }
        let Frame::Call { id: original, .. } = &mut frame else { unreachable!() };
        *original = id;
        Ok(frame)
    }

    async fn exchange_with<'a, E: From<CallError> + Send>(
        &'a self, prepare: impl FnOnce(u64, mpsc::Permit<'a, Outgoing>) -> Result<(), E> + Send,
    ) -> Result<Vec<u8>, E> {
        let permit = self.pending.capacity.acquire().await.map_err(|_| CallError::Unavailable)?;
        // Reserve both frames atomically so concurrent callers cannot hold all
        // queue capacity while waiting for a second slot. The cancel permit
        // stays with the call and makes cancellation reliable even under load.
        let mut frames = self.outgoing.reserve_many(2).await.map_err(|_| CallError::Unavailable)?;
        let call = frames.next().expect("two reserved frame slots");
        let cancel = frames.next().expect("two reserved frame slots");
        let (waiter, response) = oneshot::channel();
        let id = self.pending.register(waiter)?;
        let mut pending = PendingCall { pending: &self.pending, id, _permit: permit, cancel: Some(cancel) };

        if let Err(source) = prepare(id, call) {
            // No Call Was Sent, So Release The Reserved Cancel Without Sending It.
            pending.cancel.take();
            return Err(source);
        }
        response.await.map_err(|_| CallError::Unavailable)?.map_err(E::from)
    }

    pub(crate) fn receive_answer(&self, id: u64, result: Vec<u8>) {
        self.pending.answer(id, Ok(result));
    }

    pub(crate) fn receive_refusal(&self, id: u64, refusal: harmos_domain::Refusal) {
        let code = refusal.code();
        let failure = refusal.into_failure();
        self.pending.answer(id, Err(CallError::Refused { code, failure }));
    }

    pub(crate) fn disconnected(&self) {
        self.pending.close();
    }

    // The kit clock that stamps rows when they are produced.
    pub(super) fn clock(&self) -> fn() -> Option<i64> {
        self.clock
    }

    pub(super) fn now(&self) -> Option<i64> {
        (self.clock)()
    }

    // Publish rows in order through as few `host.publish` calls as fit the
    // frame and batch limits; each call awaits its answer before the next.
    // `stamps[i]` is the production time of `rows[i]`.
    pub(super) async fn publication<S>(
        &self, rows: &[S::Row], stamps: &[Option<i64>], context: Option<&str>,
    ) -> Result<(), super::PublishError>
    where
        S: crate::StreamSpec,
        S::Row: Message,
    {
        debug_assert_eq!(rows.len(), stamps.len());
        if rows.is_empty() {
            return Ok(());
        }
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable.into());
        }
        let mut rest = (rows, stamps);
        while !rest.0.is_empty() {
            let mut sent = 0;
            let bytes = self
                .exchange_with(|id, call| {
                    sent = self.with_publication::<S, super::PublishError>(rest.0, rest.1, context, |request| {
                        let params = self.child::<harmos_domain::operations::streams::Publish>(&request, false)?;
                        let frame = Self::request_frame(id, CHILD_METHOD, params)?;
                        call.send(Outgoing::Frame(frame));
                        Ok(())
                    })?;
                    Ok::<(), super::PublishError>(())
                })
                .await?;
            <() as Message>::decode(bytes.as_slice()).map_err(|source| CallError::Malformed {
                method: harmos_domain::operations::streams::Publish::ID,
                message: source.to_string(),
            })?;
            rest = (&rest.0[sent..], &rest.1[sent..]);
        }
        Ok(())
    }

    // Take the longest leading run of rows that fits one call, reserve its
    // contiguous sequences and send it under the sequence lock, so frames
    // leave in sequence order. A row too large on its own still spends its
    // sequence, exactly as a single-row publication does. The head carries
    // its stamp; each following row carries its delta from its predecessor,
    // and a stamp no delta can express starts the next call. A call carries
    // at most one window of rows, so the host never evicts its own rows.
    fn with_publication<S: crate::StreamSpec, E: From<PublicationError>>(
        &self, rows: &[S::Row], stamps: &[Option<i64>], context: Option<&str>,
        send: impl FnOnce(harmos_domain::operations::streams::Publish) -> Result<(), E>,
    ) -> Result<usize, E>
    where
        S::Row: Message,
    {
        let fits = |body: usize| Frame::request_payload_encoded_len(harmos_domain::operations::streams::Publish::ID, body) <= Frame::LIMIT;
        let mut sequences = self.sequences.lock().expect("the publication sequence lock is healthy");
        let next = sequences.entry(S::ID).or_insert(0);
        let first = next.checked_add(1).ok_or(PublicationError::SequenceExhausted { stream: S::ID })?;
        *next = first;
        let published_unix_micros = stamps[0];
        let head = crate::wire::publication_encoded_len(S::ID, S::VERSION, first, rows[0].encoded_len(), published_unix_micros, context);
        let encoded = |count: usize, following: usize| head.saturating_add(crate::wire::varint_len(count as u64)).saturating_add(following);
        if !fits(encoded(0, 0)) {
            return Err(PublicationError::Oversized { encoded: encoded(0, 0), limit: Frame::LIMIT }.into());
        }
        let mut following = 0_usize;
        let mut deltas = Vec::with_capacity(rows.len() - 1);
        for (row, pair) in rows[1..].iter().zip(stamps.windows(2)) {
            if deltas.len() + 1 >= S::WINDOW {
                break;
            }
            let delta = match (pair[0], pair[1]) {
                (_, None) => None,
                (Some(previous), Some(stamp)) => match stamp.checked_sub(previous) {
                    Some(delta) => Some(delta),
                    None => break,
                },
                (None, Some(_)) => break,
            };
            let grown = following.saturating_add(crate::wire::following_encoded_len(row.encoded_len(), delta));
            let total = encoded(deltas.len() + 1, grown);
            if total > PUBLICATION_BATCH_BYTES || !fits(total) || *next == u64::MAX {
                break;
            }
            following = grown;
            deltas.push(delta);
            *next += 1;
        }
        let taken = deltas.len() + 1;
        send(harmos_domain::operations::streams::Publish {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: Some(first),
            published_unix_micros,
            context: context.map(str::to_owned),
            payload: rows[0].encode_to_vec(),
            following: rows[1..taken]
                .iter()
                .zip(deltas)
                .map(|(row, published_delta_micros)| harmos_domain::operations::streams::FollowingRow {
                    payload: row.encode_to_vec(),
                    published_delta_micros,
                })
                .collect(),
        })?;
        Ok(taken)
    }

    pub fn exiting(&self, reason: impl Into<String>) -> bool {
        self.outgoing.try_send(Outgoing::Exiting(reason.into())).is_ok()
    }

    pub fn abandon(&self) -> bool {
        self.outgoing.try_send(Outgoing::Abandon).is_ok()
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn refused_line(code: i64, failure: &harmos_domain::Failure) -> String {
    match failure.context().operation() {
        Some(operation) => format!("operation `{operation}` failed"),
        None => format!("the host refused the call ({code})"),
    }
}

pub(super) fn published_at() -> Option<i64> {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_micros()).ok(),
        Err(error) => i64::try_from(error.duration().as_micros()).ok().and_then(i64::checked_neg),
    }
}

/* === Tests === */
