/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod attachment;
mod control;
mod lifecycle;

/* === Re-Exports === */
pub use attachment::ObserverAttachment;
pub(crate) use control::Controlled;
pub use harmos_contract::inventory::observers::{Activation, DemandKey};
pub use lifecycle::{ObserverExit, ObserverStatus};

/* === Imports === */
// Standard Library Imports
use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior, interval};
use tracing::Instrument;
// Local Imports
use super::identity::{ActivationId, Identity};
use super::{
    ObserverCaller, ObserverHost, ObserverPublisher, ObserverStreamPublisher, PublicationError,
    Supervisor,
};
use harmos_contract::inventory::publications;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type ContextFactory<H> = Arc<dyn Fn(&H) -> Arc<dyn Any + Send + Sync> + Send + Sync>;
type HostFactory<H> = Box<dyn FnOnce() -> H + Send>;
type Start<R, H> = fn(
    Arc<R>,
    HostFactory<H>,
    Option<ContextFactory<H>>,
    Identity,
) -> std::io::Result<ObserverTask>;

/* === Structs === */

pub struct Declarations<R, H = Supervisor>(std::marker::PhantomData<(R, H)>);

pub struct Inventory<H = Supervisor>(std::marker::PhantomData<H>);

pub struct Outputs<H = Supervisor>(std::marker::PhantomData<H>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    ordinal: u64,
    elapsed: Duration,
}

pub struct ObserverContext<'a, R, H = Supervisor> {
    identity: Identity,
    resources: &'a R,
    tick: &'a Tick,
    capabilities: &'a H,
    streams: &'static [publications::Entry],
    extension: Option<&'a (dyn Any + Send + Sync)>,
}

pub struct Instance<R, H = Supervisor> {
    entry: harmos_contract::inventory::observers::Entry,
    open: Start<R, H>,
    context: Option<ContextFactory<H>>,
}

#[must_use = "dropping an observer task requests cancellation"]
pub struct ObserverTask {
    identity: Identity,
    status: watch::Sender<ObserverStatus>,
    control: Arc<lifecycle::Control>,
    ending: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
    interrupted: Option<Arc<tokio::task::JoinError>>,
}

/* === Enums === */

/* === Traits === */
pub trait Observer<H: Send + Sync + 'static = Supervisor>: Sized + Send + 'static {
    type Resources: Send + Sync + 'static;

    type Error: std::error::Error + Send + Sync + 'static;

    const ID: &'static str;

    const DESCRIPTION: &'static str = "";

    const STREAMS: &'static [publications::Entry] = &[];

    const DEMAND_KEYS: &'static [DemandKey] = &[];

    const ACTIVATION: Activation = Activation::Plugin;

    const INTERVAL: Duration;

    fn initialize(
        resources: &Self::Resources,
    ) -> impl Future<Output = Result<Self, Self::Error>> + Send;

    fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Self::Resources, H>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }

    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/* === Implementations === */
impl<R: Send + Sync + 'static, H: Send + Sync + 'static> harmos_contract::catalog::Visitor
    for Declarations<R, H>
{
    type Entry = fn() -> Instance<R, H>;
}

impl<R: Send + Sync + 'static, H: Send + Sync + 'static, S: Observer<H, Resources = R>>
    harmos_contract::catalog::Member<S> for Declarations<R, H>
{
    const ENTRY: fn() -> Instance<R, H> = Instance::of::<S>;
}

impl<H: Send + Sync + 'static> harmos_contract::catalog::Visitor for Outputs<H> {
    type Entry = &'static [publications::Entry];
}

impl<H: Send + Sync + 'static, S: Observer<H>> harmos_contract::catalog::Member<S> for Outputs<H> {
    const ENTRY: &'static [publications::Entry] = S::STREAMS;
}

impl<H: Send + Sync + 'static> harmos_contract::catalog::Visitor for Inventory<H> {
    type Entry = harmos_contract::inventory::observers::Entry;
}

impl<H: Send + Sync + 'static, S: Observer<H>> harmos_contract::catalog::Member<S>
    for Inventory<H>
{
    const ENTRY: harmos_contract::inventory::observers::Entry =
        harmos_contract::inventory::observers::Entry {
            id: S::ID,
            description: S::DESCRIPTION,
            interval: S::INTERVAL,
            activation: S::ACTIVATION,
            streams: S::STREAMS,
            demand_keys: S::DEMAND_KEYS,
        };
}

