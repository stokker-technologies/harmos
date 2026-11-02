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
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports

// Local Imports
use super::{Attachment, Context as PluginContext, Error, Plugin};
use crate::Setup;
use crate::operations::{Contract, ExecutionContext, ServingError, StartupError, WireTable};
use harmos_domain::model::operations::lifecycle;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Table = Arc<Mutex<WireTable>>;
type Transition<P> = Pin<Box<dyn Future<Output = Completed<P>> + Send>>;

/* === Structs === */

pub struct Owner<P: Plugin> {
    slot: Arc<Mutex<Slot<P>>>,
    table: Weak<Mutex<WireTable>>,
    contracts: Arc<[Contract]>,
    observers: Arc<[crate::inventory::observers::Declaration]>,
    retirement: Mutex<Retirement<P>>,
}

struct LifecycleInvocation<P: Plugin> {
    owner: Arc<Owner<P>>,
    state: Option<State<P>>,
    action: lifecycle::Action,
}

struct Retirement<P: Plugin> {
    state: Option<State<P>>,
    active: bool,
    finished: bool,
}

struct RetirementLoan<P: Plugin> {
    owner: Arc<Owner<P>>,
    state: Option<State<P>>,
    finished: bool,
}

struct Slot<P: Plugin> {
    plugin: Option<P>,
    context: Option<P::Context>,
    phase: Phase,
    attachment: Option<Arc<dyn Attachment<P::Context>>>,
    generation_context: Option<[u8; 32]>,
}

struct State<P: Plugin> {
    slot: Arc<Mutex<Slot<P>>>,
    table: Weak<Mutex<WireTable>>,
    plugin: Option<P>,
    config: Option<P::Config>,
    configuration: Option<WireTable>,
    work: Option<Transition<P>>,
    result: Option<Result<(), Error<P>>>,
    action: lifecycle::Action,
    delivered: bool,
    failure_reported: bool,
    shutdown_attempted: bool,
    cleanup_finished: bool,
    context: Option<P::Context>,
    assignments: Vec<u8>,
    attachment: Option<Arc<dyn Attachment<P::Context>>>,
    generation_context: Option<[u8; 32]>,
}

struct Completed<P: Plugin> {
    plugin: Option<P>,
    context: Option<P::Context>,
    configuration: Option<WireTable>,
    attachment: Option<Arc<dyn Attachment<P::Context>>>,
    result: Result<(), Error<P>>,
    shutdown_attempted: bool,
    cleanup_finished: bool,
}

/* === Enums === */

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Empty,
    Initializing,
    Initialized,
    Stopping,
    Closed,
}

/* === Traits === */

/* === Implementations === */

impl<P: Plugin> Owner<P> {
    pub fn new(owner: &str, contracts: Vec<Contract>, capacity: NonZeroUsize) -> Result<(Arc<Self>, Table), Error<P>> {
        Self::new_with_observers(owner, contracts, capacity, Vec::new())
    }

    pub fn new_with_observers(
        assigned_owner: &str, contracts: Vec<Contract>, capacity: NonZeroUsize, observers: Vec<crate::inventory::observers::Declaration>,
    ) -> Result<(Arc<Self>, Table), Error<P>> {
        let definitions: Vec<_> = contracts.iter().map(|contract| contract.operation().clone()).collect();
        crate::inventory::operations::validate(&definitions).map_err(|error| Error::Inventory(Arc::new(error)))?;
        let mut serving = WireTable::new(assigned_owner).map_err(|error| Error::Serving(Arc::new(error)))?;
        serving.capacity(capacity);
        serving.provide(std::any::TypeId::of::<P::Context>());
        serving.provide(std::any::TypeId::of::<crate::operations::Calls>());
        let table = Arc::new(Mutex::new(serving));
        let owner = Arc::new(Self {
            slot: Arc::new(Mutex::new(Slot {
                plugin: None,
                context: None,
                phase: Phase::Empty,
                attachment: None,
                generation_context: None,
            })),
            table: Arc::downgrade(&table),
            contracts: contracts.into(),
            observers: observers.into(),
            retirement: Mutex::new(Retirement {
                state: None,
                active: false,
                finished: false,
            }),
        });
        Ok((owner, table))
    }

