/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// An observer is a resident body with the shared lifecycle verbs: it
// initializes (retrying each cadence until it succeeds), observes once per
// declared interval for as long as its activation runs, and stops and cleans
// up when that activation is cancelled. `Activity` is that body for one
// activation; whoever runs the activation owns its lifecycle.

mod attachment;

pub use attachment::ObserverAttachment;
pub use harmos_domain::inventory::observers::{Activation, DemandKey};

use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::FutureExt;
use tokio::time::{Instant, MissedTickBehavior, interval};

use super::identity::{ActivationId, Identity};
use super::{ObserverCaller, ObserverHost, ObserverPublisher, ObserverStreamPublisher, PublicationError, Supervisor};
use harmos_domain::inventory::publications;

type ContextFactory<H> = Arc<dyn Fn(&H) -> Arc<dyn Any + Send + Sync> + Send + Sync>;
type Open<H> = fn(Arc<harmos_sdk::ExecutionContext>, H, Option<ContextFactory<H>>, Identity) -> Result<Activity, ObserverError>;
type Step<'a> = Pin<Box<dyn Future<Output = Result<(), ObserverError>> + Send + 'a>>;

pub struct Declarations<H = Supervisor>(std::marker::PhantomData<H>);

pub struct Inventory<H = Supervisor>(std::marker::PhantomData<H>);

pub struct Outputs<H = Supervisor>(std::marker::PhantomData<H>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    ordinal: u64,
    elapsed: Duration,
}

pub struct ObserverContext<'a, H = Supervisor> {
    identity: Identity,
    resources: &'a harmos_sdk::ExecutionContext,
    tick: &'a Tick,
    host: &'a H,
    streams: &'static [publications::Entry],
    extension: Option<&'a (dyn Any + Send + Sync)>,
}

pub struct Instance<H = Supervisor> {
    entry: harmos_domain::inventory::observers::Entry,
    open: Open<H>,
    context: Option<ContextFactory<H>>,
}

// One activation of one observer: its resources, bound host and, once
// initialized, the observer itself. Dropping an `observe` future interrupts the
// activation; `stop` and `cleanup` then release whatever it acquired.
pub struct Activity {
    identity: Identity,
    body: Box<dyn Body>,
}

struct Bound<S: Observer<H>, H: Send + Sync + 'static> {
    identity: Identity,
    resources: Arc<harmos_sdk::ExecutionContext>,
    host: H,
    extension: Option<Arc<dyn Any + Send + Sync>>,
    observer: Option<S>,
    stopped: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("the observer host or context factory panicked")]
    BindingPanicked,

    #[error("the observer activation identity could not be drawn: {0}")]
    Identity(#[source] std::io::Error),

    #[error("the observer callback panicked")]
    CallbackPanicked,

    #[error("the observer tick ordinal is exhausted")]
    TickExhausted,

    #[error("the observer refused to stop: {0}")]
    StopFailed(#[source] Arc<dyn std::error::Error + Send + Sync>),

    #[error("the observer panicked while stopping")]
    StopPanicked,

    #[error("the observer refused to clean up: {0}")]
    CleanupFailed(#[source] Arc<dyn std::error::Error + Send + Sync>),

    #[error("the observer panicked while cleaning up")]
    CleanupPanicked,
}

trait Body: Send {
    fn observe<'a>(&'a mut self, ready: &'a (dyn Fn() + Send + Sync)) -> Step<'a>;
    fn stop(&mut self) -> Step<'_>;
    fn cleanup(&mut self) -> Step<'_>;
    fn is_initialized(&self) -> bool;
}

pub trait Observer<H: Send + Sync + 'static = Supervisor>: Sized + Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    const ID: &'static str;

    const DESCRIPTION: &'static str = "";

    const STREAMS: &'static [publications::Entry] = &[];

    const DEMAND_KEYS: &'static [DemandKey] = &[];

    const ACTIVATION: Activation = Activation::Plugin;

    const INTERVAL: Duration;

    fn initialize(resources: &harmos_sdk::ExecutionContext) -> impl Future<Output = Result<Self, Self::Error>> + Send;

    fn observe(&mut self, ctx: &ObserverContext<'_, H>) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }

    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

impl<H: Send + Sync + 'static> harmos_domain::catalog::Visitor for Declarations<H> {
    type Entry = fn() -> Instance<H>;
}

impl<H: Send + Sync + 'static, S: Observer<H>> harmos_domain::catalog::Member<S> for Declarations<H> {
    const ENTRY: fn() -> Instance<H> = Instance::of::<S>;
}

impl<H: Send + Sync + 'static> harmos_domain::catalog::Visitor for Outputs<H> {
    type Entry = &'static [publications::Entry];
}

