/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
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

struct Ingress<A: Application> {
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
    ingress: Arc<Ingress<A>>,
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
    ingress: &'a Ingress<A>,
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

impl DemandSource {
    pub fn hold(&self, key: &str) -> Result<DemandLease, SidecarError> {
        let count = self
            .keys
            .get(key)
            .ok_or_else(|| SidecarError::Malformed(format!("undeclared demand key `{key}`")))?
            .clone();
        count.send_modify(|count| *count += 1);

        Ok(DemandLease { count })
    }
}

impl Drop for DemandLease {
    fn drop(&mut self) {
        self.count.send_modify(|count| *count -= 1);
    }
}

impl<A: Application> Clone for Ingress<A> {
    fn clone(&self) -> Self {
        Self {
            streams: self.streams.clone(),
            demands: self.demands.clone(),
        }
    }
}

impl<A: Application> Default for Ingress<A> {
    fn default() -> Self {
        Self {
            streams: BTreeMap::new(),
            demands: BTreeMap::new(),
        }
    }
}

impl<A: Application, S: Attach, H> Supervised<A, S, H> {
    pub fn new(attach: S, notice: H) -> Self {
        let (registrations, _) = tokio::sync::watch::channel(None);
        Self {
            attach,
            notice: Arc::new(notice),
            ingress: Arc::new(Ingress::default()),
            demand_keys: Arc::new(BTreeMap::new()),
            expected_registrations: None,
            handshake: SETTLING,
            grace: SETTLING,
            calls: Arc::new(Calls::new(registrations.clone())),
            registrations,
            routing: None,
            manager_owned_stop: false,
            cleanup: tokio::sync::Mutex::new(None),
        }
    }

    pub fn demand_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        let publications = self.attach.publications();
        let mut admitted = BTreeMap::new();
        for key in keys {
            assert!(
                key.split_once(':')
                    .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty()),
                "demand keys must be namespaced"
            );
            assert!(
                !publications.iter().any(|entry| entry.id == key),
                "demand keys must differ from publication identities"
            );
            admitted
                .entry(key)
                .or_insert_with(|| tokio::sync::watch::channel(0).0);
        }

        self.demand_keys = Arc::new(admitted);
        self
    }

    pub fn demands(&self) -> DemandSource {
        DemandSource {
            keys: Arc::clone(&self.demand_keys),
        }
    }

    pub fn routed(mut self, router: &crate::plugin::Router) -> Self {
        self.routing = Some(router.clone());
        self
    }

    pub fn registrations(&self) -> tokio::sync::watch::Receiver<Option<Vec<Registration>>> {
        self.registrations.subscribe()
    }

    pub fn expected_registrations(mut self, registrations: &[Registration]) -> Self {
        let mut registrations = registrations.to_vec();
        registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        self.expected_registrations = Some(Arc::new(registrations));
        self
    }

    pub fn ready(&self) -> tokio::sync::watch::Receiver<bool> {
        self.calls.ready.subscribe()
    }

    pub fn ingest<Source>(mut self) -> Self
    where
        Source: harmos_sidecar::StreamSpec,
        Source::Row: Stream<A> + Message + Default,
    {
        let ingress = Arc::make_mut(&mut self.ingress);
        ingress
            .demands
            .insert(Source::ID, |streams: &StreamManager<A>| {
                streams.demand::<Source::Row>()
            });
        let streams = &mut ingress.streams;
        let previous = streams.insert(
            Source::ID,
            Arc::new(|context, publication| {
                if publication.schema() != Source::VERSION {
                    return Err(format!(
                        "sidecar stream `{}` sent schema {}, expected {}",
                        Source::ID,
                        publication.schema(),
                        Source::VERSION
                    ));
                }
                let row = Source::Row::decode(publication.payload()).map_err(|error| {
                    format!(
                        "sidecar stream `{}` sent malformed protobuf: {error}",
                        Source::ID
                    )
                })?;
                context.streams().miss::<Source::Row>(publication.missed());
                context.streams().publish(row);

                Ok(())
            }),
        );
        assert!(
            previous.is_none(),
            "sidecar stream `{}` was registered twice",
            Source::ID
        );

        self
    }

    pub fn handshake(mut self, within: Duration) -> Self {
        self.handshake = within;
        self
    }

    pub fn grace(mut self, within: Duration) -> Self {
        self.grace = within;
        self
    }

    pub async fn staged_operations(&self) -> Result<crate::operations::Native, SidecarError> {
        let mut staged = self.calls.staged.subscribe();
        loop {
            if *staged.borrow_and_update() {
                return self.calls.current().map(crate::operations::Native::new);
            }
            staged
                .changed()
                .await
                .map_err(|_| SidecarError::Unavailable)?;
        }
    }

    pub fn operations(&self) -> Result<crate::operations::Native, SidecarError> {
        let endpoint = self.calls.current()?;
        if !*endpoint.initialized().borrow()
            || !endpoint.scope().is_some_and(|scope| scope.is_ready())
        {
            return Err(SidecarError::Unavailable);
        }
        Ok(crate::operations::Native::new(endpoint))
    }
}