    pub async fn quiesce(&self) -> Result<(), Error<P>> {
        let attachment = self.slot.lock().map_err(|_| Error::Poisoned)?.attachment.clone();
        if let Some(attachment) = attachment {
            guarded(async { attachment.quiesce().await.map_err(|error| Error::Serving(Arc::new(error))) }).await?;
        }
        Ok(())
    }

    pub async fn retire(self: &Arc<Self>) -> Result<(), Error<P>> {
        let table = self.table.upgrade().ok_or(Error::Phase)?;
        {
            let table = table.lock().map_err(|_| Error::Poisoned)?;
            if !table.is_settled() {
                return Err(Error::Phase);
            }
        }
        let mut loan = {
            let mut retirement = self.retirement.lock().map_err(|_| Error::Poisoned)?;
            if retirement.finished {
                return Ok(());
            }
            if retirement.active {
                return Err(Error::Phase);
            }
            let state = match retirement.state.take() {
                Some(state) => state,
                None => self.prepare_shutdown()?,
            };
            retirement.active = true;
            RetirementLoan {
                owner: Arc::clone(self),
                state: Some(state),
                finished: false,
            }
        };
        loan.state.as_mut().ok_or(Error::Phase)?.finalize().await?;
        loan.finished = true;
        Ok(())
    }

    fn state(&self, plugin: Option<P>, action: lifecycle::Action) -> State<P> {
        State {
            slot: Arc::clone(&self.slot),
            table: self.table.clone(),
            plugin,
            config: None,
            configuration: None,
            work: None,
            result: None,
            action,
            delivered: false,
            failure_reported: false,
            shutdown_attempted: false,
            cleanup_finished: false,
            context: None,
            assignments: Vec::new(),
            attachment: None,
            generation_context: None,
        }
    }
}

impl<P: Plugin> Drop for RetirementLoan<P> {
    fn drop(&mut self) {
        let mut retirement = self.owner.retirement.lock().unwrap_or_else(|error| error.into_inner());
        retirement.active = false;
        retirement.finished = self.finished;
        if !self.finished {
            retirement.state = self.state.take();
        }
    }
}

impl<P: Plugin> Owner<P> {
    fn prepare_initialize(&self, config_json: &[u8], generation_context: &[u8], assignments: &[u8]) -> Result<State<P>, Error<P>> {
        let generation_context = if generation_context.is_empty() {
            None
        } else {
            let token: [u8; 32] = generation_context.try_into().map_err(|_| Error::GenerationContext)?;
            if token == [0; 32] {
                return Err(Error::GenerationContext);
            }
            Some(token)
        };
        let config = crate::decode_config(config_json).map_err(|error| Error::Configuration(Arc::new(error)))?;
        let mut slot = self.slot.lock().map_err(|_| Error::Poisoned)?;
        if slot.phase != Phase::Empty {
            return Err(Error::Phase);
        }
        slot.phase = Phase::Initializing;
        let mut state = self.state(None, lifecycle::Action::Initialize);
        state.config = Some(config);
        state.generation_context = generation_context;
        state.assignments = assignments.to_vec();
        Ok(state)
    }

