/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod cleanup;
mod configuration;
mod exchange;
mod frames;
mod handshake;
mod lifecycle;
mod publication;
mod carrier;
mod connection;
mod managed;
mod observers;
mod physical;

/* === Re-Exports === */
pub(crate) use connection::Endpoint;
pub(crate) use managed::RuntimeDriver;
pub use managed::{Managed, ManagedError};

/* === Imports === */
// Standard Library Imports
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::io::{AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot};
// Local Imports
use super::{Fault, Resident, Service};
use crate::operations::children::{CHILD_REQUESTS, Children};
use crate::streams::{Demand, StreamManager};
use crate::supervision::executor;
use crate::{Application, streams::ManagedPublication, streams::Stream};
use carrier::{Receiver as CarrierReceiver, Sender as CarrierSender};
use harmos_domain::Message;
use harmos_sidecar::{Attach, Attached, Frame, Registration, UNKNOWN};

/* === Constants === */
const SETTLING: Duration = Duration::from_secs(5);

const ARRIVALS: usize = 64;

/* === Macros === */

/* === Type Aliases === */
type Ingest<A> =
    Arc<dyn Fn(&Service<A>, &ManagedPublication) -> Result<(), String> + Send + Sync + 'static>;

type Delivery = Pin<
    Box<
        dyn Future<Output = Result<ManagedPublication, crate::streams::StreamPublicationError>>
            + Send,
    >,
>;

/* === Structs === */
#[derive(Clone)]
pub struct DemandSource {
    keys: Arc<BTreeMap<String, tokio::sync::watch::Sender<usize>>>,
}

pub struct DemandLease {
    count: tokio::sync::watch::Sender<usize>,
}

struct Inbound<A: Application> {
    streams: BTreeMap<&'static str, Ingest<A>>,
    demands: BTreeMap<&'static str, fn(&StreamManager<A>) -> Demand>,
}

pub struct Publish<A: Application, S, H> {
    _resources: Arc<Supervised<A, S, H>>,
    endpoint: Arc<Endpoint>,
}

pub struct Supervised<A: Application, S, H> {
    attach: S,
    notice: Arc<H>,
    ingress: Arc<Inbound<A>>,
    demand_keys: Arc<BTreeMap<String, tokio::sync::watch::Sender<usize>>>,
    expected_registrations: Option<Arc<Vec<Registration>>>,
    handshake: Duration,
    grace: Duration,
    calls: Arc<Calls>,
    registrations: tokio::sync::watch::Sender<Option<Vec<Registration>>>,
    routing: Option<crate::plugin::Router>,
    manager_owned_stop: bool,
    cleanup: tokio::sync::Mutex<Option<Cleanup>>,
}

struct Attempt<'a>(&'a Calls);

struct Calls {
    live: Mutex<Option<Arc<Endpoint>>>,
    ready: tokio::sync::watch::Sender<bool>,
    staged: tokio::sync::watch::Sender<bool>,
    registrations: tokio::sync::watch::Sender<Option<Vec<Registration>>>,
}

struct Request {
    method: String,
    params: Vec<u8>,
    answer: oneshot::Sender<Result<Vec<u8>, SidecarError>>,
    children: Option<Children>,
}

#[derive(Clone, Copy)]
struct Settling<'a> {
    admitted: &'a BTreeMap<String, Arc<crate::streams::PublicationLease>>,
    demand_keys: &'a BTreeMap<String, tokio::sync::watch::Sender<usize>>,
    handshake: Duration,
    manager_owned_stop: bool,
    expected: Option<&'a [Registration]>,
    publications: &'a [harmos_sidecar::StreamDeclaration],
    observers: &'a [harmos_sidecar::inventory::observers::Declaration],
    operations: &'a [crate::operations::Definition],
    contracts: &'a [crate::operations::Contract],
}

struct Reader(tokio::task::JoinHandle<()>);

struct RoutedTask {
    running: tokio::task::JoinHandle<()>,
    token: Arc<()>,
    cancellation: Option<oneshot::Sender<()>>,
    failure: Option<String>,
}