impl<A: Application, S: Clone, H> Clone for Supervised<A, S, H> {
    fn clone(&self) -> Self {
        Self {
            attach: self.attach.clone(),
            notice: Arc::clone(&self.notice),
            ingress: Arc::clone(&self.ingress),
            demand_keys: Arc::clone(&self.demand_keys),
            expected_registrations: self.expected_registrations.clone(),
            handshake: self.handshake,
            grace: self.grace,
            calls: Arc::clone(&self.calls),
            registrations: self.registrations.clone(),
            routing: self.routing.clone(),
            manager_owned_stop: self.manager_owned_stop,
            cleanup: tokio::sync::Mutex::new(None),
        }
    }
}

impl<A, S, H> crate::operations::Declares for Publish<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Declaration = crate::operations::host::Publish;
}

impl<A, S, H> crate::operations::Operation for Publish<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Declaration = crate::operations::host::Publish;
    type Resources = Supervised<A, S, H>;
    type Error = crate::Error;
    type Endpoints = ();
    type Context<'a> = crate::operations::Context<'a>;

    async fn initialize<'ctx>(
        resources: Arc<Self::Resources>,
        _: Self::Context<'ctx>,
    ) -> crate::Result<Self> {
        let endpoint =
            resources
                .calls
                .current()
                .map_err(|source| crate::Error::OperationFailed {
                    operation: "host.publish".into(),
                    source: Arc::new(source),
                })?;
        Ok(Self {
            _resources: resources,
            endpoint,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut crate::operations::host::Publication,
        _endpoints: &mut Self::Endpoints,
    ) -> crate::Result<crate::operations::Response<()>> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            let scope = context.caller_scope().ok_or(crate::Error::InvalidInput {
                field: "native publication",
                reason: "the original plugin caller is required",
            })?;
            self.endpoint.publish(scope, request, &context).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl<A, S, H> Resident<A> for Supervised<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Error = String;

    async fn run(&mut self, mut context: Service<A>) -> Result<(), Fault<String>> {
        self.calls.close();
        let _attempt = Attempt(&self.calls);
        if context.is_stopping() {
            return Ok(());
        }
        if self.cleanup.get_mut().is_some() {
            return Err(Fault::Application(
                "native physical cleanup is still pending".into(),
            ));
        }
        let instance = self
            .routing
            .as_ref()
            .map(|router| router.enter(self.attach.owner()))
            .transpose()
            .map_err(|error| Fault::Application(error.to_string()))?;
        let declarations = self.attach.publications();
        let observers = self.attach.observers();
        let operations = self
            .attach
            .operations()
            .map_err(|error| Fault::Application(error.to_string()))?;
        harmos_sidecar::inventory::operations::validate(&operations)
            .map_err(|error| Fault::Application(error.to_string()))?;
        let contracts = self
            .attach
            .operation_contracts()
            .map_err(|error| Fault::Application(error.to_string()))?;
        if let Some(contracts) = &contracts {
            let mut ids = std::collections::BTreeSet::new();
            if contracts.len() != operations.len()
                || contracts.iter().any(|contract| {
                    !ids.insert(contract.operation().id.clone())
                        || !operations
                            .iter()
                            .any(|operation| operation == contract.operation())
                })
            {
                return Err(Fault::Application(
                    "native canonical contracts differ from the admitted operations".into(),
                ));
            }
        }
        let contracts = contracts.unwrap_or_default();
        let sdk = self
            .attach
            .sdk_lifecycle()
            .map_err(|error| Fault::Application(error.to_string()))?;
        let scope = instance.as_ref().and_then(|endpoint| endpoint.scope());
        if sdk.is_none() || scope.is_none() {
            return Err(Fault::Application(
                "native SDK transport requires the original manager-issued scope".to_owned(),
            ));
        }
        if sdk.as_ref() != scope.and_then(|scope| scope.lifecycle()) {
            return Err(Fault::Application(
                "native SDK pair differs from the original admitted generation".to_owned(),
            ));
        }
        let control = self
            .attach
            .observer_control()
            .map_err(|error| Fault::Application(error.to_string()))?;
        let mut required = self
            .attach
            .required_contracts()
            .map_err(|error| Fault::Application(error.to_string()))?;
        if let Some(required) = &mut required {
            required.sort_by(|a, b| a.contract.operation().id.cmp(&b.contract.operation().id));
        }
        if required.as_deref() != scope.and_then(|scope| scope.required_contracts()) {
            return Err(Fault::Application(
                "native required contracts differ from the original admitted generation".to_owned(),
            ));
        }
        if control.as_ref() != scope.and_then(|scope| scope.observer_control())
            || scope.is_some_and(|scope| {
                normalized_observers(scope.observers().to_vec())
                    != normalized_observers(observers.clone())
            })
        {
            return Err(Fault::Application(
                "native observer roles or inventory differ from the original admitted generation"
                    .to_owned(),
            ));
        }
        harmos_sidecar::inventory::observers::validate(&observers, &declarations)
            .map_err(|error| Fault::Application(error.to_string()))?;
        let plugin = instance
            .as_ref()
            .and_then(|endpoint| endpoint.scope())
            .cloned();
        if let Some(scope) = &plugin {
            scope
                .validate_operations(operations.iter())
                .map_err(|error| Fault::Application(error.to_string()))?;
        }
        *self.cleanup.get_mut() = Some(Cleanup {
            instance,
            registration: None,
            admitted: BTreeMap::new(),
            session: None,
            readers: Vec::new(),
            failed_readers: Vec::new(),
            routed_tasks: BTreeMap::new(),
            child_scopes: BTreeMap::new(),
            endpoint: None,
            logical_failure: None,
            disposition: None,
            ended: None,
            failure: None,
        });
        let cleanup = self
            .cleanup
            .get_mut()
            .as_mut()
            .expect("the attempt owns its cleanup");
        let admitted: BTreeMap<String, Arc<crate::streams::PublicationLease>> =
            executor::timeout(self.handshake, async {
                let scope = plugin
                    .as_ref()
                    .expect("SDK transport retains its original admitted scope");
                let registration = context
                    .streams()
                    .stage_plugin(scope, declarations.clone())
                    .await
                    .map_err(|error| Fault::Application(error.to_string()))?;
                let leases = registration.leases().to_vec();
                cleanup.registration = Some(registration);
                Ok::<_, Fault<String>>(
                    declarations
                        .iter()
                        .zip(leases)
                        .map(|(declaration, lease)| (declaration.id.clone(), lease))
                        .collect(),
                )
            })
            .await
            .map_err(|_| {
                Fault::Application("stream admission exceeded the handshake deadline".into())
            })??;

        // Retain the reservation through physical attempt settlement.
        cleanup.admitted = admitted.clone();
        let attached = self.attach.attach().await.map_err(|error| {
            Fault::Environmental(format!("the sidecar could not be started: {error}"))
        })?;

        let result = converse(
            attached,
            &mut context,
            &*self.notice,
            &self.ingress,
            Settling {
                admitted: &admitted,
                demand_keys: &self.demand_keys,
                handshake: self.handshake,
                manager_owned_stop: self.manager_owned_stop,
                expected: self.expected_registrations.as_deref().map(Vec::as_slice),
                publications: &declarations,
                observers: &observers,
                operations: &operations,
                contracts: &contracts,
            },
            &self.calls,
            cleanup,
        )
        .await;
        cleanup.disposition = Some(result);
        let settled = cleanup.finish().await;
        let disposition = cleanup
            .disposition
            .take()
            .expect("the attempt retained its disposition");
        disposition?;
        settled.map_err(Fault::Application)?;
        if let Some(failure) = &cleanup.logical_failure {
            return Err(Fault::Environmental(failure.clone()));
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.calls.close();
        if let Some(cleanup) = self.cleanup.get_mut().as_mut() {
            cleanup.finish().await?;
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), String> {
        self.cleanup.get_mut().take();
        Ok(())
    }
}

impl Cleanup {
    async fn finish(&mut self) -> Result<(), String> {
        if let Some(instance) = self.instance.as_mut() {
            instance.quiesce();
        }
        if let Some(endpoint) = &self.endpoint {
            endpoint.close();
        }
        for task in self.routed_tasks.values_mut() {
            if let Some(cancellation) = task.cancellation.take() {
                let _ = cancellation.send(());
            }
        }
        // Native Finalizers May Be Children Of The Routed Calls Below.
        // Prove Their Carrier Ended Before Joining Those Host-Side Parents.
        if let Some(session) = self.session.as_mut() {
            session.end().await?;
        }
        if let Some(ended) = self.ended.take() {
            ended.send_replace(true);
        }
        let ids: Vec<_> = self.routed_tasks.keys().copied().collect();
        for id in ids {
            let task = self
                .routed_tasks
                .get_mut(&id)
                .expect("the retained slot exists");
            if task.failure.is_some() {
                continue;
            }
            if let Err(error) = (&mut task.running).await {
                let failure = format!(
                    "native operation drain failed: {}",
                    harmos_domain::failure::Report(&error)
                );
                task.failure = Some(failure.clone());
                self.failure.get_or_insert(failure);
                continue;
            }
            self.routed_tasks.remove(&id);
        }
        for reader in &self.readers {
            reader.0.abort();
        }
        while let Some(reader) = self.readers.last_mut() {
            let joined = (&mut reader.0).await;
            let reader = self.readers.pop().expect("the retained reader exists");
            if let Err(error) = joined
                && !error.is_cancelled()
            {
                self.failure
                    .get_or_insert_with(|| format!("native transport task failed: {error}"));
                self.failed_readers.push(reader);
            }
        }
        if let Some(session) = self.session.as_mut() {
            session.finish().await?;
            if let Some(failure) = session.outcome() {
                self.logical_failure
                    .get_or_insert_with(|| failure.to_string());
            }
        }
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        self.child_scopes.clear();
        self.session.take();
        self.endpoint.take();
        self.admitted.clear();
        self.registration.take();
        self.instance.take();
        Ok(())
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl Calls {
    fn new(registrations: tokio::sync::watch::Sender<Option<Vec<Registration>>>) -> Self {
        Self {
            live: Mutex::new(None),
            ready: tokio::sync::watch::channel(false).0,
            staged: tokio::sync::watch::channel(false).0,
            registrations,
        }
    }

    fn open(
        &self,
        live: mpsc::UnboundedSender<Request>,
        arrivals: mpsc::Sender<Arrival>,
        operations: &[crate::operations::Definition],
        contracts: &[crate::operations::Contract],
        ended: tokio::sync::watch::Receiver<bool>,
        plugin: Option<(crate::plugin::PluginScope, Arc<observers::Remote>)>,
    ) -> Arc<Endpoint> {
        let scope = plugin.as_ref().map(|(scope, _)| scope.clone());
        let endpoint = Endpoint::new(live, arrivals, operations, contracts, ended, scope);
        let endpoint = match &plugin {
            Some((_, remote)) => endpoint.with_observers(remote),
            None => endpoint,
        };
        let endpoint = Arc::new(endpoint);
        let mut current = self.live.lock().expect("the sidecar call lock is healthy");
        let previous = current.replace(endpoint.clone());
        if let Some(previous) = previous.as_ref() {
            previous.close();
        }
        drop(current);
        drop(previous);
        self.staged.send_replace(true);
        self.ready.send_replace(*endpoint.initialized().borrow());
        endpoint
    }

    fn close(&self) {
        let mut current = self.live.lock().expect("the sidecar call lock is healthy");
        let previous = current.take();
        if let Some(previous) = previous.as_ref() {
            previous.close();
        }
        drop(current);
        drop(previous);
        self.ready.send_replace(false);
        self.staged.send_replace(false);
        self.registrations.send_replace(None);
    }

    fn current(&self) -> Result<Arc<Endpoint>, SidecarError> {
        let current = self.live.lock().expect("the sidecar call lock is healthy");
        current.as_ref().cloned().ok_or(SidecarError::Unavailable)
    }
}

impl Drop for RoutedTask {
    fn drop(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            let _ = cancellation.send(());
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<A, H> Exchange<'_, A, H>
where
    A: Application,
    H: Fn(&Service<A>, Notice) + Send + Sync,
{
    async fn open(&mut self, within: Duration) -> Result<(), Fault<String>> {
        let encoded = self.open_call(Frame::REGISTER, Vec::new(), within).await?;
        let catalog =
            serde_json::from_slice::<harmos_sidecar::PluginCatalog>(&encoded).map_err(|error| {
                Fault::Application(format!(
                    "the sidecar returned an invalid plugin catalog: {error}"
                ))
            })?;
        harmos_sidecar::inventory::observers::validate(&catalog.observers, self.publications)
            .map_err(|error| Fault::Application(error.to_string()))?;
        if normalized_observers(catalog.observers) != normalized_observers(self.observers.to_vec())
        {
            return Err(Fault::Application(
                "the sidecar observers differ from the admitted catalog".to_owned(),
            ));
        }
        harmos_sidecar::inventory::operations::validate_match(self.operations, &catalog.operations)
            .map_err(|error| {
                Fault::Application(format!(
                    "the sidecar operations differ from the admitted catalog: {error}"
                ))
            })?;
        let mut registrations = catalog.registrations;
        if let Some(expected) = self.expected {
            registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
            if registrations
                .windows(2)
                .any(|pair| pair[0].namespace == pair[1].namespace)
                || registrations != expected
            {
                return Err(Fault::Application(
                    "the sidecar registrations differ from the expected catalog".to_owned(),
                ));
            }
        }
        self.registrations.send_replace(Some(registrations));
        Ok(())
    }

    async fn open_call(
        &mut self,
        method: &str,
        params: Vec<u8>,
        within: Duration,
    ) -> Result<Vec<u8>, Fault<String>> {
        let id = self.mint();
        if let Err(error) = self
            .send(Frame::Call {
                id,
                method: method.to_owned(),
                params,
            })
            .await
        {
            return Err(Fault::Environmental(format!(
                "the sidecar could not answer `{method}`: {error}"
            )));
        }

        let opened = executor::timeout(within, async {
            loop {
                let Some(arrival) = self.arrival().await else {
                    return Err(Fault::Environmental(
                        "the sidecar stopped speaking while opening".to_owned(),
                    ));
                };
                let frame = match &arrival {
                    Arrival::Frame(frame) => Some(frame),
                    Arrival::Binary(received) => Some(&received.frame),
                    _ => None,
                };
                let answered = match frame {
                    Some(Frame::Answer { id: at, result }) if *at == id => Some(result.clone()),
                    _ => None,
                };
                let refused = match frame {
                    Some(Frame::Refusal { id: at, refusal }) if *at == id => {
                        Some(refusal.message().to_owned())
                    }
                    _ => None,
                };
                match self.absorb(arrival).await {
                    Absorbed::Ended => {
                        return Err(Fault::Environmental(
                            "the sidecar exited while opening".to_owned(),
                        ));
                    }
                    Absorbed::Malformed(error) => return Err(Fault::Environmental(error)),
                    Absorbed::Carried => {}
                }
                if let Some(message) = refused {
                    return Err(Fault::Application(format!(
                        "the sidecar refused `{method}`: {message}"
                    )));
                }
                if let Some(result) = answered {
                    return Ok(result);
                }
            }
        })
        .await;

        opened.unwrap_or_else(|_| {
            Err(Fault::Environmental(
                "the sidecar did not open inside its handshake".to_owned(),
            ))
        })
    }

    async fn arrival(&mut self) -> Option<Arrival> {
        tokio::select! {
            reply = self.returned.recv() => reply.map(|(frame, token)| Arrival::Reply(frame, token)),
            arrival = self.inbound.recv() => arrival,
        }
    }

    async fn serve(
        &mut self,
        requests: &mut mpsc::UnboundedReceiver<Request>,
        mut initialized: tokio::sync::watch::Receiver<bool>,
        on_ready: impl FnOnce(&mut Service<A>) -> Result<(), String>,
    ) -> bool {
        let plugin = self
            .plugin
            .clone()
            .expect("canonical native execution retains its original scope");
        let activation = plugin.ready();
        tokio::pin!(activation);
        let mut committed = plugin.is_ready();
        let mut activation_pending = !committed;
        let mut on_ready = Some(on_ready);
        loop {
            if *initialized.borrow_and_update()
                && committed
                && plugin.is_ready()
                && let Some(ready) = on_ready.take()
                && let Err(error) = ready(self.context)
            {
                self.failure = Some(error);
                return false;
            }
            let step = if on_ready.is_some() {
                tokio::select! {
                    changed = initialized.changed() => {
                        if changed.is_err() { return false; }
                        continue;
                    }
                    result = &mut activation, if activation_pending => {
                        activation_pending = false;
                        // The Lifecycle Owner May Already Be Shutting Down.
                        // Its Retained Service Lease Still Owns SDK Cleanup.
                        committed = result.is_ok();
                        continue;
                    }
                    step = self.step(requests) => step,
                }
            } else {
                self.step(requests).await
            };
            match step {
                Step::Stopping => return true,
                Step::Published(Err(crate::streams::StreamPublicationError::Revoked))
                    if plugin.is_closed() && !plugin.has_committed() =>
                {
                    // Aborted Staging Discards Rows Without Ending SDK Cleanup.
                }
                Step::Published(publication) => {
                    let delivered = publication
                        .map_err(|error| error.to_string())
                        .and_then(|publication| self.deliver(publication));
                    if let Err(error) = delivered {
                        self.failure = Some(error);
                        return false;
                    }
                }
                Step::Arrived(arrival) => match self.absorb(arrival).await {
                    Absorbed::Carried => {}
                    Absorbed::Malformed(error) => {
                        self.failure = Some(error);
                        return false;
                    }
                    Absorbed::Ended => return false,
                },
                Step::Cancelled(id) => {
                    self.child_scopes.remove(&id);
                    self.pending.remove(&id);
                    if self.send(Frame::Cancel { id }).await.is_err() {
                        return false;
                    }
                }
                Step::Requested(request) => {
                    if request.answer.is_closed() {
                        continue;
                    }
                    if request.children.is_some() && self.child_scopes.len() >= 128 {
                        let _ = request.answer.send(Err(SidecarError::refused(
                            UNKNOWN,
                            "native attempt context capacity exhausted",
                        )));
                        continue;
                    }
                    let id = self.mint();
                    let frame = Frame::Call {
                        id,
                        method: request.method,
                        params: request.params,
                    };
                    if !frame.fits() {
                        let _ = request.answer.send(Err(SidecarError::refused(
                            Frame::OVERSIZED,
                            format!(
                                "the route request exceeds the {}-byte sidecar frame limit",
                                Frame::LIMIT
                            ),
                        )));
                        continue;
                    }
                    match self.send(frame).await {
                        Ok(()) => {
                            if let Some(children) = request.children {
                                self.child_scopes.insert(id, children);
                            }
                            self.pending.insert(id, request.answer);
                        }
                        Err(_) => {
                            let _ = request.answer.send(Err(SidecarError::Unavailable));
                        }
                    }
                }
            }
        }
    }

    async fn step(&mut self, requests: &mut mpsc::UnboundedReceiver<Request>) -> Step {
        let Self {
            inbound,
            returned,
            context,
            pending,
            deliveries,
            delivery_turn,
            manager_owned_stop,
            ..
        } = self;
        let mut stopping = std::pin::pin!(context.stopping());

        std::future::poll_fn(|cx| {
            if !*manager_owned_stop && stopping.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Step::Stopping);
            }
            for (id, answer) in pending.iter_mut() {
                if answer.poll_closed(cx).is_ready() {
                    return Poll::Ready(Step::Cancelled(*id));
                }
            }
            if let Poll::Ready(Some((frame, token))) = returned.poll_recv(cx) {
                return Poll::Ready(Step::Arrived(Arrival::Reply(frame, token)));
            }
            if !*delivery_turn && let Poll::Ready(arrival) = inbound.poll_recv(cx) {
                *delivery_turn = true;
                return Poll::Ready(Step::Arrived(arrival.unwrap_or(Arrival::Ended)));
            }
            if let Some(delivery) = deliveries.front_mut()
                && let Poll::Ready(publication) = delivery.as_mut().poll(cx)
            {
                deliveries.pop_front();
                *delivery_turn = false;
                return Poll::Ready(Step::Published(publication));
            }
            if let Poll::Ready(arrival) = inbound.poll_recv(cx) {
                *delivery_turn = true;
                return Poll::Ready(Step::Arrived(arrival.unwrap_or(Arrival::Ended)));
            }
            if let Poll::Ready(Some(request)) = requests.poll_recv(cx) {
                return Poll::Ready(Step::Requested(request));
            }
            Poll::Pending
        })
        .await
    }

    async fn absorb(&mut self, arrival: Arrival) -> Absorbed {
        let (arrival, receipt) = match arrival {
            Arrival::Binary(received) => {
                if received.cancelled && matches!(received.frame, Frame::Call { .. }) {
                    drop(received.frame);
                    if let Some(receipt) = received.consumption {
                        let _ = receipt.consumed();
                    }
                    return Absorbed::Carried;
                }
                (Arrival::Frame(received.frame), received.consumption)
            }
            arrival => (arrival, None),
        };
        let result = self.absorb_frame(arrival).await;
        if let Some(receipt) = receipt {
            let _ = receipt.consumed();
        }
        result
    }

    async fn absorb_frame(&mut self, arrival: Arrival) -> Absorbed {
        match arrival {
            Arrival::Binary(_) => unreachable!("binary ownership is consumed by absorb"),
            Arrival::Publications(receipts) => {
                self.deliveries.extend(
                    receipts
                        .into_iter()
                        .map(|receipt| Box::pin(receipt.committed()) as _),
                );
                Absorbed::Carried
            }
            Arrival::ObserverFailed(error) => Absorbed::Malformed(error),
            Arrival::Demanded { stream, wanted } => {
                self.demand.send_modify(|demand| {
                    demand.insert(stream, wanted);
                });
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Answer { id, result }) => {
                self.settle(id, Ok(result));
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Refusal { id, refusal }) => {
                let refused = SidecarError::Refused {
                    code: refusal.code(),
                    failure: refusal.into_failure(),
                };
                self.settle(id, Err(refused));
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Push { method, params }) => {
                if method == Frame::EXITING {
                    self.announced = Some(reason(&params));
                } else {
                    return Absorbed::Malformed(
                        "application notifications require an admitted operation".to_owned(),
                    );
                }
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Call { id, method, params }) => {
                if self.routed_tasks.contains_key(&id) {
                    return Absorbed::Malformed("duplicate live route call id".to_owned());
                }
                if method == crate::operations::CHILD_METHOD {
                    let submitted = executor::now();
                    let child = match crate::operations::WireChild::decode(&params) {
                        Ok(child) => child,
                        Err(error) => {
                            let refusal = harmos_domain::Refusal::of(error);
                            return self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                                |error| Absorbed::Malformed(error.to_string()),
                                |()| Absorbed::Carried,
                            );
                        }
                    };
                    let scope = self
                        .child_scopes
                        .values()
                        .find(|scope| &scope.context == child.context())
                        .cloned();
                    let admission = if self.routed_tasks.len() >= CHILD_REQUESTS {
                        Err(crate::plugin::DispatchError::Busy.into_refusal())
                    } else if let Some(scope) = scope {
                        Ok(Some(scope))
                    } else {
                        Ok(None)
                    };
                    let refusal = match admission {
                        Err(refusal) => refusal,
                        Ok(scope) => {
                            let (cancellation, cancelled) = oneshot::channel();
                            let invocation = match scope {
                                Some(scope) => Box::pin(async move {
                                    match child {
                                        crate::operations::WireChild::Call(call) => {
                                            scope.call(call, submitted, cancelled).await
                                        }
                                        crate::operations::WireChild::Control(control) => {
                                            scope.control(control, cancelled).await
                                        }
                                    }
                                })
                                    as Pin<
                                        Box<
                                            dyn Future<
                                                    Output = Result<
                                                        Vec<u8>,
                                                        harmos_domain::Refusal,
                                                    >,
                                                > + Send,
                                        >,
                                    >,
                                None => {
                                    // Submitted Children Belong To An Attempt Scope
                                    let call = match child {
                                        crate::operations::WireChild::Call(call)
                                            if !call.submit =>
                                        {
                                            call
                                        }
                                        _ => {
                                            let refusal = harmos_domain::Refusal::new(
                                                harmos_domain::UNKNOWN,
                                                "submitted children require an attempt context",
                                            );
                                            return self
                                                .send(Frame::Refusal { id, refusal })
                                                .await
                                                .map_or_else(
                                                    |error| Absorbed::Malformed(error.to_string()),
                                                    |()| Absorbed::Carried,
                                                );
                                        }
                                    };
                                    match if call.generation_context.as_ref().is_some_and(|token| {
                                        self.plugin.as_ref().is_some_and(|scope| {
                                            scope.accepts_generation_context(token)
                                        })
                                    }) {
                                        self.caller
                                            .as_ref()
                                            .ok_or(crate::Error::Closing {
                                                domain: "native generation caller",
                                            })
                                            .and_then(|caller| {
                                                caller.submit_encoded_at(
                                                    &call.operation,
                                                    call.payload,
                                                    submitted,
                                                    call.values,
                                                )
                                            })
                                    } else {
                                        Err(crate::Error::Closing {
                                            domain: "native attempt or generation context",
                                        })
                                    } {
                                        Ok(invocation) => Box::pin(async move {
                                            crate::operations::children::lifecycle(
                                                invocation, cancelled,
                                            )
                                            .await
                                            .map_err(crate::operations::children::refused)
                                        }),
                                        Err(error) => {
                                            let refusal =
                                                crate::operations::children::refused(error);
                                            return self
                                                .send(Frame::Refusal { id, refusal })
                                                .await
                                                .map_or_else(
                                                    |error| Absorbed::Malformed(error.to_string()),
                                                    |()| Absorbed::Carried,
                                                );
                                        }
                                    }
                                }
                            };
                            let returns = self.returns.clone();
                            let token = Arc::new(());
                            let task_token = Arc::clone(&token);
                            let future = async move {
                                let frame = match invocation.await {
                                    Ok(result) => Frame::Answer { id, result },
                                    Err(error) => Frame::Refusal { id, refusal: error },
                                };
                                let _ = returns.send((frame, task_token)).await;
                            };
                            self.routed_tasks.insert(
                                id,
                                RoutedTask {
                                    running: executor::task(future),
                                    token,
                                    cancellation: Some(cancellation),
                                    failure: None,
                                },
                            );
                            return Absorbed::Carried;
                        }
                    };
                    return self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                        |error| Absorbed::Malformed(error.to_string()),
                        |()| Absorbed::Carried,
                    );
                }
                let refusal = if self.routed_tasks.len() >= CHILD_REQUESTS {
                    crate::plugin::DispatchError::Busy.into_refusal()
                } else if let Some(caller) = &self.caller {
                    match caller.submit_encoded(&method, params) {
                        Ok(invocation) => {
                            let returns = self.returns.clone();
                            let token = Arc::new(());
                            let task_token = Arc::clone(&token);
                            let (cancellation, cancelled) = oneshot::channel();
                            let future = async move {
                                let outcome =
                                    crate::operations::children::lifecycle(invocation, cancelled)
                                        .await;
                                let frame = match outcome {
                                    Ok(result) => Frame::Answer { id, result },
                                    Err(error) => {
                                        let refusal = crate::operations::children::refused(error);
                                        Frame::Refusal { id, refusal }
                                    }
                                };
                                let _ = returns.send((frame, task_token)).await;
                            };
                            self.routed_tasks.insert(
                                id,
                                RoutedTask {
                                    running: executor::task(future),
                                    token,
                                    cancellation: Some(cancellation),
                                    failure: None,
                                },
                            );
                            return Absorbed::Carried;
                        }
                        Err(error) => crate::operations::children::refused(error),
                    }
                } else {
                    harmos_domain::Refusal::new(
                        -32_606,
                        "no managed operation caller is configured",
                    )
                };
                self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                    |error| Absorbed::Malformed(error.to_string()),
                    |()| Absorbed::Carried,
                )
            }
            Arrival::Frame(Frame::Cancel { id }) => {
                if let Some(task) = self.routed_tasks.get_mut(&id)
                    && let Some(cancel) = task.cancellation.take()
                {
                    let _ = cancel.send(());
                }
                // Keep The Slot Until Its Original OperationInstance Physically Settles
                Absorbed::Carried
            }
            Arrival::Reply(frame, token) => {
                let id = match &frame {
                    Frame::Answer { id, .. } | Frame::Refusal { id, .. } => *id,
                    _ => unreachable!("routed tasks produce only replies"),
                };
                // A cancelled id can be reused before its already-queued reply
                // is consumed. Only the task that owns the current slot may reply.
                if !self
                    .routed_tasks
                    .get(&id)
                    .is_some_and(|current| Arc::ptr_eq(&current.token, &token))
                {
                    return Absorbed::Carried;
                }
                let task = self
                    .routed_tasks
                    .get_mut(&id)
                    .expect("the reply owns its slot");
                if let Err(error) = (&mut task.running).await {
                    let failure = format!(
                        "native operation drain failed: {}",
                        harmos_domain::failure::Report(&error)
                    );
                    task.failure = Some(failure.clone());
                    *self.cleanup_failure = Some(failure.clone());
                    return Absorbed::Malformed(failure);
                }
                self.routed_tasks.remove(&id);
                self.send(frame).await.map_or_else(
                    |error| Absorbed::Malformed(error.to_string()),
                    |()| Absorbed::Carried,
                )
            }
            Arrival::Diagnostic(line) => {
                (self.notice)(self.context, Notice::Logged { line });
                Absorbed::Carried
            }
            Arrival::Ended => Absorbed::Ended,
        }
    }

    fn deliver(&mut self, publication: ManagedPublication) -> Result<(), String> {
        if let Some(ingest) = self.ingress.streams.get(publication.stream()) {
            ingest(self.context, &publication)?;
            return Ok(());
        }

        (self.notice)(self.context, Notice::Published { publication });

        Ok(())
    }

    fn settle(&mut self, id: u64, answer: Result<Vec<u8>, SidecarError>) {
        self.child_scopes.remove(&id);
        if let Some(waiting) = self.pending.remove(&id) {
            let _ = waiting.send(answer);
        }
    }

    fn mint(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    async fn send(&mut self, frame: Frame) -> std::io::Result<()> {
        self.outgoing.send(frame).await
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
    ingress: &Ingress<A>,
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
