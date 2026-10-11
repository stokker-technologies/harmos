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
use harmos_contract::Message;
use harmos_contract::operations::OperationDeclaration;

/* === Constants === */
const PENDING_CALLS: usize = 256;

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
    call: Arc<harmos_contract::operations::CallContext>,
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
    #[error("the host refused the call ({code}): {message}")]
    Refused { code: i64, message: String },
    #[error("the host connection is unavailable")]
    Unavailable,
    #[error("the host returned a malformed reply for `{method}`: {message}")]
    Malformed {
        method: &'static str,
        message: String,
    },
    #[error("the request for `{method}` requires {encoded} bytes; the frame limit is {limit}")]
    Oversized {
        method: &'static str,
        encoded: usize,
        limit: usize,
    },
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
impl harmos_contract::plugin::Context for Supervisor {
    fn with_generation(&self, token: [u8; 32]) -> Self {
        let mut host = self.clone();
        host.generation_context = Some(token);
        host
    }
}

impl PendingCalls {
    fn new() -> Self {
        Self {
            state: Mutex::new(PendingState {
                closed: false,
                next_id: 1,
                waiters: HashMap::new(),
            }),
            capacity: Semaphore::new(PENDING_CALLS),
        }
    }

    fn register(
        &self,
        waiter: oneshot::Sender<Result<Vec<u8>, CallError>>,
    ) -> Result<u64, CallError> {
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
        let waiter = self
            .state
            .lock()
            .expect("the pending-call lock is healthy")
            .waiters
            .remove(&id);
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
        let unanswered = self
            .pending
            .state
            .lock()
            .expect("the pending-call lock is healthy")
            .waiters
            .remove(&self.id)
            .is_some();
        if unanswered && let Some(cancel) = self.cancel.take() {
            cancel.send(Outgoing::Frame(Frame::Cancel { id: self.id }));
        }
    }
}