struct Cleanup {
    instance: Option<crate::plugin::PluginEndpoint>,
    registration: Option<crate::streams::StreamRegistration>,
    admitted: BTreeMap<String, Arc<crate::streams::PublicationLease>>,
    session: Option<observers::Session>,
    readers: Vec<Reader>,
    failed_readers: Vec<Reader>,
    routed_tasks: BTreeMap<u64, RoutedTask>,
    child_scopes: BTreeMap<u64, Children>,
    endpoint: Option<Arc<Endpoint>>,
    logical_failure: Option<String>,
    disposition: Option<Result<(), Fault<String>>>,
    ended: Option<tokio::sync::watch::Sender<bool>>,
    failure: Option<String>,
}

struct Exchange<'a, A: Application, H> {
    outgoing: CarrierSender,
    inbound: mpsc::Receiver<Arrival>,
    context: &'a mut Service<A>,
    notice: &'a H,
    ingress: &'a Inbound<A>,
    demand: tokio::sync::watch::Sender<BTreeMap<String, bool>>,
    remote_observers: Arc<observers::Remote>,
    announced: Option<String>,
    failure: Option<String>,
    pending: BTreeMap<u64, oneshot::Sender<Result<Vec<u8>, SidecarError>>>,
    deliveries: VecDeque<Delivery>,
    delivery_turn: bool,
    next: u64,
    manager_owned_stop: bool,
    registrations: &'a tokio::sync::watch::Sender<Option<Vec<Registration>>>,
    expected: Option<&'a [Registration]>,
    publications: &'a [harmos_sidecar::StreamDeclaration],
    observers: &'a [harmos_sidecar::inventory::observers::Declaration],
    operations: &'a [crate::operations::Definition],
    caller: Option<crate::operations::Caller>,
    plugin: Option<crate::plugin::PluginScope>,
    returns: mpsc::Sender<(Frame, Arc<()>)>,
    returned: mpsc::Receiver<(Frame, Arc<()>)>,
    routed_tasks: &'a mut BTreeMap<u64, RoutedTask>,
    cleanup_failure: &'a mut Option<String>,
    child_scopes: &'a mut BTreeMap<u64, Children>,
}