    async fn initialize(&self, state: &mut State<P>, context: &P::Context) -> Result<(), Error<P>> {
        if state.work.is_none() && state.result.is_none() {
            context.assign_operations(&state.assignments).map_err(|error| Error::Serving(Arc::new(error)))?;
            let config = state.config.take().ok_or(Error::Phase)?;
            let configuration = {
                let table = self.table.upgrade().ok_or(Error::Phase)?;
                let table = table.lock().map_err(|_| Error::Poisoned)?;
                table.configuration().map_err(|error| Error::Serving(Arc::new(error)))?
            };
            let context = state.generation_context.map_or_else(|| context.clone(), |token| context.with_generation(token));
            state.work = Some(Box::pin(prepare::<P>(
                config,
                configuration,
                context,
                self.contracts.clone(),
                self.observers.clone(),
                state.assignments.clone(),
            )));
        }
        state.finish().await?;
        state.result.as_ref().ok_or(Error::Phase)?.clone()?;
        if state.delivered {
            return Ok(());
        }
        let attachment = state.attachment.clone();
        let table = self.table.upgrade().ok_or(Error::Phase)?;
        let binding = {
            let mut table = table.lock().map_err(|_| Error::Poisoned)?;
            let configuration = state.configuration.take().ok_or(Error::Phase)?;
            table.install_configuration(configuration).map_err(|error| Error::Serving(Arc::new(error)))
        };
        if let Err(error) = binding {
            state.result = Some(Err(error.clone()));
            return Err(error);
        }
        let mut slot = self.slot.lock().map_err(|_| Error::Poisoned)?;
        slot.plugin = state.plugin.take();
        slot.context = state.context.take();
        slot.attachment = attachment;
        slot.generation_context = state.generation_context;
        slot.phase = Phase::Initialized;
        state.delivered = true;
        Ok(())
    }

    fn prepare_shutdown(&self) -> Result<State<P>, Error<P>> {
        let mut slot = self.slot.lock().map_err(|_| Error::Poisoned)?;
        if matches!(slot.phase, Phase::Empty | Phase::Closed) {
            let mut state = self.state(None, lifecycle::Action::Shutdown);
            state.result = Some(Ok(()));
            return Ok(state);
        }
        if slot.phase != Phase::Initialized {
            return Err(Error::Phase);
        }
        let plugin = slot.plugin.take().ok_or(Error::Phase)?;
        slot.phase = Phase::Stopping;
        let mut state = self.state(Some(plugin), lifecycle::Action::Shutdown);
        state.attachment = slot.attachment.take();
        state.context = slot.context.take();
        state.generation_context = slot.generation_context;
        Ok(state)
    }
}

impl<P: Plugin> crate::operations::lifecycle::PluginLifecycle for Owner<P> {
    fn begin(self: Arc<Self>, action: lifecycle::Action) -> Box<dyn crate::operations::lifecycle::LifecycleInstance> {
        Box::new(LifecycleInvocation { owner: self, state: None, action })
    }
}
impl<P: Plugin> crate::operations::lifecycle::LifecycleInstance for LifecycleInvocation<P> {
    fn run<'a>(
        &'a mut self, context: ExecutionContext, config: &'a [u8], generation: &'a [u8], assignments: &'a [u8],
    ) -> crate::operations::lifecycle::Transition<'a> {
        Box::pin(async move {
            let result: Result<(), Error<P>> = async {
                match self.action {
                    lifecycle::Action::Initialize => {
                        if self.state.is_none() {
                            self.state = Some(self.owner.prepare_initialize(config, generation, assignments)?);
                        }
                        let carrier = context.resource::<P::Context>().map_err(|error| Error::Resource(Arc::new(error)))?;
                        self.owner.initialize(self.state.as_mut().ok_or(Error::Phase)?, &carrier).await
                    }
                    lifecycle::Action::Shutdown => {
                        if self.state.is_none() {
                            self.state = Some(self.owner.prepare_shutdown()?);
                        }
                        let state = self.state.as_mut().ok_or(Error::Phase)?;
                        if state.work.is_none() && state.result.is_none() {
                            let carrier = context.resource::<P::Context>().map_err(|error| Error::Resource(Arc::new(error)))?;
                            state.context =
                                Some(state.generation_context.map_or_else(|| (*carrier).clone(), |token| carrier.with_generation(token)));
                            state.start_shutdown()?;
                        }
                        state.finish().await?;
                        state.result.as_ref().ok_or(Error::Phase)?.clone()
                    }
                }
            }
            .await;
            result.map_err(Into::into)
        })
    }
    fn cleanup(&mut self) -> crate::operations::lifecycle::Transition<'_> {
        Box::pin(async {
            match self.state.as_mut() {
                Some(state) => state.finalize().await.map_err(Into::into),
                None => Ok(()),
            }
        })
    }
}

