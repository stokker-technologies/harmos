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
use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports
use futures_util::{StreamExt, stream::FuturesUnordered};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::Instrument;
use wasmtime::AsContextMut;
// Local Imports
use super::{Active, Error};
use crate::Application;
use crate::operations::children::Children;
use crate::operations::{OperationDefinition, WireAnswer, WireChildCall, WireCommand};
use crate::plugin::PluginEndpoint;
use crate::supervision::executor;
use crate::supervision::executor::InContext;

/* === Constants === */

const CALLS: usize = 128;

/* === Macros === */

/* === Type Aliases === */

type Contexts = Arc<Mutex<BTreeMap<[u8; 32], Children>>>;
type Running<A> = JoinHandle<(Owned<A>, Result<(), Error>)>;

/* === Structs === */

struct Called<'a> {
    future: Pin<Box<dyn Future<Output = bool> + Send + 'a>>,
    admission: bool,
}

#[derive(Default)]
struct Loan {
    attempts: Vec<Retired>,
    background: Option<Arc<crate::supervision::instance::Instance>>,
}

pub(crate) struct Endpoint {
    definitions: Vec<OperationDefinition>,
    scope: Option<crate::plugin::PluginScope>,
    accepting: AtomicBool,
    requests: mpsc::Sender<Request>,
    ended: watch::Receiver<bool>,
    failure: Mutex<Option<Arc<wasmtime::Error>>>,
}

pub(super) struct Session<A: Application> {
    endpoint: Arc<Endpoint>,
    stopping: watch::Sender<bool>,
    running: Option<Running<A>>,
    ending: Option<Owned<A>>,
    instance: Option<PluginEndpoint>,
    closing: bool,
    _application: PhantomData<fn() -> A>,
}

struct Owned<A: Application> {
    guest: Option<Active<A>>,
    endpoint: Arc<Endpoint>,
    ended: watch::Sender<bool>,
    loans: Arc<Mutex<BTreeMap<u64, Loan>>>,
}

struct Request {
    trace: crate::supervision::executor::SpanContext,
    call: Command,
    children: Option<Children>,
    answer: oneshot::Sender<Result<Reply, Error>>,
}

struct Retired {
    contexts: Contexts,
    token: [u8; 32],
}

/* === Enums === */

enum Command {
    Operation(WireCommand),
    Background { call: WireChildCall, submitted: tokio::time::Instant },
    Close(Option<PluginEndpoint>),
}

enum Reply {
    Operation(WireAnswer),
    Background(Vec<u8>),
}

enum Ending {
    Stopped,
    Closing(Option<PluginEndpoint>),
}

/* === Traits === */

/* === Implementations === */

impl Future for Called<'_> {
    type Output = bool;
    fn poll(mut self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<bool> {
        self.future.as_mut().poll(context)
    }
}

impl Endpoint {
    pub(crate) fn accepts(&self) -> bool {
        self.accepting.load(Ordering::Acquire)
    }

    pub(crate) fn scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.scope.as_ref()
    }

    pub(crate) fn definitions(&self) -> &[OperationDefinition] {
        &self.definitions
    }

    pub(crate) async fn phase(&self, command: WireCommand, children: Option<Children>) -> Result<WireAnswer, Error> {
        let reply = self.request(Command::Operation(command), children).await?;
        match reply {
            Reply::Operation(answer) => Ok(answer),
            Reply::Background(_) => unreachable!("operation replies retain their request kind"),
        }
    }

    pub(super) async fn background(&self, call: WireChildCall, submitted: tokio::time::Instant) -> Result<Vec<u8>, Error> {
        if !call
            .generation_context
            .as_ref()
            .is_some_and(|token| self.scope.as_ref().is_some_and(|scope| scope.accepts_generation_context(token)))
        {
            return Err(Error::OperationCaller(crate::Error::Closing { domain: "guest generation context" }));
        }
        match self.request(Command::Background { call, submitted }, None).await? {
            Reply::Background(payload) => Ok(payload),
            Reply::Operation(_) => unreachable!("background replies retain their request kind"),
        }
    }

    async fn request(&self, call: Command, children: Option<Children>) -> Result<Reply, Error> {
        if !self.accepts() {
            return Err(unavailable());
        }
        let (answer, answered) = oneshot::channel();
        let admission = self
            .requests
            .reserve()
            .instrument(tracing::debug_span!(target: "harmos::carrier", "guest.admission_wait"))
            .await
            .map_err(|_| unavailable())?;
        let request = Request {
            call,
            children,
            answer,
            trace: crate::supervision::executor::SpanContext::new(
                tracing::Span::current(),
                tracing::debug_span!(target: "harmos::carrier", "guest.queue"),
            ),
        };
        admission.send(request);
        answered.await.map_err(|_| {
            self.failure.lock().expect("guest terminal failure lock").clone().map_or_else(unavailable, |source| Error::Execution { source })
        })?
    }

    pub(crate) async fn settle(&self) -> Result<(), Error> {
        let mut ended = self.ended.clone();
        ended.wait_for(|ended| *ended).await.map_err(|_| unavailable())?;
        Ok(())
    }
}

