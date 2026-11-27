/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Child Call Queues

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
    mem,
    num::NonZeroUsize,
    result,
    sync::{Arc, Mutex, atomic::Ordering},
};

// Third Party Imports
use tokio::sync::{Notify, mpsc, oneshot};
use tonic::Status;

// Local Imports
use super::{ChildOutput, ChildTransport, Context};
use crate::{
    Error, OperationRequest, Result,
    message::{self, request_fingerprint},
};
use harmos_protocol::model::{Classification, Failure};
use harmos_protocol::plugin::{ChildCancel, ChildDemand, ChildRequest, instance_invoke_response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type ChildResult = result::Result<Vec<u8>, Error>;
type WatchResult = Result<Option<Vec<u8>>>;
pub(crate) type Pending = BTreeMap<u64, PendingCall>;

/* === Structs === */

#[derive(Clone, Default)]
pub(crate) struct Queue {
    pub child: Option<Arc<dyn ChildTransport>>,
    pub calls: Arc<Mutex<Calls>>,
    pub output: Option<ChildOutput>,
    pub call_changes: Arc<Notify>,
}

pub(crate) struct PendingCall {
    pub sender: oneshot::Sender<ChildResult>,
    pub sent: bool,
}

#[derive(Clone)]
pub(crate) struct WatchCall {
    pub sender: mpsc::Sender<WatchResult>,
    pub limit: usize,
    pub sent: bool,
}

struct PendingDrop {
    context: Context,
    call: u64,
}

#[derive(Default)]
pub(crate) struct Calls {
    next: u64,
    pub pending: Pending,
    pub watches: BTreeMap<u64, WatchCall>,
    pub cancelled: BTreeSet<u64>,
    pub cancel_queue: BTreeSet<u64>,
}

/// A Child Watch Holds at Most One Demanded Item; Dropping It Cancels Its Demand.
#[must_use]
pub struct Watching<R: message::Message> {
    context: Context,
    pub(super) call: u64,
    receiver: mpsc::Receiver<WatchResult>,
    closed: bool,
    response: PhantomData<fn() -> R>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for PendingDrop {
    fn drop(&mut self) {
        let mut calls = self.context.queue.calls.lock().expect("Child Calls Poisoned");
        if let Some(pending) = calls.pending.remove(&self.call)
            && pending.sent
        {
            calls.cancelled.insert(self.call);
            calls.cancel_queue.insert(self.call);
            self.context.queue.call_changes.notify_one();
        }
    }
}

impl Context {
    pub(crate) fn request_wind_down(&self) {
        self.wind_down.store(true, Ordering::Release);
    }

    pub(crate) fn cancel_failure(&self, reason: String) -> Error {
        Failure {
            classification: Classification::Cancelled as i32,
            code: "cancelled".into(),
            message: reason,
            action: "Retry If Needed".into(),
            operation: self.operation.clone(),
            plugin: self.plugin.clone(),
            ..Default::default()
        }
        .into()
    }

    /// Open a declared context Watch with a consumer-owned maximum encoded item size.
    pub async fn watch<Q: OperationRequest>(&self, request: Q, max_item_bytes: NonZeroUsize) -> Result<Watching<Q::Response>> {
        if !Q::WATCH {
            return Err(Error::Invalid(format!("Watch {}: Server-Streaming Contract Required", Q::ID)));
        }
        let output = self.queue.output.as_ref().ok_or_else(|| Error::Invalid(format!("Watch {}: Child Carrier Required", Q::ID)))?;
        output.check()?;
        let fingerprint = request_fingerprint::<Q>()?;
        if self.requires.get(Q::ID) != Some(&fingerprint) {
            return Err(Error::Invalid(format!("Watch {}: Undeclared Requirement", Q::ID)));
        }
        let (sender, receiver) = mpsc::channel(1);
        let call = {
            let mut calls = self.queue.calls.lock().expect("Child Calls Poisoned");
            calls.next += 1;
            let call = calls.next;
            calls.watches.insert(
                call,
                WatchCall {
                    sender,
                    limit: max_item_bytes.get(),
                    sent: false,
                },
            );
            call
        };
        let watching = Watching {
            context: self.clone(),
            call,
            receiver,
            closed: false,
            response: PhantomData,
        };
        let limit = self.queue.calls.lock().expect("Child Calls Poisoned").watches[&call].limit;
        output
            .send(instance_invoke_response::Frame::Child(ChildRequest {
                call,
                operation: Q::ID.into(),
                payload: request.encode_to_vec(),
                fingerprint,
                watch: true,
                max_response_bytes: limit as u64,
                ..Default::default()
            }))
            .await?;
        if let Some(watch) = self.queue.calls.lock().expect("Child Calls Poisoned").watches.get_mut(&call) {
            watch.sent = true;
        }
        Ok(watching)
    }

    pub(super) async fn send_call(&self, mut child: ChildRequest) -> Result<Vec<u8>> {
        if let Some(transport) = &self.queue.child {
            return transport.call(child, self.values.clone(), false).await;
        }
        let operation = &child.operation;
        let output = self.queue.output.as_ref().ok_or_else(|| Error::Invalid(format!("Call {operation}: Child Carrier Required")))?;
        output.check()?;
        if let Some(reason) = self.cancellation.borrow().clone() {
            return Err(self.cancel_failure(reason));
        }
        self.flush_cancels().await?;
        let (send, receive) = oneshot::channel();
        let call = {
            let mut calls = self.queue.calls.lock().expect("Child Calls Poisoned");
            calls.next += 1;
            let call = calls.next;
            calls.pending.insert(call, PendingCall { sender: send, sent: false });
            call
        };
        let _pending = PendingDrop { context: self.clone(), call };
        child.call = call;
        output.send(instance_invoke_response::Frame::Child(child)).await?;
        if let Some(pending) = self.queue.calls.lock().expect("Child Calls Poisoned").pending.get_mut(&call) {
            pending.sent = true;
        }
        receive.await.map_err(|_| Status::unavailable("Invocation Closed"))?
    }

    pub(crate) async fn flush_cancels(&self) -> Result<()> {
        let cancelled = mem::take(&mut self.queue.calls.lock().expect("Child Calls Poisoned").cancel_queue);
        if let Some(output) = &self.queue.output {
            for call in cancelled {
                output.send(instance_invoke_response::Frame::ChildCancel(ChildCancel { call })).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn cancel_calls(&self, reason: &str) -> Result<()> {
        let (pending, watches) = {
            let mut calls = self.queue.calls.lock().expect("Child Calls Poisoned");
            let pending = mem::take(&mut calls.pending);
            let watches = mem::take(&mut calls.watches);
            let sent = pending.iter().filter(|(_, call)| call.sent).map(|(id, _)| *id);
            for id in sent.chain(watches.iter().filter(|(_, watch)| watch.sent).map(|(id, _)| *id)) {
                calls.cancelled.insert(id);
                calls.cancel_queue.insert(id);
            }
            (pending, watches)
        };
        for call in pending.into_values() {
            let _ = call.sender.send(Err(self.cancel_failure(reason.into())));
        }
        for watch in watches.into_values() {
            let _ = watch.sender.try_send(Err(self.cancel_failure(reason.into())));
        }
        self.flush_cancels().await
    }
}

impl<R: message::Message> Watching<R> {
    pub fn close(&mut self) {
        self.closed = true;
        let mut calls = self.context.queue.calls.lock().expect("Child Calls Poisoned");
        if let Some(watch) = calls.watches.remove(&self.call)
            && watch.sent
        {
            calls.cancelled.insert(self.call);
            calls.cancel_queue.insert(self.call);
            self.context.queue.call_changes.notify_one();
        }
    }

    pub async fn next(&mut self) -> Result<Option<R>> {
        if self.closed {
            return Ok(None);
        }
        let output = self.context.queue.output.as_ref().expect("Native Watch");
        let active = self.context.queue.calls.lock().expect("Child Calls Poisoned").watches.contains_key(&self.call);
        if active {
            output.send(instance_invoke_response::Frame::ChildDemand(ChildDemand { call: self.call })).await?;
        }
        match self.receiver.recv().await {
            Some(Ok(Some(bytes))) => R::decode(bytes.as_slice()).map(Some).map_err(|error| Error::Invalid(format!("Watch: {error}"))),
            Some(Ok(None)) => {
                self.closed = true;
                Ok(None)
            }
            Some(Err(error)) => {
                self.closed = true;
                Err(error)
            }
            None => Err(Status::unavailable("Watch: Invocation Closed").into()),
        }
    }
}
impl<R: message::Message> Drop for Watching<R> {
    fn drop(&mut self) {
        self.close();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
