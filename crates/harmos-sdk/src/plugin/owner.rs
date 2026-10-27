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
use crate::operations::{Contract, ExecutionContext, ServingError, WireTable};
use harmos_domain::model::operations::lifecycle;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Table = Arc<Mutex<WireTable>>;
type Transition<P> = Pin<Box<dyn Future<Output = (P, Result<(), Error<P>>, bool)> + Send>>;

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
    phase: Phase,
    attachment: Option<Arc<dyn Attachment<P::Context>>>,
    generation_context: Option<[u8; 32]>,
}

struct State<P: Plugin> {
    slot: Arc<Mutex<Slot<P>>>,
    table: Weak<Mutex<WireTable>>,
    plugin: Option<P>,
    work: Option<Transition<P>>,
    result: Option<Result<(), Error<P>>>,
    action: lifecycle::Action,
    delivered: bool,
    failure_reported: bool,
    shutdown_attempted: bool,
    context: Option<P::Context>,
    assignments: Vec<u8>,
    attachment: Option<Arc<dyn Attachment<P::Context>>>,
    generation_context: Option<[u8; 32]>,
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
            work: None,
            result: None,
            action,
            delivered: false,
            failure_reported: false,
            shutdown_attempted: false,
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
        let plugin = P::prepare(config).map_err(|error| Error::Plugin(Arc::new(error)))?;
        let mut slot = self.slot.lock().map_err(|_| Error::Poisoned)?;
        if slot.phase != Phase::Empty {
            return Err(Error::Phase);
        }
        slot.phase = Phase::Initializing;
        let mut state = self.state(Some(plugin), lifecycle::Action::Initialize);
        state.generation_context = generation_context;
        state.assignments = assignments.to_vec();
        Ok(state)
    }

    async fn initialize(&self, state: &mut State<P>, context: &P::Context) -> Result<(), Error<P>> {
        if state.work.is_none() && state.result.is_none() {
            context.assign_operations(&state.assignments).map_err(|error| Error::Serving(Arc::new(error)))?;
            let mut plugin = state.plugin.take().ok_or(Error::Phase)?;
            let context = state.generation_context.map_or_else(|| context.clone(), |token| context.with_generation(token));
            state.work = Some(Box::pin(async move {
                let result = guarded(async { plugin.initialize(&context).await.map_err(|error| Error::Plugin(Arc::new(error))) }).await;
                (plugin, result, false)
            }));
        }
        state.finish().await?;
        state.result.as_ref().ok_or(Error::Phase)?.clone()?;
        if state.delivered {
            return Ok(());
        }
        let inventory = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let plugin = state.plugin.as_ref().ok_or(Error::Phase)?;
            state.attachment = plugin.attachment();
            if let Some(attachment) = &state.attachment {
                let resources: Arc<dyn std::any::Any + Send + Sync> = plugin.resources();
                if !Arc::ptr_eq(&resources, &attachment.resources()) {
                    return Err(Error::Serving(Arc::new(ServingError::Catalog)));
                }
            }
            let actual = state.attachment.as_ref().map_or_else(Vec::new, |attachment| attachment.observers());
            if actual.as_slice() != self.observers.as_ref() {
                return Err(Error::Serving(Arc::new(ServingError::Catalog)));
            }
            Ok(())
        }))
        .map_err(|_| Error::Panicked)
        .and_then(|result| result);
        if let Err(error) = inventory {
            state.result = Some(Err(error.clone()));
            return Err(error);
        }
        let attachment = state.attachment.clone();
        let table = self.table.upgrade().ok_or(Error::Phase)?;
        let binding = {
            let mut table = table.lock().map_err(|_| Error::Poisoned)?;
            let plugin = state.plugin.as_ref().ok_or(Error::Phase)?;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                plugin
                    .bind_operations(&mut table)
                    .and_then(|()| table.validate_contracts(&self.contracts))
                    .and_then(|()| table.validate_callees(&state.assignments))
            }))
            .map_err(|_| Error::Panicked)
            .and_then(|result| result.map_err(|error| Error::Serving(Arc::new(error))))
        };
        if let Err(error) = binding {
            state.result = Some(Err(error.clone()));
            return Err(error);
        }
        let mut slot = self.slot.lock().map_err(|_| Error::Poisoned)?;
        slot.plugin = state.plugin.take();
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
            let (plugin, result, shutdown_attempted) = work.await;
            self.shutdown_attempted = shutdown_attempted;
            self.plugin = Some(plugin);
            self.result = Some(result);
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
                if !shutdown_attempted && let Some(context) = &context {
                    // Domain Effects Are Never Replayed By Physical Cleanup Retries.
                    shutdown_attempted = true;
                    plugin.stop(context).await.map_err(|error| Error::Plugin(Arc::new(error)))?;
                }
                plugin.cleanup().await.map_err(|error| Error::Plugin(Arc::new(error)))
            })
            .await;
            (plugin, result, shutdown_attempted)
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

// Guard Only The Borrowed Machinery Future; The Outer Future Retains P
async fn guarded<P: Plugin>(future: impl Future<Output = Result<(), Error<P>>>) -> Result<(), Error<P>> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result,
        Err(_) => std::task::Poll::Ready(Err(Error::Panicked)),
    })
    .await
}

/* === Tests === */