impl<A: Application> Session<A> {
    pub(super) fn start(mut guest: Active<A>) -> Self {
        let (requests, incoming) = mpsc::channel(CALLS);
        let (ended, receipt) = watch::channel(false);
        let definitions = guest.store.data().declaration.operations.clone();
        let endpoint = Arc::new(Endpoint {
            scope: guest.store.data().scope.clone(),
            definitions,
            accepting: AtomicBool::new(true),
            requests,
            ended: receipt,
            failure: Mutex::new(None),
        });
        guest.store.data_mut().session = Some(Arc::downgrade(&endpoint));
        let owned = Owned { guest: Some(guest), endpoint: Arc::clone(&endpoint), ended, loans: Arc::new(Mutex::new(BTreeMap::new())) };
        let (stopping, stopped) = watch::channel(false);
        let running = executor::task(async move {
            let mut owned = owned;
            let result = {
                let running = owned.run(incoming, stopped);
                let mut running = std::pin::pin!(running);
                std::future::poll_fn(|context| {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| running.as_mut().poll(context))) {
                        Ok(poll) => poll.map(Ok),
                        Err(_) => std::task::Poll::Ready(Err(())),
                    }
                })
                .await
            };
            let result = match result {
                Ok(result) => result,
                Err(()) => {
                    let source = Arc::new(wasmtime::Error::msg("guest session machinery panicked"));
                    *owned.endpoint.failure.lock().expect("guest terminal failure lock") = Some(Arc::clone(&source));
                    owned.endpoint.accepting.store(false, Ordering::Release);
                    // A Poisoned Store Cannot Be Re-Entered; Its Host Debts Stay Owned.
                    owned.destroy_store();
                    Err(Error::Execution { source })
                }
            };
            (owned, result)
        });
        Self { endpoint, stopping, running: Some(running), ending: None, instance: None, closing: false, _application: PhantomData }
    }

    pub(super) fn endpoint(&self) -> Arc<Endpoint> {
        Arc::clone(&self.endpoint)
    }

    pub(super) async fn close(&mut self, instance: Option<PluginEndpoint>) -> Result<(), Error> {
        self.endpoint.accepting.store(false, Ordering::Release);
        if !self.closing {
            self.instance = instance;
            if !*self.endpoint.ended.borrow() {
                let (answer, _answered) = oneshot::channel();
                let admission = self
                    .endpoint
                    .requests
                    .reserve()
                    .instrument(tracing::debug_span!(target: "harmos::carrier", "guest.admission_wait"))
                    .await;
                if let Ok(admission) = admission {
                    let request = Request {
                        call: Command::Close(self.instance.take()),
                        children: None,
                        answer,
                        trace: crate::supervision::executor::SpanContext::new(
                            tracing::Span::current(),
                            tracing::debug_span!(target: "harmos::carrier", "guest.queue"),
                        ),
                    };
                    admission.send(request);
                }
            }
            self.closing = true;
        }
        if let Some(running) = &mut self.running {
            let (owned, result) = running.await.map_err(|source| Error::Guest { message: source.to_string() })?;
            self.running.take();
            let mut owned = owned;
            if let Some(guest) = &mut owned.guest
                && self.instance.is_some()
            {
                guest.routing = self.instance.take();
            }
            self.ending = Some(owned);
            // A Failed Cleanup Returns Its Exact Store Owner, Never Settlement.
            result?;
        }
        if let Some(ending) = &mut self.ending {
            ending.finish().await?;
        }
        self.ending.take();
        self.instance.take();
        Ok(())
    }
}

impl<A: Application> Drop for Session<A> {
    fn drop(&mut self) {
        self.endpoint.accepting.store(false, Ordering::Release);
        self.stopping.send_replace(true);
    }
}

impl Drop for Retired {
    fn drop(&mut self) {
        self.contexts.lock().expect("guest child context lock").remove(&self.token);
    }
}

impl<A: Application> Drop for Owned<A> {
    fn drop(&mut self) {
        // Destroy The Original Store Before Publishing Physical Settlement
        let guest = self.guest.take();
        drop(guest);
        let loans = std::mem::take(&mut *self.loans.lock().expect("guest invocation loan lock"));
        drop(loans);
        self.endpoint.accepting.store(false, Ordering::Release);
        self.ended.send_replace(true);
    }
}