impl<H: Send + Sync + 'static, S: Observer<H>> harmos_domain::catalog::Member<S> for Outputs<H> {
    const ENTRY: &'static [publications::Entry] = S::STREAMS;
}

impl<H: Send + Sync + 'static> harmos_domain::catalog::Visitor for Inventory<H> {
    type Entry = harmos_domain::inventory::observers::Entry;
}

impl<H: Send + Sync + 'static, S: Observer<H>> harmos_domain::catalog::Member<S> for Inventory<H> {
    const ENTRY: harmos_domain::inventory::observers::Entry = harmos_domain::inventory::observers::Entry {
        id: S::ID,
        description: S::DESCRIPTION,
        interval: S::INTERVAL,
        activation: S::ACTIVATION,
        streams: S::STREAMS,
        demand_keys: S::DEMAND_KEYS,
    };
}

impl Tick {
    fn advance(&mut self, elapsed: Duration) -> Result<(), ObserverError> {
        let ordinal = self.ordinal.checked_add(1).ok_or(ObserverError::TickExhausted)?;
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

impl<H> ObserverContext<'_, H> {
    pub fn observer_id(&self) -> &'static str {
        self.identity.observer
    }

    pub fn activation_id(&self) -> ActivationId {
        self.identity.activation
    }

    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, harmos_sdk::operations::ResourceError> {
        self.resources.resource::<T>()
    }

    pub fn tick(&self) -> &Tick {
        self.tick
    }

    pub fn host(&self) -> &H {
        self.host
    }

    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<&T> {
        let extension = self.extension?;
        extension.downcast_ref()
    }
}

impl ObserverContext<'_, Supervisor> {
    pub fn supervisor(&self) -> &Supervisor {
        self.host
    }
}

impl<H> ObserverContext<'_, H> {
    pub async fn publish<V: Send>(&self, value: V) -> Result<(), H::Error>
    where
        H: ObserverPublisher<V>,
    {
        if let Some(stream) = H::STREAM {
            self.check_stream(stream.id, stream.schema)?;
        }
        self.host.publish(self, value).await
    }

    pub async fn publish_on<S: harmos_domain::StreamSpec>(&self, row: &S::Row) -> Result<(), H::Error>
    where
        S::Row: Sync,
        H: ObserverStreamPublisher<S>,
    {
        self.check_stream(S::ID, S::VERSION)?;
        self.host.publish_on(self, row).await
    }

    pub async fn call<C>(&self, request: H::Request) -> Result<H::Response, H::Error>
    where
        H: ObserverCaller<C>,
    {
        self.host.call(self, request).await
    }

    fn check_stream(&self, id: &'static str, version: u32) -> Result<(), PublicationError> {
        if self.streams.iter().any(|entry| entry.id == id && entry.schema == version) {
            Ok(())
        } else {
            Err(PublicationError::Undeclared { stream: id })
        }
    }
}

impl<H: Send + Sync + 'static> Instance<H> {
    pub fn of<S: Observer<H>>() -> Self {
        assert!(
            !S::ID.trim().is_empty() && !S::ID.chars().any(char::is_control),
            "an observer identity must be nonempty and contain no control characters"
        );
        assert!(!S::INTERVAL.is_zero(), "an observer interval must be nonzero");
        assert!(
            S::ACTIVATION != Activation::Demand || !S::STREAMS.is_empty() || !S::DEMAND_KEYS.is_empty(),
            "a demand observer must declare streams or demand keys"
        );

        for key in S::DEMAND_KEYS {
            assert!(!S::STREAMS.iter().any(|entry| entry.id == key.name()), "demand keys must differ from publication identities");
        }

        Self {
            entry: <Inventory<H> as harmos_domain::catalog::Member<S>>::ENTRY,
            open: Activity::open::<S, H>,
            context: None,
        }
    }

    pub fn with_context<T: Send + Sync + 'static>(mut self, factory: impl Fn(&H) -> T + Send + Sync + 'static) -> Self {
        let erased = move |host: &H| -> Arc<dyn Any + Send + Sync> {
            let extension = factory(host);
            Arc::new(extension)
        };
        self.context = Some(Arc::new(erased));
        self
    }

    pub fn declaration(&self) -> harmos_domain::inventory::observers::Declaration {
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

    // Open one activation over its resources and bound host. Nothing runs
    // until the caller drives `observe`.
    pub fn activate(&self, resources: Arc<harmos_sdk::ExecutionContext>, host: H) -> Result<Activity, ObserverError> {
        let identity = Identity::new(self.entry.id).map_err(ObserverError::Identity)?;
        (self.open)(resources, host, self.context.clone(), identity)
    }
}