/* === Enums === */
#[derive(Clone, Debug)]
pub enum Notice {
    Logged { line: String },
    Published { publication: ManagedPublication },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SidecarError {
    #[error("the sidecar is not running")]
    Unavailable,
    #[error("the sidecar did not answer inside the route's deadline")]
    Deadline,
    // The refusal's failure travels unchanged; this layer adds only its line.
    #[error("the sidecar refused the call ({code})")]
    Refused {
        code: i64,
        #[source]
        failure: harmos_domain::Failure,
    },
    #[error("the call left the route's contract: {0}")]
    Malformed(String),
}

enum Arrival {
    Publications(Vec<crate::streams::StagedPublication>),
    Frame(Frame),
    Binary(harmos_sidecar::transport::Received),
    Reply(Frame, Arc<()>),
    Diagnostic(String),
    Demanded { stream: String, wanted: bool },
    ObserverFailed(String),
    Ended,
}

enum Step {
    Arrived(Arrival),
    Requested(Request),
    Cancelled(u64),
    Published(Result<ManagedPublication, crate::streams::StreamPublicationError>),
    Stopping,
}

enum Absorbed {
    Carried,
    Malformed(String),
    Ended,
}

/* === Traits === */

/* === Implementations === */
impl SidecarError {
    // A refusal the host raises itself, before any plugin answered.
    pub(crate) fn refused(code: i64, message: impl Into<String>) -> Self {
        let failure =
            harmos_domain::Failure::new(crate::operations::FailureKind::Permanent, message);
        Self::Refused { code, failure }
    }
}

impl Notice {
    pub fn published(&self) -> Option<&ManagedPublication> {
        match self {
            Self::Published { publication } => Some(publication),
            Self::Logged { .. } => None,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn normalized_observers(
    mut declarations: Vec<harmos_sidecar::inventory::observers::Declaration>,
) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
    declarations.sort_by(|left, right| left.id.cmp(&right.id));
    for declaration in &mut declarations {
        declaration.streams.sort();
        declaration.demand_keys.sort();
    }
    declarations
}

async fn converse<A, H>(
    attached: Attached,
    context: &mut Service<A>,
    notice: &H,
    ingress: &Inbound<A>,
    settling: Settling<'_>,
    calls: &Calls,
    cleanup: &mut Cleanup,
) -> Result<(), Fault<String>>
where
    A: Application,
    H: Fn(&Service<A>, Notice) + Send + Sync,
{
    let Settling { admitted, .. } = settling;
    let (write, read, control, diagnostics, end) = attached.into_parts();
    let (connecting, end) = carrier::Connecting::new((read, write), control, end);
    let (live, mut requests) = mpsc::unbounded_channel();
    let (ended, physical_end) = tokio::sync::watch::channel(false);
    let (arrivals, inbound) = mpsc::channel(ARRIVALS);
    let operations = context.operations.clone();
    let remote = observers::Remote::new(arrivals.clone(), Some(context.provider.clone()));
    cleanup.session = Some(observers::Session::new(
        remote.clone(),
        operations.clone(),
        end,
    ));
    cleanup.ended = Some(ended);
    cleanup
        .session
        .as_mut()
        .expect("the attempt owns its session")
        .bind()
        .map_err(Fault::Application)?;
    let plugin = cleanup
        .instance
        .as_ref()
        .and_then(|endpoint| endpoint.scope())
        .cloned();
    let candidate = observers::stage(
        &operations,
        &remote,
        plugin.as_ref(),
        settling.observers,
        settling.publications,
    )
    .map_err(|error| Fault::Application(error.to_string()))?;
    let (demand, changes) = tokio::sync::watch::channel(BTreeMap::new());

    let connected = executor::timeout(settling.handshake, connecting.connect())
        .await
        .map_err(|_| "the sidecar did not open inside its handshake".to_owned())
        .and_then(|result| {
            result.map_err(|error| format!("the sidecar gRPC transport failed: {error}"))
        });
    let peer = match connected {
        Ok(peer) => peer,
        Err(error) => {
            return Err(Fault::Environmental(error));
        }
    };
    let (outgoing, incoming) = peer;

    cleanup
        .readers
        .push(Reader(executor::task(protocol(incoming, arrivals.clone()))));
    if let Some(stream) = diagnostics {
        cleanup
            .readers
            .push(Reader(executor::task(diagnostic(stream, arrivals.clone()))));
    }
    // One demand watcher per admitted stream, combining plugin readers with
    // any typed host projection. Restart re-reads counts held across attempts.
    // Demand received during initialization is retained until serving begins.
    let mut leases: Vec<Reader> = admitted
        .iter()
        .map(|(stream, lease)| {
            let native = ingress
                .demands
                .get(stream.as_str())
                .map(|demand| demand(context.streams()));
            Reader(executor::task(leasing(
                stream.clone(),
                lease.stream().demand(),
                native,
                arrivals.clone(),
            )))
        })
        .collect();
    leases.extend(settling.demand_keys.iter().map(|(key, count)| {
        Reader(executor::task(key_leasing(
            key.clone(),
            count.subscribe(),
            arrivals.clone(),
        )))
    }));
    cleanup.readers.extend(leases);
    let caller = cleanup
        .instance
        .as_ref()
        .map(|instance| context.operations.caller(&instance.caller()))
        .transpose()
        .map_err(|error| Fault::Application(error.to_string()))?;
    if let (Some(plugin), Some(_)) = (&plugin, &caller) {
        remote
            .configure(context.operations.clone(), plugin.clone())
            .map_err(|error| Fault::Application(error.to_string()))?;
    }
    let (returns, returned) = mpsc::channel(CHILD_REQUESTS);
    let observing = Arc::clone(&remote);
    let mut exchange = Exchange {
        outgoing,
        inbound,
        context,
        notice,
        ingress,
        demand,
        remote_observers: remote,
        announced: None,
        failure: None,
        pending: BTreeMap::new(),
        deliveries: VecDeque::new(),
        delivery_turn: true,
        next: 1,
        manager_owned_stop: settling.manager_owned_stop,
        registrations: &calls.registrations,
        expected: settling.expected,
        publications: settling.publications,
        observers: settling.observers,
        operations: settling.operations,
        caller,
        plugin: plugin.clone(),
        returns,
        returned,
        routed_tasks: &mut cleanup.routed_tasks,
        child_scopes: &mut cleanup.child_scopes,
        cleanup_failure: &mut cleanup.failure,
    };

    let mut stopped = false;
    let settled = exchange.open(settling.handshake).await;
    if settled.is_ok() {
        cleanup.endpoint = Some(
            calls.open(
                live.clone(),
                arrivals.clone(),
                settling.operations,
                settling.contracts,
                physical_end,
                cleanup
                    .instance
                    .as_ref()
                    .and_then(|instance| instance.scope())
                    .cloned()
                    .map(|scope| (scope, Arc::clone(&exchange.remote_observers))),
            ),
        );
        let initialized = cleanup
            .endpoint
            .as_ref()
            .expect("the original endpoint is retained")
            .initialized();
        let manager_owned_stop = exchange.manager_owned_stop;
        stopped = exchange
            .serve(&mut requests, initialized, |context| {
                candidate.commit().map_err(|error| error.to_string())?;
                cleanup
                    .readers
                    .push(Reader(executor::task(observers::demand(
                        operations,
                        observing,
                        plugin,
                        settling.observers.to_vec(),
                        changes,
                        arrivals,
                    ))));
                calls.ready.send_replace(true);
                if !manager_owned_stop {
                    context.ready();
                }
                Ok(())
            })
            .await;
        if let Some(instance) = cleanup.instance.as_mut() {
            instance.quiesce();
        }
        calls.close();
        drop(live);
    }

    let failure = exchange.failure.take();
    let announced = exchange.announced.take();
    drop(exchange);

    match (stopped, settled, failure, announced) {
        (true, _, _, _) => Ok(()),
        (false, Err(fault), _, _) => Err(fault),
        (false, Ok(()), Some(failure), _) => Err(Fault::Environmental(failure)),
        (false, Ok(()), None, Some(reason)) => Err(Fault::Application(reason)),
        (false, Ok(()), None, None) => Err(Fault::Environmental(
            "the sidecar exited without announcing it".to_owned(),
        )),
    }
}

async fn key_leasing(
    key: String,
    mut count: tokio::sync::watch::Receiver<usize>,
    arrivals: mpsc::Sender<Arrival>,
) {
    let mut held = false;
    loop {
        let wanted = *count.borrow_and_update() > 0;
        if wanted != held {
            held = wanted;
            if arrivals
                .send(Arrival::Demanded {
                    stream: key.clone(),
                    wanted,
                })
                .await
                .is_err()
            {
                return;
            }
        }
        if count.changed().await.is_err() {
            return;
        }
    }
}

async fn leasing(
    stream: String,
    mut demand: Demand,
    mut native: Option<Demand>,
    arrivals: mpsc::Sender<Arrival>,
) {
    let wanted = |demand: &Demand, native: &Option<Demand>| {
        *demand.borrow() > 0 || native.as_ref().is_some_and(|value| *value.borrow() > 0)
    };
    let mut held = false;
    loop {
        let now = wanted(&demand, &native);
        if now != held {
            held = now;
            if arrivals
                .send(Arrival::Demanded {
                    stream: stream.clone(),
                    wanted: now,
                })
                .await
                .is_err()
            {
                return;
            }
        }
        let changed = if let Some(native) = &mut native {
            tokio::select! {
                changed = demand.changed() => changed,
                changed = native.changed() => changed,
            }
        } else {
            demand.changed().await
        };
        if changed.is_err() {
            return;
        }
    }
}

fn reason(params: &[u8]) -> String {
    Frame::exiting_reason(params).unwrap_or_else(|| "the sidecar announced its own exit".to_owned())
}

async fn protocol(mut incoming: CarrierReceiver, arrivals: mpsc::Sender<Arrival>) {
    while let Ok(Some(frame)) = incoming.receive().await {
        if arrivals.send(frame).await.is_err() {
            return;
        }
    }
    let _ = arrivals.send(Arrival::Ended).await;
}

async fn diagnostic(read: Box<dyn AsyncRead + Send + Unpin>, arrivals: mpsc::Sender<Arrival>) {
    let mut input = BufReader::new(read);
    while let Ok(Some(line)) = Frame::diagnostic(&mut input).await {
        if arrivals.send(Arrival::Diagnostic(line)).await.is_err() {
            return;
        }
    }
}

/* === Tests === */