impl Tick {
    fn advance(&mut self, elapsed: Duration) -> Result<(), ObserverExit> {
        let ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(ObserverExit::TickExhausted)?;
        self.ordinal = ordinal;
        self.elapsed = elapsed;
        Ok(())
    }

    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

impl<R, H> ObserverContext<'_, R, H> {
    pub fn observer_id(&self) -> &'static str {
        self.identity.observer
    }

    pub fn activation_id(&self) -> ActivationId {
        self.identity.activation
    }

    pub fn resources(&self) -> &R {
        self.resources
    }

    pub fn tick(&self) -> &Tick {
        self.tick
    }

    pub fn capabilities(&self) -> &H {
        self.capabilities
    }

    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extension
            .and_then(|extension| extension.downcast_ref())
    }
}

impl<R> ObserverContext<'_, R, Supervisor> {
    pub fn supervisor(&self) -> &Supervisor {
        self.capabilities
    }
}

impl<R: Sync, H> ObserverContext<'_, R, H> {
    pub async fn publish<V: Send>(&self, value: V) -> Result<(), H::Error>
    where
        H: ObserverPublisher<R, V>,
    {
        if let Some(stream) = H::STREAM {
            self.check_stream(stream.id, stream.schema)?;
        }
        self.capabilities.publish(self, value).await
    }

    pub async fn publish_on<S: harmos_contract::StreamSpec>(
        &self,
        row: &S::Row,
    ) -> Result<(), H::Error>
    where
        S::Row: Sync,
        H: ObserverStreamPublisher<R, S>,
    {
        self.check_stream(S::ID, S::VERSION)?;
        self.capabilities.publish_on(self, row).await
    }

    pub async fn call_as<C>(&self, request: H::Request) -> Result<H::Response, H::Error>
    where
        H: ObserverCaller<R, C>,
    {
        self.capabilities.call(self, request).await
    }

    fn check_stream(&self, id: &'static str, version: u32) -> Result<(), PublicationError> {
        if self
            .streams
            .iter()
            .any(|entry| entry.id == id && entry.schema == version)
        {
            Ok(())
        } else {
            Err(PublicationError::Undeclared { stream: id })
        }
    }
}

impl<R: Send + Sync + 'static, H: Send + Sync + 'static> Instance<R, H> {
    pub fn of<S: Observer<H, Resources = R>>() -> Self {
        assert!(
            !S::ID.trim().is_empty() && !S::ID.chars().any(char::is_control),
            "an observer identity must be nonempty and contain no control characters"
        );
        assert!(
            !S::INTERVAL.is_zero(),
            "an observer interval must be nonzero"
        );
        assert!(
            S::ACTIVATION != Activation::Demand
                || !S::STREAMS.is_empty()
                || !S::DEMAND_KEYS.is_empty(),
            "a demand observer must declare streams or demand keys"
        );

        for key in S::DEMAND_KEYS {
            assert!(
                !S::STREAMS.iter().any(|entry| entry.id == key.name()),
                "demand keys must differ from publication identities"
            );
        }

        Self {
            entry: <Inventory<H> as harmos_contract::catalog::Member<S>>::ENTRY,
            open: |resources, host, extension, identity| {
                ObserverTask::open::<S, H>(resources, host, extension, identity)
            },
            context: None,
        }
    }

    pub fn with_context<T: Send + Sync + 'static>(
        mut self,
        factory: impl Fn(&H) -> T + Send + Sync + 'static,
    ) -> Self {
        self.context = Some(Arc::new(move |host| Arc::new(factory(host))));
        self
    }

    pub fn declaration(&self) -> harmos_contract::inventory::observers::Declaration {
        self.entry.declaration()
    }

    pub fn id(&self) -> &'static str {
        self.entry.id
    }

    pub fn streams(&self) -> &'static [publications::Entry] {
        self.entry.streams
    }

    pub fn demand_keys(&self) -> &'static [DemandKey] {
        self.entry.demand_keys
    }

    pub fn activation(&self) -> Activation {
        self.entry.activation
    }

    pub fn interval(&self) -> Duration {
        self.entry.interval
    }

    pub fn start(&self, resources: Arc<R>, capabilities: H) -> std::io::Result<ObserverTask> {
        self.start_with(resources, move || capabilities)
    }

    pub fn start_with(
        &self,
        resources: Arc<R>,
        factory: impl FnOnce() -> H + Send + 'static,
    ) -> std::io::Result<ObserverTask> {
        let identity = Identity::new(self.entry.id)?;
        (self.open)(resources, Box::new(factory), self.context.clone(), identity)
    }
}

