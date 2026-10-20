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
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

// Third Party Imports
use futures_core::Stream;
use futures_util::stream::FuturesUnordered;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};
use tokio::task::JoinError;

// Local Imports
use super::execution::{self, Execution, Prepared};
use super::lane::{Admission, Lanes};
use super::{OperationInstanceId, OperationStatus, Settings};
use crate::supervision::{Task, executor, instance::Instance};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Driver = Pin<Box<dyn Future<Output = ()> + Send>>;
type Completion = Pin<Box<dyn Future<Output = (Arc<Instance>, std::result::Result<(), JoinError>)> + Send>>;

/* === Structs === */

pub(crate) struct Instances {
    lanes: Lanes,
    state: Mutex<State>,
    ordinary: Queue,
    lifecycle: Queue,
    supervisor: Task<()>,
    application_state: Box<dyn Any + Send + Sync>,
    resources: Arc<dyn Any + Send + Sync>,
}

pub(crate) struct Reservation<O> {
    outcome: oneshot::Receiver<Result<O>>,
    deposit: oneshot::Sender<(Prepared<O>, Option<crate::state::Position>)>,
    control: Arc<Instance>,
    admission: Admission,
    cause: Option<crate::state::Position>,
}

struct State {
    closing: bool,
    active: BTreeMap<OperationInstanceId, Arc<Instance>>,
    // The submitting caller of each active instance, for the built-ins.
    owners: BTreeMap<OperationInstanceId, super::caller::Identity>,
    // Resident instances run outside the dispatcher until stopped; every
    // caller holding one of their leases may report them.
    residents: BTreeMap<OperationInstanceId, (Arc<Instance>, Vec<super::caller::Identity>)>,
}

struct Queue {
    sending: mpsc::Sender<Submission>,
    capacity: Arc<Semaphore>,
}

struct Receiving {
    ordinary: mpsc::Receiver<Submission>,
    lifecycle: mpsc::Receiver<Submission>,
}

struct Submission {
    instance: Arc<Instance>,
    driver: Driver,
}

#[derive(Clone)]
pub(crate) struct QueuePermit(Arc<Mutex<Option<OwnedSemaphorePermit>>>);

/* === Enums === */

enum Event {
    Closing,
    Submit(Submission),
    Completed(Arc<Instance>, std::result::Result<(), JoinError>),
    Drained,
}

/* === Traits === */

/* === Implementations === */

impl Instances {
    pub(crate) fn start(
        lanes: Lanes, application_state: Box<dyn Any + Send + Sync>, resources: Arc<dyn Any + Send + Sync>, settings: &Settings,
    ) -> Arc<Self> {
        let (ordinary, ordinary_receiving) = Queue::new(settings.queued.get());
        let (lifecycle, lifecycle_receiving) = Queue::new(settings.lifecycle_queued.get());
        let receiving = Receiving { ordinary: ordinary_receiving, lifecycle: lifecycle_receiving };
        let supervisor = Task::spawn(move |stopping| dispatch(receiving, stopping.subscribe()));
        Arc::new(Self {
            lanes,
            state: Mutex::new(State { closing: false, active: BTreeMap::new(), owners: BTreeMap::new(), residents: BTreeMap::new() }),
            ordinary,
            lifecycle,
            supervisor,
            application_state,
            resources,
        })
    }

    pub(crate) fn is_accepting(&self) -> bool {
        !self.state.lock().expect("operation instance index is healthy").closing
    }

    pub(crate) fn lane_bounded(&self, name: &str) -> Option<bool> {
        self.lanes.get(name).map(|lane| lane.bounded())
    }

    pub(crate) fn status(&self, id: OperationInstanceId) -> Option<OperationStatus> {
        let state = self.state.lock().expect("operation instance index is healthy");
        state.active.get(&id).or_else(|| state.residents.get(&id).map(|(instance, _)| instance)).map(|instance| instance.status())
    }