impl<P: Plugin> State<P> {
    async fn finish(&mut self) -> Result<(), Error<P>> {
        if let Some(work) = &mut self.work {
            let completed = work.await;
            self.shutdown_attempted = completed.shutdown_attempted;
            self.cleanup_finished = completed.cleanup_finished;
            self.plugin = completed.plugin;
            if completed.context.is_some() {
                self.context = completed.context;
            }
            self.result = Some(completed.result);
            if completed.configuration.is_some() {
                self.configuration = completed.configuration;
            }
            if completed.attachment.is_some() {
                self.attachment = completed.attachment;
            }
            self.work = None;
        }
        Ok(())
    }

    fn start_shutdown(&mut self) -> Result<(), Error<P>> {
        let attachment = self.attachment.clone().or_else(|| self.plugin.as_ref().and_then(Plugin::attachment));
        let mut plugin = self.plugin.take().ok_or(Error::Phase)?;
        let table = self.table.clone();
        let context = self.context.clone();
        let mut shutdown_attempted = self.shutdown_attempted;
        let mut cleanup_finished = self.cleanup_finished;
        self.result = None;
        self.work = Some(Box::pin(async move {
            let result = guarded(async {
                if let Some(attachment) = attachment {
                    attachment.quiesce().await.map_err(|error| Error::Serving(Arc::new(error)))?;
                }
                let table = table.upgrade().ok_or(Error::Phase)?;
                let settlement = {
                    let mut table = table.lock().map_err(|_| Error::Poisoned)?;
                    table.close_ordinary()
                };
                settlement.await.map_err(|error| Error::Serving(Arc::new(error)))?;
                let retired = {
                    let mut table = table.lock().map_err(|_| Error::Poisoned)?;
                    table.release_ordinary().map_err(|error| Error::Serving(Arc::new(error)))?
                };
                drop(retired);
                let mut errors = Vec::new();
                if !shutdown_attempted && let Some(context) = &context {
                    // Domain Effects Are Never Replayed By Physical Cleanup Retries.
                    shutdown_attempted = true;
                    if let Err(error) = guarded(async { plugin.stop(context).await.map_err(|error| Error::Plugin(Arc::new(error))) }).await
                    {
                        errors.push(error);
                    }
                }
                if !cleanup_finished {
                    match guarded(async { plugin.cleanup().await.map_err(|error| Error::Plugin(Arc::new(error))) }).await {
                        Ok(()) => cleanup_finished = true,
                        Err(error) => errors.push(error),
                    }
                }
                combine(errors)
            })
            .await;
            Completed {
                plugin: Some(plugin),
                context: None,
                configuration: None,
                attachment: None,
                result,
                shutdown_attempted,
                cleanup_finished,
            }
        }));
        Ok(())
    }