impl Activity {
    fn open<S: Observer<H>, H: Send + Sync + 'static>(
        resources: Arc<harmos_sdk::ExecutionContext>, host: H, context: Option<ContextFactory<H>>, identity: Identity,
    ) -> Result<Self, ObserverError> {
        let create_extension = || match context {
            Some(factory) => {
                let extension = factory(&host);
                Some(extension)
            }
            None => None,
        };
        let guarded_extension = AssertUnwindSafe(create_extension);
        let extension = std::panic::catch_unwind(guarded_extension).map_err(|_| {
            tracing::error!(
                observer = identity.observer, activation_id = %identity.activation,
                "observer context factory panicked; activation refused"
            );
            ObserverError::BindingPanicked
        })?;
        let bound = Bound::<S, H> {
            identity,
            resources,
            host,
            extension,
            observer: None,
            stopped: false,
        };
        Ok(Self { identity, body: Box::new(bound) })
    }

    pub fn observer_id(&self) -> &'static str {
        self.identity.observer
    }

    pub fn activation_id(&self) -> ActivationId {
        self.identity.activation
    }

    pub fn is_initialized(&self) -> bool {
        self.body.is_initialized()
    }

    // Initialize at the declared cadence until it succeeds, report readiness,
    // then observe once per interval. Returns only when the activation can no
    // longer continue; dropping the future interrupts it.
    pub async fn observe(&mut self, ready: impl Fn() + Send + Sync) -> ObserverError {
        match self.body.observe(&ready).await {
            Ok(()) => unreachable!("observation runs until it is interrupted or fails"),
            Err(failure) => failure,
        }
    }

    pub async fn stop(&mut self) -> Result<(), ObserverError> {
        self.body.stop().await
    }

    pub async fn cleanup(&mut self) -> Result<(), ObserverError> {
        self.body.cleanup().await
    }
}

impl<S: Observer<H>, H: Send + Sync + 'static> Body for Bound<S, H> {
    fn observe<'a>(&'a mut self, ready: &'a (dyn Fn() + Send + Sync)) -> Step<'a> {
        Box::pin(async move {
            let name = self.identity.observer;
            let activation = self.identity.activation;
            let mut clock = interval(S::INTERVAL);
            clock.set_missed_tick_behavior(MissedTickBehavior::Skip);

            // Initialization Owns Its Partial Acquisitions; Only Success Is Cleaned Up
            while self.observer.is_none() {
                clock.tick().await;
                match AssertUnwindSafe(S::initialize(&self.resources)).catch_unwind().await {
                    Ok(Ok(observer)) => self.observer = Some(observer),
                    Ok(Err(error)) => tracing::warn!(
                        observer = name, activation_id = %activation, error = %harmos_domain::failure::Report(&error),
                        "observer initialization failed; retrying next cadence"
                    ),
                    Err(_) => tracing::error!(
                        observer = name, activation_id = %activation,
                        "observer initialization panicked; retrying next cadence"
                    ),
                }
            }
            ready();

            // Observe Once Per Declared Interval
            let opened = Instant::now();
            clock.reset();
            let mut tick = Tick { ordinal: 0, elapsed: Duration::ZERO };
            let observer = self.observer.as_mut().expect("initialized observer");
            loop {
                clock.tick().await;
                advance_tick(&mut tick, opened.elapsed(), self.identity)?;
                let context = ObserverContext {
                    identity: self.identity,
                    resources: self.resources.as_ref(),
                    tick: &tick,
                    host: &self.host,
                    extension: self.extension.as_deref(),
                    streams: S::STREAMS,
                };
                let observed = AssertUnwindSafe(async { observer.observe(&context).await }).catch_unwind().await;
                observed_result(observed, self.identity, tick.ordinal())?;
            }
        })
    }

    fn stop(&mut self) -> Step<'_> {
        Box::pin(async move {
            let Some(observer) = self.observer.as_mut() else {
                return Ok(());
            };
            if self.stopped {
                return Ok(());
            }
            guarded(async { observer.stop().await }, ObserverError::StopFailed, ObserverError::StopPanicked).await?;
            self.stopped = true;
            Ok(())
        })
    }

    fn cleanup(&mut self) -> Step<'_> {
        Box::pin(async move {
            let Some(observer) = self.observer.as_mut() else {
                return Ok(());
            };
            guarded(async { observer.cleanup().await }, ObserverError::CleanupFailed, ObserverError::CleanupPanicked).await?;
            self.observer = None;
            Ok(())
        })
    }

    fn is_initialized(&self) -> bool {
        self.observer.is_some()
    }
}