    // Index a resident instance for the built-ins until it stops.
    pub(super) fn track_resident(&self, id: OperationInstanceId, instance: &Arc<Instance>, holder: super::caller::Identity) {
        let mut state = self.state.lock().expect("operation instance index is healthy");
        if instance.is_settled() {
            return;
        }
        let (_, holders) = state.residents.entry(id).or_insert_with(|| (Arc::clone(instance), Vec::new()));
        if !holders.contains(&holder) {
            holders.push(holder);
        }
    }

    pub(crate) fn forget_resident(&self, id: OperationInstanceId) {
        let retired = self.state.lock().expect("operation instance index is healthy").residents.remove(&id);
        drop(retired);
    }

    pub(super) fn assign(&self, id: OperationInstanceId, owner: super::caller::Identity) {
        let mut state = self.state.lock().expect("operation instance index is healthy");
        if state.active.contains_key(&id) {
            state.owners.insert(id, owner);
        }
    }

    // Resolve an active instance only for the caller that submitted it.
    pub(super) fn owned(&self, id: OperationInstanceId, caller: &super::caller::Identity) -> Option<Arc<Instance>> {
        let state = self.state.lock().expect("operation instance index is healthy");
        if let Some((instance, holders)) = state.residents.get(&id) {
            return holders.contains(caller).then(|| Arc::clone(instance));
        }
        if state.owners.get(&id) != Some(caller) {
            return None;
        }
        state.active.get(&id).cloned()
    }

    pub(crate) fn state_of<A: crate::Application>(&self) -> crate::state::StateManager<A> {
        self.handles_of::<A>().state.clone()
    }

    // The runtime's typed managers a service context lends its resident.
    pub(crate) fn handles_of<A: crate::Application>(&self) -> &crate::services::Handles<A> {
        self.application_state.downcast_ref::<crate::services::Handles<A>>().expect("operation scope names its runtime's application")
    }

    pub(crate) fn resources_of<A: crate::Application>(&self) -> &A::Resources {
        self.resources.downcast_ref::<A::Resources>().expect("operation scope names its runtime's application")
    }

    pub(crate) fn reserve<O: Send + 'static>(
        self: &Arc<Self>, id: OperationInstanceId, lane: &'static str, submitted: tokio::time::Instant, privileged: bool,
        retired: impl Fn() + Send + Sync + 'static,
    ) -> Result<Reservation<O>> {
        let admission = self.lanes.get(lane).expect("registration validated its lane");
        let mut state = self.state.lock().expect("operation instance index is healthy");
        if state.closing {
            return Err(Error::Closing { domain: "operation instances" });
        }
        let queue = if privileged { &self.lifecycle } else { &self.ordinary };
        let permit =
            queue.capacity.clone().try_acquire_owned().map_err(|_| Error::ResourceExhausted { resource: "operation admission queue" })?;
        let sending = queue.sending.try_reserve().map_err(|_| Error::Closing { domain: "operation dispatcher" })?;
        let queue_permit = QueuePermit(Arc::new(Mutex::new(Some(permit))));
        let retiring = queue_permit.clone();
        let owner = Arc::downgrade(self);
        let (cancellation, cancelled) = watch::channel(false);
        let (reply, outcome) = oneshot::channel();
        let instance = Instance::queued(cancellation).on_settled(move || {
            retiring.release();
            if let Some(owner) = owner.upgrade() {
                let retired = {
                    let mut state = owner.state.lock().expect("operation instance index is healthy");
                    state.owners.remove(&id);
                    state.active.remove(&id)
                };
                drop(retired);
            }
            retired();
        });
        let instance = Arc::new(instance);
        let (deposit, prepared) = oneshot::channel();
        let reservation = Reservation { outcome, deposit, control: instance.clone(), admission: admission.clone(), cause: None };
        let execution = Execution { admission, submitted, cancelled, instance: instance.clone(), queue: queue_permit };
        assert!(state.active.insert(id, instance.clone()).is_none(), "instance IDs are unique");
        let owner = self.clone();
        let control = instance.clone();
        sending.send(Submission {
            instance,
            driver: Box::pin(async move {
                match prepared.await {
                    Ok((prepared, cause)) => {
                        let answer = execution::execute(owner, prepared, execution, cause).await;
                        let _ = reply.send(answer);
                    }
                    Err(_) => control.finished(),
                }
            }),
        });
        Ok(reservation)
    }

    pub(crate) fn close(&self) {
        let instances: Vec<_> = {
            let mut state = self.state.lock().expect("operation instance index is healthy");
            state.closing = true;
            state.active.values().cloned().collect()
        };
        for instance in instances {
            instance.cancel();
        }
        self.supervisor.stop();
    }

    pub(crate) async fn settle(&self) -> Result<()> {
        self.supervisor.settle().await.map_err(|source| Error::TaskFailed { domain: "operation dispatcher", source })?;
        Ok(())
    }
}