impl Supervisor {
    pub(crate) async fn observer_update(&self, update: crate::ObserverUpdate) -> bool {
        let params = serde_json::to_vec(&update).expect("observer state serializes");
        self.outgoing
            .send(Outgoing::Frame(Frame::Push {
                method: Frame::OBSERVER_STATUS.to_owned(),
                params,
            }))
            .await
            .is_ok()
    }

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
        &self,
        context: [u8; 32],
        values: Vec<harmos_contract::operations::CallValue>,
    ) -> Result<Self, harmos_contract::operations::ContextError> {
        let mut host = self.for_operation(context);
        host.call = Arc::new(harmos_contract::operations::CallContext::from_portable(
            values,
        )?);
        Ok(host)
    }

    pub fn context(&self) -> &harmos_contract::operations::CallContext {
        &self.call
    }

    pub(crate) fn for_observer_context(&self, context: [u8; 32]) -> Result<Self, crate::Refusal> {
        if context == [0; 32] {
            return Err(crate::Refusal::of("observer context must be host-issued"));
        }
        Ok(self.for_operation(context))
    }

    pub async fn operation<O>(&self, request: O::Request) -> Result<O::Response, CallError>
    where
        O: harmos_contract::operations::OperationDeclaration,
        O::Request: Message,
        O::Response: Message,
    {
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable);
        }
        let bytes = self
            .exchange_with(|id, call| {
                let params = self.child::<O>(&request)?;
                let frame =
                    Self::request_frame(id, harmos_contract::operations::CHILD_METHOD, params)?;
                call.send(Outgoing::Frame(frame));
                Ok::<(), CallError>(())
            })
            .await?;
        O::Response::decode(bytes.as_slice()).map_err(|error| CallError::Malformed {
            method: O::ID,
            message: error.to_string(),
        })
    }

    fn child<O: OperationDeclaration>(&self, request: &O::Request) -> Result<Vec<u8>, CallError>
    where
        O::Request: Message,
    {
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable);
        }
        let context = self.operation_context.unwrap_or([0; 32]);
        let encoded = Frame::request_encoded_len::<O>(request);
        if encoded > Frame::LIMIT {
            return Err(CallError::Oversized {
                method: O::ID,
                encoded,
                limit: Frame::LIMIT,
            });
        }
        let payload = request.encode_to_vec();
        let command = harmos_contract::operations::WireChildCall {
            context,
            generation_context: self.generation_context,
            operation: O::ID.to_owned(),
            payload,
        };
        command.encode().map_err(|error| CallError::Malformed {
            method: O::ID,
            message: error.to_string(),
        })
    }

    fn request_frame(id: u64, method: &'static str, params: Vec<u8>) -> Result<Frame, CallError> {
        let mut frame = Frame::Call {
            id: u64::MAX,
            method: method.to_owned(),
            params,
        };
        let encoded = frame.encoded_len();
        if encoded > Frame::LIMIT {
            return Err(CallError::Oversized {
                method,
                encoded,
                limit: Frame::LIMIT,
            });
        }
        let Frame::Call { id: original, .. } = &mut frame else {
            unreachable!()
        };
        *original = id;
        Ok(frame)
    }

    async fn exchange_with<'a, E: From<CallError> + Send>(
        &'a self,
        prepare: impl FnOnce(u64, mpsc::Permit<'a, Outgoing>) -> Result<(), E> + Send,
    ) -> Result<Vec<u8>, E> {
        let permit = self
            .pending
            .capacity
            .acquire()
            .await
            .map_err(|_| CallError::Unavailable)?;
        // Reserve both frames atomically so concurrent callers cannot hold all
        // queue capacity while waiting for a second slot. The cancel permit
        // stays with the call and makes cancellation reliable even under load.
        let mut frames = self
            .outgoing
            .reserve_many(2)
            .await
            .map_err(|_| CallError::Unavailable)?;
        let call = frames.next().expect("two reserved frame slots");
        let cancel = frames.next().expect("two reserved frame slots");
        let (waiter, response) = oneshot::channel();
        let id = self.pending.register(waiter)?;
        let mut pending = PendingCall {
            pending: &self.pending,
            id,
            _permit: permit,
            cancel: Some(cancel),
        };

        if let Err(source) = prepare(id, call) {
            // No Call Was Sent, So Release The Reserved Cancel Without Sending It.
            pending.cancel.take();
            return Err(source);
        }
        response
            .await
            .map_err(|_| CallError::Unavailable)?
            .map_err(E::from)
    }

    pub(crate) fn receive_answer(&self, id: u64, result: Vec<u8>) {
        self.pending.answer(id, Ok(result));
    }

    pub(crate) fn receive_refusal(&self, id: u64, code: i64, message: String) {
        self.pending
            .answer(id, Err(CallError::Refused { code, message }));
    }

    pub(crate) fn disconnected(&self) {
        self.pending.close();
    }

    pub(super) async fn publication<S>(
        &self,
        row: &S::Row,
        context: Option<&str>,
    ) -> Result<(), super::PublishError>
    where
        S: crate::StreamSpec,
        S::Row: Message,
    {
        use harmos_contract::operations::{CHILD_METHOD, host};
        if self.operation_context.is_none() && self.generation_context.is_none() {
            return Err(CallError::Unavailable.into());
        }
        let bytes = self
            .exchange_with(|id, call| {
                self.with_publication::<S, _, super::PublishError>(row, context, |request| {
                    let params = self.child::<host::Publish>(&request)?;
                    let frame = Self::request_frame(id, CHILD_METHOD, params)?;
                    call.send(Outgoing::Frame(frame));
                    Ok(())
                })
            })
            .await?;
        <() as Message>::decode(bytes.as_slice()).map_err(|source| {
            CallError::Malformed {
                method: host::Publish::ID,
                message: source.to_string(),
            }
            .into()
        })
    }

    fn with_publication<S: crate::StreamSpec, T, E: From<PublicationError>>(
        &self,
        row: &S::Row,
        context: Option<&str>,
        finish: impl FnOnce(harmos_contract::operations::host::Publication) -> Result<T, E>,
    ) -> Result<T, E>
    where
        S::Row: Message,
    {
        let mut sequences = self
            .sequences
            .lock()
            .expect("the publication sequence lock is healthy");
        let next = sequences.entry(S::ID).or_insert(0);
        let sequence = next
            .checked_add(1)
            .ok_or(PublicationError::SequenceExhausted { stream: S::ID })?;
        *next = sequence;
        let published_unix_micros = (self.clock)();
        let body = crate::wire::publication_encoded_len(
            S::ID,
            S::VERSION,
            sequence,
            row.encoded_len(),
            published_unix_micros,
            context,
        );
        let encoded = body;
        if encoded > Frame::LIMIT {
            return Err(PublicationError::Oversized {
                encoded,
                limit: Frame::LIMIT,
            }
            .into());
        }
        finish(harmos_contract::operations::host::Publication {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: Some(sequence),
            published_unix_micros,
            context: context.map(str::to_owned),
            payload: row.encode_to_vec(),
        })
    }

    pub fn exiting(&self, reason: impl Into<String>) -> bool {
        self.outgoing
            .try_send(Outgoing::Exiting(reason.into()))
            .is_ok()
    }

    pub fn abandon(&self) -> bool {
        self.outgoing.try_send(Outgoing::Abandon).is_ok()
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(super) fn published_at() -> Option<i64> {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_micros()).ok(),
        Err(error) => i64::try_from(error.duration().as_micros())
            .ok()
            .and_then(i64::checked_neg),
    }
}

/* === Tests === */