impl ObserverTask {
    pub fn observer_id(&self) -> &'static str {
        self.identity.observer
    }

    pub fn activation_id(&self) -> ActivationId {
        self.identity.activation
    }

    fn open<S: Observer<H>, H: Send + Sync + 'static>(
        resources: Arc<S::Resources>,
        factory: HostFactory<H>,
        context: Option<ContextFactory<H>>,
        identity: Identity,
    ) -> std::io::Result<Self> {
        let (ending, ended) = oneshot::channel();
        let (lifecycle, control) = lifecycle::Lifecycle::new();
        let status = control.state.clone();

        let name = identity.observer;
        let span = tracing::info_span!("observer_activation", observer = name, activation_id = %identity.activation);
        let task = super::task::spawn(
            name.to_owned(),
            async move {
                let exit = async {
                    let (host, extension) = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let host = factory();
                        let extension = context.map(|factory| factory(&host));
                        (host, extension)
                    })) {
                        Ok(bound) => bound,
                        Err(_) => {
                            tracing::error!(
                                observer = name, activation_id = %identity.activation,
                                "observer host or context factory panicked; activation refused"
                            );
                            return ObserverExit::BindingPanicked;
                        }
                    };

                    drive::<S, H>(resources, host, extension, identity, ended, &lifecycle).await
                }
                .await;
                lifecycle.finish(exit);
            }
            .instrument(span),
        )?;

        Ok(Self {
            identity,
            status,
            control,
            ending: Some(ending),
            task: Some(task),
            interrupted: None,
        })
    }

    pub fn status(&self) -> ObserverStatus {
        *self.status.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<ObserverStatus> {
        self.status.subscribe()
    }

    pub fn cancel(&mut self) {
        if let Some(ending) = self.ending.take() {
            lifecycle::stopping(&self.status);
            let _ = ending.send(());
        }
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_none() && self.interrupted.is_none()
    }

    pub async fn join(&mut self) -> std::io::Result<()> {
        let mut changed = self.subscribe();
        loop {
            if let Some(failure) = &self.interrupted {
                return Err(std::io::Error::other(failure.clone()));
            }
            if let Some(failure) = self.control.failure() {
                return Err(failure);
            }
            let Some(task) = self.task.as_mut() else {
                return Ok(());
            };
            tokio::select! {
                result = task => {
                    self.task = None;
                    return result.map_err(|failure| {
                        let failure = Arc::new(failure);
                        self.interrupted = Some(failure.clone());
                        std::io::Error::other(failure)
                    });
                }
                _ = changed.changed() => {}
            }
        }
    }

    pub async fn recover(&mut self) -> std::io::Result<()> {
        self.cancel();
        self.control.recover();
        self.join().await
    }

    pub async fn stop(&mut self) -> std::io::Result<()> {
        self.cancel();
        self.join().await
    }
}

impl Drop for ObserverTask {
    fn drop(&mut self) {
        self.cancel();
    }
}

/* === Public Functions === */
pub(super) fn validate_outputs<R, H>(
    instances: &[Instance<R, H>],
    grants: &[publications::Entry],
) -> Result<(), String> {
    for output in instances.iter().flat_map(|instance| instance.entry.streams) {
        if !grants.iter().any(|grant| {
            grant.id == output.id
                && grant.schema == output.schema
                && grant.window == output.window
                && grant.row == output.row
        }) {
            return Err(format!(
                "observer output `{}` differs from the artifact publication grant",
                output.id
            ));
        }
    }
    Ok(())
}