pub(super) fn validate_outputs<H>(instances: &[Instance<H>], grants: &[publications::Entry]) -> Result<(), String> {
    let outputs = instances.iter().flat_map(|instance| instance.entry.streams);
    let missing = |output: &&publications::Entry| !matches_output(output, grants);
    let mut invalid = outputs.filter(missing);
    match invalid.next() {
        Some(output) => {
            let message = format!("observer output `{}` differs from the artifact publication grant", output.id);
            Err(message)
        }
        None => Ok(()),
    }
}

fn observed_result<E: std::error::Error + 'static>(
    observed: Result<Result<(), E>, Box<dyn Any + Send>>, identity: Identity, tick: u64,
) -> Result<(), ObserverError> {
    match observed {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            tracing::warn!(observer = identity.observer, activation_id = %identity.activation,
                tick, error = %harmos_domain::failure::Report(&error), "observer callback refused");
            Ok(())
        }
        Err(_) => {
            tracing::error!(observer = identity.observer, activation_id = %identity.activation,
                tick, "observer callback panicked; activation stopping");
            Err(ObserverError::CallbackPanicked)
        }
    }
}

fn matches_output(output: &publications::Entry, grants: &[publications::Entry]) -> bool {
    grants
        .iter()
        .any(|grant| grant.id == output.id && grant.schema == output.schema && grant.window == output.window && grant.row == output.row)
}

fn advance_tick(tick: &mut Tick, elapsed: Duration, identity: Identity) -> Result<(), ObserverError> {
    let result = tick.advance(elapsed);
    if result.is_err() {
        tracing::error!(observer = identity.observer, activation_id = %identity.activation,
            "observer tick ordinal exhausted; activation stopping");
    }
    result
}

async fn guarded<E: std::error::Error + Send + Sync + 'static>(
    work: impl Future<Output = Result<(), E>>, refused: fn(Arc<dyn std::error::Error + Send + Sync>) -> ObserverError,
    panicked: ObserverError,
) -> Result<(), ObserverError> {
    match AssertUnwindSafe(work).catch_unwind().await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => {
            let source = Arc::new(source);
            let error = refused(source);
            Err(error)
        }
        Err(_) => Err(panicked),
    }
}

#[cfg(test)]
mod tests {
    use super::{Identity, ObserverContext, Tick};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn context_resource_resolution() {
        let value = Arc::new(17_u32);
        let mut table = harmos_sdk::__private::WireTable::new("observer").unwrap();
        table.provide_resource(Arc::clone(&value)).unwrap();
        let resources = table.context();
        let tick = Tick { ordinal: 1, elapsed: Duration::ZERO };
        let context = ObserverContext {
            identity: Identity::new("resource_observer").unwrap(),
            resources: &resources,
            tick: &tick,
            host: &(),
            streams: &[],
            extension: None,
        };

        let resolved = context.resource::<u32>().unwrap();
        assert!(Arc::ptr_eq(&value, &resolved));
        assert!(context.resource::<u64>().is_err());
    }
}
