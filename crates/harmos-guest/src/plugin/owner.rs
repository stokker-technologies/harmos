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
use super::{Context as PluginContext, Error, Plugin};
use crate::dispatch::Table as WireTable;
use harmos_domain::model::operations::lifecycle;
use harmos_operations::execution::Setup;
use harmos_operations::execution::operations::{Contract, ExecutionContext, ServingError, StartupError};

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
}

struct LifecycleInvocation<P: Plugin> {
    owner: Arc<Owner<P>>,
    state: Option<State<P>>,
    action: lifecycle::Action,
}

struct Slot<P: Plugin> {
    plugin: Option<P>,
    context: Option<P::Context>,
    phase: Phase,
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
    generation_context: Option<[u8; 32]>,
}

struct Completed<P: Plugin> {
    plugin: Option<P>,
    context: Option<P::Context>,
    configuration: Option<WireTable>,
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
        let assigned_owner = owner;
        let definitions: Vec<_> = contracts.iter().map(|contract| contract.operation().clone()).collect();
        harmos_operations::execution::inventory::operations::validate(&definitions).map_err(|error| Error::Inventory(Arc::new(error)))?;
        let mut serving = WireTable::new(assigned_owner).map_err(|error| Error::Serving(Arc::new(error)))?;
        serving.capacity(capacity);
        serving.provide(std::any::TypeId::of::<P::Context>());
        serving.provide(std::any::TypeId::of::<harmos_operations::execution::operations::Calls>());
        let table = Arc::new(Mutex::new(serving));
        let owner = Arc::new(Self {
            slot: Arc::new(Mutex::new(Slot {
                plugin: None,
                context: None,
                phase: Phase::Empty,
                generation_context: None,
            })),
            table: Arc::downgrade(&table),
            contracts: contracts.into(),
        });
        Ok((owner, table))
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
            generation_context: None,
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
        let config = harmos_operations::execution::decode_config(config_json).map_err(|error| Error::Configuration(Arc::new(error)))?;
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
            state.work = Some(Box::pin(prepare::<P>(config, configuration, context, self.contracts.clone(), state.assignments.clone())));
        }
        state.finish().await?;
        state.result.as_ref().ok_or(Error::Phase)?.clone()?;
        if state.delivered {
            return Ok(());
        }
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
        state.context = slot.context.take();
        state.generation_context = slot.generation_context;
        Ok(state)
    }
}

impl<P: Plugin> crate::lifecycle::access::PluginLifecycle for Owner<P> {
    fn begin(self: Arc<Self>, action: lifecycle::Action) -> Box<dyn crate::lifecycle::access::LifecycleInstance> {
        Box::new(LifecycleInvocation { owner: self, state: None, action })
    }
}
impl<P: Plugin> crate::lifecycle::access::LifecycleInstance for LifecycleInvocation<P> {
    fn run<'a>(
        &'a mut self, context: ExecutionContext, config: &'a [u8], generation: &'a [u8], assignments: &'a [u8],
    ) -> crate::lifecycle::access::Transition<'a> {
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
    fn cleanup(&mut self) -> crate::lifecycle::access::Transition<'_> {
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
            self.work = None;
        }
        Ok(())
    }

    fn start_shutdown(&mut self) -> Result<(), Error<P>> {
        let mut plugin = self.plugin.take().ok_or(Error::Phase)?;
        let table = self.table.clone();
        let context = self.context.clone();
        let mut shutdown_attempted = self.shutdown_attempted;
        let mut cleanup_finished = self.cleanup_finished;
        self.result = None;
        self.work = Some(Box::pin(async move {
            let result = guarded(async {
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
    config: P::Config, configuration: WireTable, context: P::Context, contracts: Arc<[Contract]>, assignments: Vec<u8>,
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