    async fn finalize(&mut self) -> Result<(), Error<P>> {
        if self.delivered {
            return Ok(());
        }
        if self.action == lifecycle::Action::Initialize {
            self.finish().await?;
            self.action = lifecycle::Action::Shutdown;
            self.result = None;
        }
        if self.plugin.is_none() {
            self.configuration.take();
            self.slot.lock().map_err(|_| Error::Poisoned)?.phase = Phase::Closed;
            return Ok(());
        }
        if let Some(Err(error)) = &self.result {
            if !self.failure_reported {
                self.failure_reported = true;
                return Err(error.clone());
            }
            self.failure_reported = false;
            self.start_shutdown()?;
        } else if self.work.is_none() && self.result.is_none() {
            self.start_shutdown()?;
        }
        self.finish().await?;
        if let Some(Err(error)) = &self.result {
            self.failure_reported = true;
            return Err(error.clone());
        }
        self.plugin.take();
        self.slot.lock().map_err(|_| Error::Poisoned)?.phase = Phase::Closed;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn prepare<P: Plugin>(
    config: P::Config, configuration: WireTable, context: P::Context, contracts: Arc<[Contract]>,
    observers: Arc<[crate::inventory::observers::Declaration]>, assignments: Vec<u8>,
) -> Completed<P> {
    let mut setup = Setup::<P>::new(configuration);
    for catalog in P::CATALOGS {
        setup.settings().catalog(catalog);
    }
    let prepared = guarded::<P, _>(async { P::prepare(config, &mut setup).await.map_err(|error| Error::Plugin(Arc::new(error))) }).await;
    let mut errors = Vec::new();
    let mut plugin = match prepared {
        Ok(plugin) => Some(plugin),
        Err(source) => {
            errors.push(StartupError::Runtime { source: Box::new(source) });
            None
        }
    };

    let mut attachment = None;
    let binding = guarded::<P, _>(async {
        if let Some(plugin) = &plugin {
            attachment = plugin.attachment();
            let actual = attachment.as_ref().map_or_else(Vec::new, |attachment| attachment.observers());
            if actual.as_slice() != observers.as_ref() {
                return Err(Error::Serving(Arc::new(ServingError::Catalog)));
            }
            if let Some(attachment) = &attachment {
                attachment.clone().bind(setup.settings()).map_err(|error| Error::Serving(Arc::new(error)))?;
            }
        }
        Ok(())
    })
    .await;
    if let Err(source) = binding {
        errors.push(StartupError::Runtime { source: Box::new(source) });
    }
    if let Err(error) = setup.validate() {
        match error {
            StartupError::Multiple { errors: configuration } => errors.extend(configuration),
            error => errors.push(error),
        }
    }
    let configuration = setup.into_configuration();
    if errors.is_empty() {
        if let Err(source) = configuration.validate_contracts(&contracts) {
            errors.push(StartupError::Binding { source: Box::new(source) });
        }
        if let Err(source) = configuration.validate_callees(&assignments) {
            errors.push(StartupError::Binding { source: Box::new(source) });
        }
    }
    let result = match StartupError::aggregate(errors) {
        Some(error) => Err(Error::Serving(Arc::new(ServingError::Startup(Box::new(error))))),
        None => {
            let plugin = plugin.as_mut().expect("successful preparation retains the plugin owner");
            guarded(async { plugin.initialize(&context).await.map_err(|error| Error::Plugin(Arc::new(error))) }).await
        }
    };
    let context = result.is_ok().then_some(context);
    Completed {
        plugin,
        context,
        configuration: Some(configuration),
        attachment,
        result,
        shutdown_attempted: false,
        cleanup_finished: false,
    }
}

fn combine<P: Plugin>(mut errors: Vec<Error<P>>) -> Result<(), Error<P>> {
    if errors.len() == 1 {
        return Err(errors.pop().expect("one lifecycle error"));
    }
    let errors = errors.into_iter().map(|source| StartupError::Runtime { source: Box::new(source) }).collect();
    match StartupError::aggregate(errors) {
        None => Ok(()),
        Some(error) => Err(Error::Serving(Arc::new(ServingError::Startup(Box::new(error))))),
    }
}

// Guard Only The Borrowed Machinery Future; The Outer Future Retains P
async fn guarded<P: Plugin, T>(future: impl Future<Output = Result<T, Error<P>>>) -> Result<T, Error<P>> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result,
        Err(_) => std::task::Poll::Ready(Err(Error::Panicked)),
    })
    .await
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;
    use crate::operations::{OperationCatalog, RegistrationBatch, WireDeclaration};

    struct Events(Arc<Mutex<Vec<&'static str>>>);
    struct ServedCatalog;

    static SERVED: [crate::catalog::Node<WireDeclaration>; 1] = [crate::catalog::Node::Leaf(WireDeclaration::of::<Serve>())];

    impl OperationCatalog for ServedCatalog {
        const CATALOG: crate::operations::Catalog = crate::operations::Catalog::of::<Self>("ServedCatalog", &SERVED, &[]);

        fn batch() -> RegistrationBatch {
            RegistrationBatch::new().operation::<Serve>()
        }
    }

    #[crate::operation(harmos_path = crate, resources = [Events, u32], id = "serve", response = u32,
        policy = <TestPlugin as lifecycle::Policy>::INITIALIZATION_POLICY)]
    struct Serve;

    impl crate::Operation for Serve {
        type Error = std::convert::Infallible;

        async fn run(self, context: crate::Context<'_, Self>) -> Result<crate::Response<u32>, Self::Error> {
            context.resource::<Events>().0.lock().unwrap().push("serve");
            Ok(crate::Response::from_owner(context.resource::<u32>()))
        }
    }

    fn contracts() -> Vec<Contract> {
        vec![WireDeclaration::of::<Serve>().contract().unwrap()]
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(default)]
    struct Config {
        #[serde(skip)]
        events: Arc<Mutex<Vec<&'static str>>>,

        value: u32,
        duplicate: bool,
        fail_prepare: bool,
        fail_stop: bool,
        fail_cleanup: bool,
    }

    struct TestPlugin(Config);

    impl lifecycle::Policy for TestPlugin {
        const INITIALIZATION_POLICY: crate::OperationPolicy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: crate::RetryPolicy::Never,
        };
        const SHUTDOWN_POLICY: crate::OperationPolicy = Self::INITIALIZATION_POLICY;
    }

    impl crate::Lifecycle for TestPlugin {
        type Config = Config;
        type Error = std::io::Error;
        type Context = ();

        async fn prepare(config: Config, setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
            config.events.lock().unwrap().push("prepare");
            setup.resource(Events(config.events.clone()));
            setup.resource(config.value);
            if config.duplicate {
                setup.resource(config.value + 1);
            }
            if config.fail_prepare {
                return Err(std::io::Error::other("prepare failure"));
            }
            Ok(Self(config))
        }

        async fn initialize(&mut self, _: &()) -> Result<(), Self::Error> {
            self.0.events.lock().unwrap().push("initialize");
            Ok(())
        }

        async fn stop(&mut self, _: &()) -> Result<(), Self::Error> {
            self.0.events.lock().unwrap().push("stop");
            if self.0.fail_stop {
                return Err(std::io::Error::other("stop failure"));
            }
            Ok(())
        }

        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            self.0.events.lock().unwrap().push("cleanup");
            if self.0.fail_cleanup {
                return Err(std::io::Error::other("cleanup failure"));
            }
            Ok(())
        }
    }

    impl Plugin for TestPlugin {
        const CATALOGS: &'static [crate::operations::Catalog] = &[ServedCatalog::CATALOG];
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the lifecycle fixture completes synchronously"),
        }
    }

    fn prepared(config: Config) -> Completed<TestPlugin> {
        ready(prepare::<TestPlugin>(config, WireTable::new("fixture").unwrap(), (), contracts().into(), Arc::from([]), Vec::new()))
    }

    #[test]
    fn validates_before_initialize() {
        let events = Arc::default();
        let valid = prepared(Config {
            events: Arc::clone(&events),
            ..Default::default()
        });
        assert!(valid.result.is_ok());
        assert_eq!(*events.lock().unwrap(), ["prepare", "initialize"]);

        events.lock().unwrap().clear();
        let invalid = prepared(Config {
            events: Arc::clone(&events),
            duplicate: true,
            ..Default::default()
        });
        assert!(invalid.result.is_err());
        assert!(invalid.plugin.is_some());
        assert_eq!(*events.lock().unwrap(), ["prepare"]);
    }

    #[test]
    fn retained_owner_lifecycle() {
        let (owner, table) = Owner::<TestPlugin>::new("fixture", contracts(), NonZeroUsize::MIN).unwrap();
        let mut state = owner.prepare_initialize(b"{}", &[], &[]).unwrap();
        ready(owner.initialize(&mut state, &())).unwrap();
        let events = {
            let slot = owner.slot.lock().unwrap();
            assert!(slot.context.is_some());
            slot.plugin.as_ref().unwrap().0.events.clone()
        };
        assert_eq!(*events.lock().unwrap(), ["prepare", "initialize"]);

        {
            let mut table = table.lock().unwrap();
            let context = table.context();
            table.prepare(1, "fixture.serve", Serve::definition().fingerprint.0, 1, Serve.encode()).unwrap();
            ready(table.bind(1, &context)).unwrap();
            assert_eq!(ready(table.attempt(1, &context)).unwrap(), 0u32.encode());
            ready(table.finalize(1)).unwrap();
            ready(table.settle()).unwrap();
        }
        ready(owner.retire()).unwrap();
        assert_eq!(*events.lock().unwrap(), ["prepare", "initialize", "serve", "stop", "cleanup"]);
    }

    #[test]
    fn failed_prepare_aggregates() {
        let completed = prepared(Config {
            fail_prepare: true,
            duplicate: true,
            ..Default::default()
        });
        let Err(Error::Serving(error)) = completed.result else {
            panic!("preparation and setup failures must be reported");
        };
        let ServingError::Startup(error) = error.as_ref() else {
            panic!("setup failures retain their typed source");
        };
        let StartupError::Multiple { errors } = error.as_ref() else {
            panic!("both independent failures must be retained");
        };
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|error| matches!(error, StartupError::DuplicateResource { .. })));
        assert!(completed.plugin.is_none());
    }

    #[test]
    fn setup_replaces_resource() {
        let mut setup = Setup::<TestPlugin>::new(WireTable::new("fixture").unwrap());
        let _plugin = ready(<TestPlugin as crate::Lifecycle>::prepare(Config { value: 1, ..Default::default() }, &mut setup)).unwrap();
        let replacement = Arc::new(2u32);
        setup.overrides(|setup| {
            setup.resource::<u32>(replacement.clone());
        });
        setup.validate().unwrap();
        let actual = setup.settings().context().resource::<u32>().unwrap();
        assert!(Arc::ptr_eq(&replacement, &actual));
    }

    #[test]
    fn stop_failure_cleanup() {
        let events = Arc::default();
        let plugin = TestPlugin(Config {
            events: Arc::clone(&events),
            fail_stop: true,
            ..Default::default()
        });
        let (owner, _table) = Owner::new("fixture", Vec::new(), NonZeroUsize::MIN).unwrap();
        let mut state = owner.state(Some(plugin), lifecycle::Action::Shutdown);
        state.context = Some(());
        state.start_shutdown().unwrap();
        ready(state.finish()).unwrap();
        assert!(state.result.as_ref().unwrap().is_err());
        assert_eq!(*events.lock().unwrap(), ["stop", "cleanup"]);

        assert!(ready(state.finalize()).is_err());
        ready(state.finalize()).unwrap();
        assert_eq!(*events.lock().unwrap(), ["stop", "cleanup"]);
    }

    #[test]
    fn retirement_reports_failures() {
        let plugin = TestPlugin(Config {
            fail_stop: true,
            fail_cleanup: true,
            ..Default::default()
        });
        let (owner, _table) = Owner::new("fixture", Vec::new(), NonZeroUsize::MIN).unwrap();
        let mut state = owner.state(Some(plugin), lifecycle::Action::Shutdown);
        state.context = Some(());
        state.start_shutdown().unwrap();
        ready(state.finish()).unwrap();

        let Err(Error::Serving(error)) = state.result.take().unwrap() else {
            panic!("both lifecycle failures must remain available");
        };
        let ServingError::Startup(error) = error.as_ref() else {
            panic!("lifecycle errors retain an aggregate source");
        };
        let StartupError::Multiple { errors } = error.as_ref() else {
            panic!("both lifecycle failures must remain available");
        };
        assert_eq!(errors.len(), 2);
    }
}