impl<O> Reservation<O> {
    pub(crate) fn control(&self) -> Arc<Instance> {
        self.control.clone()
    }

    pub(crate) fn inherit(mut self, cause: Option<crate::state::Position>) -> Self {
        self.cause = cause;
        self
    }

    pub(crate) fn submit(self, prepared: Prepared<O>, cause: Option<crate::state::Position>) -> oneshot::Receiver<Result<O>> {
        if let Err((prepared, _)) = self.deposit.send((prepared, cause.or(self.cause))) {
            prepared.retain_unavailable(&self.control, self.admission);
        }
        self.outcome
    }
}

impl Queue {
    fn new(capacity: usize) -> (Self, mpsc::Receiver<Submission>) {
        let (sending, receiving) = mpsc::channel(capacity);
        (Self { sending, capacity: Arc::new(Semaphore::new(capacity)) }, receiving)
    }
}

impl QueuePermit {
    pub(crate) fn release(&self) {
        let permit = self.0.lock().expect("operation queue claim is healthy").take();
        drop(permit);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn dispatch(mut receiving: Receiving, mut stopping: watch::Receiver<bool>) {
    let mut running: FuturesUnordered<Completion> = FuturesUnordered::new();
    let mut closing = false;
    loop {
        match next_event(&mut receiving, &mut running, &mut stopping, closing).await {
            Event::Closing => {
                closing = true;
                receiving.ordinary.close();
                receiving.lifecycle.close();
            }
            Event::Submit(submission) => {
                let task = executor::task(submission.driver);
                running.push(Box::pin(async move { (submission.instance, task.await) }));
            }
            Event::Completed(instance, Err(source)) => {
                instance.interrupted(Arc::new(Error::TaskFailed { domain: "operation instance", source: Arc::new(source) }));
            }
            Event::Completed(_, Ok(())) => {}
            Event::Drained => return,
        }
    }
}

async fn next_event(
    receiving: &mut Receiving, running: &mut FuturesUnordered<Completion>, stopping: &mut watch::Receiver<bool>, closing: bool,
) -> Event {
    if !closing && *stopping.borrow() {
        return Event::Closing;
    }
    let mut cancellation = std::pin::pin!(stopping.changed());
    std::future::poll_fn(|context| {
        if !closing && cancellation.as_mut().poll(context).is_ready() {
            return Poll::Ready(Event::Closing);
        }
        if let Poll::Ready(Some((instance, outcome))) = Pin::new(&mut *running).poll_next(context) {
            return Poll::Ready(Event::Completed(instance, outcome));
        }
        let lifecycle_closed = match receiving.lifecycle.poll_recv(context) {
            Poll::Ready(Some(submission)) => return Poll::Ready(Event::Submit(submission)),
            Poll::Ready(None) => true,
            Poll::Pending => false,
        };
        let ordinary_closed = match receiving.ordinary.poll_recv(context) {
            Poll::Ready(Some(submission)) => return Poll::Ready(Event::Submit(submission)),
            Poll::Ready(None) => true,
            Poll::Pending => false,
        };
        if lifecycle_closed && ordinary_closed && running.is_empty() { Poll::Ready(Event::Drained) } else { Poll::Pending }
    })
    .await
}

/* === Tests === */