/* === Private Functions === */
async fn drive<S: Observer<H>, H: Send + Sync + 'static>(
    resources: Arc<S::Resources>,
    host: H,
    extension: Option<Arc<dyn Any + Send + Sync>>,
    identity: Identity,
    mut ended: oneshot::Receiver<()>,
    lifecycle: &lifecycle::Lifecycle,
) -> ObserverExit {
    let name = identity.observer;
    let mut clock = interval(S::INTERVAL);
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // Each failed attempt releases its partial owner before the next cadence.
    let mut observer = loop {
        tokio::select! {
            biased;
            _ = &mut ended => return ObserverExit::Cancelled,
            _ = clock.tick() => {}
        }
        // Initialization Owns Its Partial Acquisitions; Only Success Is Cleaned Up
        let result = tokio::select! {
            biased;
            _ = &mut ended => None,
            result = AssertUnwindSafe(S::initialize(&resources)).catch_unwind() => Some(result),
        };
        match result {
            Some(Ok(Ok(observer))) => break observer,
            Some(Ok(Err(error))) => {
                tracing::warn!(observer = name, activation_id = %identity.activation, %error,
                    "observer initialization failed; retrying next cadence")
            }
            Some(Err(_)) => tracing::error!(
                observer = name, activation_id = %identity.activation,
                "observer initialization panicked; retrying next cadence"
            ),
            None => return ObserverExit::Cancelled,
        }
    };
    let opened = Instant::now();
    clock.reset();
    lifecycle.running();
    let mut tick = Tick {
        ordinal: 0,
        elapsed: Duration::ZERO,
    };

    let exit = loop {
        tokio::select! {
            biased;
            _ = &mut ended => break ObserverExit::Cancelled,
            _ = clock.tick() => {}
        }

        if let Err(exit) = tick.advance(opened.elapsed()) {
            tracing::error!(observer = name, activation_id = %identity.activation,
                "observer tick ordinal exhausted; activation stopping");
            break exit;
        }
        let ordinal = tick.ordinal();
        let context = ObserverContext {
            identity,
            resources: resources.as_ref(),
            tick: &tick,
            capabilities: &host,
            extension: extension.as_deref(),
            streams: S::STREAMS,
        };
        let result = tokio::select! {
            biased;
            _ = &mut ended => break ObserverExit::Cancelled,
            result = AssertUnwindSafe(async { observer.observe(&context).await }).catch_unwind() => result,
        };
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(observer = name, activation_id = %identity.activation, tick = ordinal, %error, "observer callback refused")
            }
            Err(_) => {
                tracing::error!(
                    observer = name, activation_id = %identity.activation,
                    tick = ordinal,
                    "observer callback panicked; activation stopping"
                );
                break ObserverExit::CallbackPanicked;
            }
        }
    };
    lifecycle.stopping();

    cleanup::<S, H>(&mut observer, identity, lifecycle, true)
        .await
        .unwrap_or(exit)
}

async fn cleanup<S: Observer<H>, H: Send + Sync + 'static>(
    observer: &mut S,
    identity: Identity,
    lifecycle: &lifecycle::Lifecycle,
    closing: bool,
) -> Option<ObserverExit> {
    let mut stopped = false;
    let mut outcome = None;
    loop {
        let result = {
            let work = async {
                if !stopped {
                    if closing || outcome.is_some() {
                        lifecycle.stopping();
                    }
                    guarded(
                        async { observer.stop().await },
                        ObserverExit::StopFailed,
                        ObserverExit::StopPanicked,
                    )
                    .await?;
                    stopped = true;
                }
                if closing || outcome.is_some() {
                    lifecycle.finalizing();
                }
                guarded(
                    async { observer.cleanup().await },
                    ObserverExit::FinalizeFailed,
                    ObserverExit::FinalizePanicked,
                )
                .await
            };
            tokio::pin!(work);
            match tokio::time::timeout(Duration::from_secs(1), &mut work).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!(observer = identity.observer, activation_id = %identity.activation,
                    "observer cleanup exceeded one second; retaining until completion");
                    work.await
                }
            }
        };
        match result {
            Ok(()) => return outcome,
            Err(failure) => {
                tracing::error!(observer = identity.observer, activation_id = %identity.activation,
                    %failure, "observer cleanup failed; retaining original owner");
                outcome = Some(failure.exit);
                lifecycle.retained(failure).await;
            }
        }
    }
}

async fn guarded<E: std::error::Error + Send + Sync + 'static>(
    work: impl Future<Output = Result<(), E>>,
    refused: ObserverExit,
    panicked: ObserverExit,
) -> Result<(), lifecycle::Failure> {
    match AssertUnwindSafe(work).catch_unwind().await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(lifecycle::Failure {
            exit: refused,
            source: Some(Arc::new(source)),
        }),
        Err(_) => Err(lifecycle::Failure {
            exit: panicked,
            source: None,
        }),
    }
}

/* === Tests === */