impl<A: Application> Owned<A> {
    async fn run(&mut self, mut incoming: mpsc::Receiver<Request>, mut stopped: watch::Receiver<bool>) -> Result<(), Error> {
        let guest = self.guest.as_mut().expect("owned guest store");
        let operate = guest.sdk.func_operate();
        let contexts = Arc::clone(&guest.store.data().children);
        let loans = Arc::clone(&self.loans);
        let fuel = guest.fuel;
        let caller = guest.store.data().caller.clone();
        let scope = guest.store.data().scope.clone();
        // The Operation Manager Owns Operation Time Policy
        guest.store.set_epoch_deadline(super::UNBOUNDED);
        let ending = guest.store.run_concurrent(async |accessor| {
            let mut pending: FuturesUnordered<Called<'_>> = FuturesUnordered::new();
            let ending = loop {
                tokio::select! {
                    biased;
                    _ = stopped.wait_for(|stopped| *stopped) => break Ending::Stopped,
                    completed = pending.next(), if !pending.is_empty() => {
                        if completed == Some(true) { break Ending::Stopped; }
                    }
                    request = incoming.recv() => {
                        let Some(mut request) = request else { break Ending::Stopped; };
                        let mut trace = request.trace;
                        trace.dequeued();
                        if let Command::Close(instance) = request.call { break Ending::Closing(instance); }
                        if request.answer.is_closed() { continue; }
                        let background_debts = loans.lock().expect("guest invocation loan lock").values().filter(|loan| loan.background.is_some()).count();
                        let finite = pending.iter().filter(|call| call.admission).count();
                        // Finalization Borrows An Already Admitted Running's Receipt.
                        // It Must Progress Even When Retained Background Debts Fill Admission.
                        let finalization = matches!(&request.call, Command::Operation(WireCommand::Finalize { .. }));
                        if !finalization && finite + background_debts >= CALLS {
                            let _ = request.answer.send(Err(crate::plugin::DispatchError::Busy.into()));
                            continue;
                        }
                        if let Command::Background { call, submitted } = request.call {
                            let admitted = if call.generation_context.as_ref().is_some_and(|token| scope.as_ref().is_some_and(|scope| scope.accepts_generation_context(token))) {
                                caller.as_ref().ok_or(crate::Error::Closing { domain: "guest generation context" })
                                    .and_then(|caller| caller.submit_encoded_at(&call.operation, call.fingerprint, call.generation, call.payload, submitted, call.values))
                            } else { Err(crate::Error::Closing { domain: "guest generation context" }) };
                            let invocation = match admitted {
                                Ok(invocation) => invocation,
                                Err(error) => { let _ = request.answer.send(Err(Error::OperationCaller(error))); continue; }
                            };
                            let id = invocation.id().value();
                            let control = Arc::clone(&invocation.control);
                            loans.lock().expect("guest invocation loan lock").entry(id).or_default().background = Some(Arc::clone(&control));
                            let retained = Arc::clone(&loans);
                            let future = async move {
                                let (_cancel, cancelled) = oneshot::channel();
                                let result = tokio::select! {
                                    biased;
                                    _ = request.answer.closed() => None,
                                    result = crate::operations::children::lifecycle(invocation, cancelled) => Some(result),
                                };
                                if result.is_some()
                                    && !matches!(&result, Some(Err(crate::Error::OperationSettlement { .. })))
                                    && control.is_settled()
                                {
                                    let retired = {
                                        let mut retained = retained.lock().expect("guest invocation loan lock");
                                        let loan = retained.get_mut(&id).expect("original background receipt");
                                        let control = loan.background.take();
                                        if loan.attempts.is_empty() { retained.remove(&id); }
                                        control
                                    };
                                    drop(retired);
                                }
                                if let Some(result) = result {
                                    let _ = request.answer.send(result.map(Reply::Background).map_err(Error::OperationCaller));
                                }
                                false
                            };
                            pending.push(Called { future: Box::pin(future.instrument(trace.span.clone()).in_context(trace.dispatcher.clone())), admission: false });
                            continue;
                        }
                        if let Some(scope) = request.children.take() {
                            let Command::Operation(WireCommand::Initialize { invocation, .. } | WireCommand::Attempt { invocation, .. } | WireCommand::StreamAttempt { invocation, .. }) = &request.call else {
                                let _ = request.answer.send(Err(Error::Guest { message: "guest child context requires initialization or an attempt".to_owned() }));
                                continue;
                            };
                            let token = scope.context;
                            let mut scopes = contexts.lock().expect("guest child context lock");
                            if scopes.contains_key(&token) {
                                let _ = request.answer.send(Err(Error::Guest { message: "guest attempt context is already active".to_owned() }));
                                continue;
                            }
                            scopes.insert(token, scope);
                            drop(scopes);
                            loans.lock().expect("guest invocation loan lock")
                                .entry(*invocation).or_default().attempts
                                .push(Retired { contexts: Arc::clone(&contexts), token });
                        }
                        let context = accessor;
                        let refill = pending.is_empty();
                        let retained = Arc::clone(&loans);
                        let finalizing = match &request.call {
                            Command::Operation(WireCommand::Finalize { invocation }) => Some(*invocation),
                            _ => None,
                        };
                        let owner = scope.as_ref().map(|scope| scope.owner().to_owned());
                        let future = async move {
                            let called = async {
                                if refill {
                                    context.with(|mut access| access.as_context_mut().set_fuel(fuel)).map_err(|source| Error::Execution { source: Arc::new(source) })?;
                                }
                                match request.call {
                                    Command::Operation(command) => {
                                        let function = operate;
                                        let payload = command.encode().map_err(|source| Error::Malformed { method: "operate".to_owned(), message: source.to_string() })?;
                                        let owner = owner.clone().ok_or_else(|| Error::Malformed { method: "operate".to_owned(), message: "operation has no assigned owner".to_owned() })?;
                                        let (reply,) = function.call_concurrent(context, (owner, payload)).await.map_err(|source| Error::Execution { source: Arc::new(source) })?;
                                        let reply = reply.map_err(|refusal| Error::Operation { refusal: super::refused(refusal) })?;
                                        let answer = WireAnswer::decode(&reply).map_err(|source| Error::Malformed { method: "operate".to_owned(), message: source.to_string() })?;
                                        Ok(Reply::Operation(answer))
                                    }
                                    Command::Close(_) | Command::Background { .. } => unreachable!("control and background work belong to the session owner"),
                                }
                            };
                            let result = tokio::select! {
                                biased;
                                _ = request.answer.closed() => None,
                                result = called => Some(result),
                            };
                            // Retain The Original Loan Until SDK State Physically Finalizes
                            if matches!(&result, Some(Ok(Reply::Operation(WireAnswer::Ready { payload }))) if payload.is_empty())
                                && let Some(invocation) = finalizing
                            {
                                let retired = {
                                    let mut retained = retained.lock().expect("guest invocation loan lock");
                                    let loan = retained.get_mut(&invocation);
                                    let retired = loan.map(|loan| std::mem::take(&mut loan.attempts));
                                    if retained.get(&invocation).is_some_and(|loan| loan.background.is_none()) { retained.remove(&invocation); }
                                    retired
                                };
                                drop(retired);
                            }
                            let failed = result.is_none() || result.as_ref().is_some_and(|result| matches!(result, Err(Error::Execution { .. })));
                            if let Some(result) = result { let _ = request.answer.send(result); }
                            failed
                        };
                        pending.push(Called { future: Box::pin(future.instrument(trace.span.clone()).in_context(trace.dispatcher.clone())), admission: !finalization });
                    }
                }
            };
            // Cancel Calls And Drive Their Guest Tasks Through Physical Exit
            drop(pending);
            if matches!(ending, Ending::Stopped) { return ending; }
            tokio::select! {
                biased;
                _ = stopped.wait_for(|stopped| *stopped) => Ending::Stopped,
                _ = std::future::poll_fn(|context| accessor.poll_no_interesting_tasks(context)) => ending,
            }
        }).await;
        // Retain The Original Trap Before Closed Reply Waiters Resume
        let ending = match ending {
            Ok(ending) => ending,
            Err(source) => {
                *self.endpoint.failure.lock().expect("guest terminal failure lock") = Some(Arc::new(source));
                self.endpoint.accepting.store(false, Ordering::Release);
                Ending::Stopped
            }
        };
        self.endpoint.accepting.store(false, Ordering::Release);
        if let Ending::Closing(instance) = ending {
            guest.routing = instance;
        }
        self.finish().await
    }

    fn destroy_store(&mut self) {
        let guest = self.guest.take();
        drop(guest);
        let attempts: Vec<_> =
            self.loans.lock().expect("guest invocation loan lock").values_mut().map(|loan| std::mem::take(&mut loan.attempts)).collect();
        drop(attempts);
        // This Acknowledges Store Destruction, Not Host Background Settlement.
        self.ended.send_replace(true);
    }

    async fn finish(&mut self) -> Result<(), Error> {
        self.destroy_store();
        let controls: Vec<_> = self
            .loans
            .lock()
            .expect("guest invocation loan lock")
            .iter()
            .filter_map(|(id, loan)| loan.background.as_ref().map(|control| (*id, Arc::clone(control))))
            .collect();
        for (id, control) in controls {
            control.settle().await.map_err(|source| Error::OperationCaller(crate::Error::OperationSettlement { source }))?;
            let retired = self.loans.lock().expect("guest invocation loan lock").remove(&id);
            drop(retired);
        }
        self.loans.lock().expect("guest invocation loan lock").clear();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn unavailable() -> Error {
    Error::Unavailable { method: "operate".to_owned(), discarded: "the original guest store is unavailable".to_owned() }
}

/* === Tests === */
