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
use std::sync::{Arc, Mutex, OnceLock, Weak};
// Third Party Imports
use tokio::sync::{mpsc, watch};
// Local Imports
use super::{Arrival, SidecarError};
use crate::observers::{
    ObserverActivation, ObserverBackend, ObserverDefinition, ObserverManager, ObserverScope,
};
use harmos_sidecar::inventory::observers::{Activation, Declaration};
use harmos_sidecar::{ObserverControl, ObserverExit, ObserverStatus, ObserverUpdate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type States = BTreeMap<String, State>;

/* === Structs === */
struct State {
    generation: u64,
    status: watch::Sender<ObserverStatus>,
    context: [u8; 32],
    children: Arc<crate::observers::ChildSettlement>,
}

struct ControlContext {
    operations: crate::operations::OperationManager,
    scope: crate::load::PluginScope,
    caller: crate::operations::Caller,
    bindings: OnceLock<[crate::operations::PluginControl; 2]>,
}

pub(super) struct Remote {
    pub(super) provider: Option<Arc<crate::supervision::instance::Instance>>,
    arrivals: mpsc::Sender<Arrival>,
    states: Mutex<States>,
    control: OnceLock<ControlContext>,
    fenced: watch::Sender<bool>,
    retirement: OnceLock<Weak<Retirement>>,
    outcome: Arc<Mutex<Option<Arc<crate::Error>>>>,
}

struct GenerationContext {
    remote: Arc<Remote>,
    control: ObserverControl,
}

struct Retained {
    context: GenerationContext,
    status: watch::Receiver<ObserverStatus>,
    children: Weak<crate::observers::ChildSettlement>,
}

struct Backend {
    definition: ObserverDefinition,
}

pub(super) struct Session {
    closing: Option<Closing>,
    outcome: Option<Arc<crate::Error>>,
}

struct Retirement {
    manager: ObserverManager,
    scope: ObserverScope<Remote>,
}

struct Closing {
    remote: Arc<Remote>,
    manager: ObserverManager,
    scope: Option<Arc<Retirement>>,
    end: Option<harmos_sidecar::attach::Ending>,
    ending_failure: Option<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Remote {
    pub(super) fn new(
        arrivals: mpsc::Sender<Arrival>,
        provider: Option<Arc<crate::supervision::instance::Instance>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            provider,
            arrivals,
            states: Mutex::new(BTreeMap::new()),
            control: OnceLock::new(),
            fenced: watch::channel(false).0,
            retirement: OnceLock::new(),
            outcome: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn outcome(&self) -> Arc<Mutex<Option<Arc<crate::Error>>>> {
        Arc::clone(&self.outcome)
    }

    pub(super) async fn settle(&self) -> crate::Result<()> {
        let Some(original) = self.retirement.get().and_then(Weak::upgrade) else {
            return Ok(());
        };
        let retired = original.manager.retire_scope(&original.scope).await?;
        *self.outcome.lock().expect("observer outcome is healthy") = retired.into_outcome().err();
        Ok(())
    }

    pub(super) fn update(&self, update: ObserverUpdate) {
        let states = self
            .states
            .lock()
            .expect("remote observer states are healthy");
        if let Some(state) = states.get(&update.control.observer)
            && state.generation == update.control.generation
        {
            state.status.send_replace(update.status);
        }
    }

    pub(super) fn configure(
        &self,
        operations: crate::operations::OperationManager,
        scope: crate::load::PluginScope,
        caller: crate::operations::Caller,
    ) -> crate::Result<()> {
        self.control
            .set(ControlContext {
                operations,
                scope,
                caller,
                bindings: OnceLock::new(),
            })
            .map_err(|_| crate::Error::InvalidInput {
                field: "observer controls",
                reason: "original transport was already configured",
            })
    }

    pub(super) fn capture_controls(&self) -> crate::Result<()> {
        let original = self.control.get().ok_or(crate::Error::NotReady {
            domain: "observer controls",
        })?;
        let Some(pair) = original.scope.observer_controls() else {
            return Ok(());
        };
        if original.bindings.get().is_some() {
            return Ok(());
        }
        let bindings = [
            original
                .operations
                .capture_host_control(&original.scope, &pair[0])?,
            original
                .operations
                .capture_host_control(&original.scope, &pair[1])?,
        ];
        let _ = original.bindings.set(bindings);
        Ok(())
    }

    pub(super) fn owns_context(&self, context: &[u8; 32]) -> bool {
        self.states
            .lock()
            .expect("remote observer states are healthy")
            .values()
            .any(|state| &state.context == context)
    }

    pub(super) fn submit_child(
        &self,
        call: crate::operations::WireChildCall,
        submitted: tokio::time::Instant,
    ) -> crate::Result<crate::operations::EncodedInvocation> {
        let control = self.control.get().ok_or(crate::Error::NotReady {
            domain: "observer controls",
        })?;
        let states = self
            .states
            .lock()
            .expect("remote observer states are healthy");
        let state = states
            .values()
            .find(|state| state.context == call.context)
            .ok_or(crate::Error::Closing {
                domain: "observer generation context",
            })?;
        state.children.submit(|| {
            control
                .caller
                .submit_observer_encoded_at(&call.operation, call.payload, submitted)
        })
    }

    async fn call(&self, stopping: bool, control: &ObserverControl) -> crate::Result<Vec<u8>> {
        let original = self.control.get().ok_or(crate::Error::NotReady {
            domain: "observer controls",
        })?;
        original
            .scope
            .observer_controls()
            .ok_or(crate::Error::NotReady {
                domain: "observer controls",
            })?;
        self.capture_controls()?;
        let (context, children) = {
            let states = self
                .states
                .lock()
                .expect("remote observer states are healthy");
            let state = states
                .get(&control.observer)
                .filter(|state| state.generation == control.generation)
                .ok_or(crate::Error::Closing {
                    domain: "observer generation context",
                })?;
            (state.context, Arc::clone(&state.children))
        };
        let binding = &original
            .bindings
            .get()
            .expect("original captured control pair")[usize::from(stopping)];
        let bytes = control
            .observer
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(256))
            .and_then(std::num::NonZeroUsize::new)
            .ok_or(crate::Error::ResourceExhausted {
                resource: "observer control encoding",
            })?;
        // Retain The Original Control Before Any Cancellable Wait
        let invocation = children.submit_control(stopping, || {
            binding.submit_with(bytes, |_| {
                let request = harmos_contract::observers::Control {
                    observer: control.observer.clone(),
                    generation: control.generation,
                    context: context.to_vec(),
                };
                Ok(harmos_contract::Message::encode_to_vec(&request))
            })
        })?;
        let (_cancellation, cancelled) = tokio::sync::oneshot::channel();
        crate::operations::children::lifecycle(invocation, cancelled).await
    }

    fn revoke(&self, control: &ObserverControl) {
        let mut states = self
            .states
            .lock()
            .expect("remote observer states are healthy");
        if states
            .get(&control.observer)
            .is_some_and(|state| state.generation == control.generation)
        {
            states.remove(&control.observer);
        }
    }

    async fn fence(&self) {
        let _ = self.fenced.subscribe().wait_for(|fenced| *fenced).await;
    }

    async fn failed(&self, error: crate::Error) -> crate::Error {
        let _ = self
            .arrivals
            .send(Arrival::ObserverFailed(error.to_string()))
            .await;
        self.fence().await;
        error
    }
}

impl Drop for GenerationContext {
    fn drop(&mut self) {
        self.remote.revoke(&self.control);
    }
}

impl ObserverBackend for Backend {
    type Resources = Remote;
    fn definition(&self) -> ObserverDefinition {
        self.definition.clone()
    }

    async fn run(
        &self,
        remote: Arc<Remote>,
        mut activation: ObserverActivation,
    ) -> crate::Result<()> {
        if let Some(provider) = &remote.provider {
            activation.depend_on(provider)?;
        }
        let control = ObserverControl {
            observer: self.definition.id.clone(),
            generation: activation.id().value(),
        };
        let (status, mut changed) = watch::channel(ObserverStatus::Starting);
        let mut context = [0; 32];
        getrandom::fill(&mut context).map_err(|source| crate::Error::OperationFailed {
            operation: control.observer.clone(),
            source: Arc::new(source),
        })?;
        if context == [0; 32] {
            return Err(crate::Error::InvalidInput {
                field: "observer context",
                reason: "zero random capability",
            });
        }
        remote.states.lock().unwrap().insert(
            control.observer.clone(),
            State {
                generation: control.generation,
                status,
                context,
                children: activation.child_settlement(),
            },
        );
        let context = GenerationContext {
            remote: remote.clone(),
            control: control.clone(),
        };
        let started = tokio::select! {
            result = remote.call(false, &control) => Some(result),
            _ = activation.stopping() => None,
            _ = remote.fence() => return Err(failure(SidecarError::Unavailable)),
        };
        if let Some(Err(error)) = started {
            return Err(remote.failed(error).await);
        }
        if started.is_some() {
            loop {
                let current = *changed.borrow_and_update();
                match current {
                    ObserverStatus::Running => activation.ready(),
                    ObserverStatus::Stopping | ObserverStatus::Finalizing => {
                        activation.begin_stopping()
                    }
                    ObserverStatus::Stopped(_) | ObserverStatus::Retained(_) => break,
                    ObserverStatus::Starting => {}
                }
                tokio::select! {
                    _ = activation.stopping() => break,
                    _ = remote.fence() => return Err(failure(SidecarError::Unavailable)),
                    result = changed.changed() => { if result.is_err() { break; } }
                }
            }
        }
        activation.begin_stopping();
        let children = activation.child_settlement();
        let bytes = tokio::select! {
            result = children.cleanup(&mut changed, async {
                // A start abandoned by retirement still owns native capacity.
                children.settle_controls().await?;
                remote.call(true, &control).await
            }) => match result {
                Ok(bytes) => bytes,
                Err(error) => return Err(remote.failed(error).await),
            },
            _ = remote.fence() => return Err(failure(SidecarError::Unavailable)),
        };
        let result = receipt(&bytes);
        let exit = match result {
            Ok((exit, true)) => exit,
            result => {
                let error = match result {
                    Ok((exit, false)) => {
                        children.cleanup_phase(ObserverStatus::Retained(exit));
                        failure(exit)
                    }
                    Err(error) => error,
                    _ => unreachable!(),
                };
                let error = Arc::new(error);
                activation.retain(
                    Retained {
                        context,
                        status: changed,
                        children: Arc::downgrade(&children),
                    },
                    error.clone(),
                );
                return Err(crate::Error::TaskFailed {
                    domain: "remote observer cleanup",
                    source: error,
                });
            }
        };
        if exit == ObserverExit::Cancelled {
            Ok(())
        } else {
            Err(failure(exit))
        }
    }
}

impl crate::supervision::cleanup::Resource for Retained {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move {
            let children = self.children.upgrade().ok_or_else(|| {
                Arc::new(crate::Error::NotReady {
                    domain: "observer cleanup owner",
                })
            })?;
            let closing = self.context.remote.call(true, &self.context.control);
            let bytes = children
                .cleanup(&mut self.status, closing)
                .await
                .map_err(Arc::new)?;
            let (exit, settled) = receipt(&bytes).map_err(Arc::new)?;
            if settled {
                Ok(())
            } else {
                children.cleanup_phase(ObserverStatus::Retained(exit));
                Err(Arc::new(failure(exit)))
            }
        })
    }
}

impl Session {
    pub(super) fn new(
        remote: Arc<Remote>,
        manager: ObserverManager,
        end: Option<harmos_sidecar::attach::Ending>,
    ) -> Self {
        Self {
            closing: Some(Closing {
                remote,
                manager,
                scope: None,
                end,
                ending_failure: None,
            }),
            outcome: None,
        }
    }

    pub(super) fn set_scope(&mut self, scope: ObserverScope<Remote>) -> Result<(), String> {
        let closing = self
            .closing
            .as_mut()
            .ok_or("native session already ended")?;
        if closing.scope.is_some() {
            return Err("native observer scope already bound".to_owned());
        }
        let original = Arc::new(Retirement {
            manager: closing.manager.clone(),
            scope,
        });
        closing
            .remote
            .retirement
            .set(Arc::downgrade(&original))
            .map_err(|_| "native observer retirement already bound".to_owned())?;
        closing.scope = Some(original);
        Ok(())
    }

    pub(super) async fn end(&mut self) -> Result<(), String> {
        if let Some(closing) = self.closing.as_mut() {
            closing.end().await?;
        }
        Ok(())
    }

    pub(super) async fn finish(&mut self) -> Result<(), String> {
        if let Some(closing) = self.closing.as_mut() {
            self.outcome = closing.finish().await?;
            self.closing.take();
        }
        Ok(())
    }

    pub(super) fn outcome(&self) -> Option<&Arc<crate::Error>> {
        self.outcome.as_ref()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut closing) = self.closing.take() {
            crate::supervision::executor::task(async move {
                let _ = closing.finish().await;
            });
        }
    }
}

impl Closing {
    async fn end(&mut self) -> Result<(), String> {
        if let Some(failure) = &self.ending_failure {
            return Err(failure.clone());
        }
        if let Some(end) = self.end.as_mut() {
            let mut ending = Box::pin(end.settle());
            let result = std::future::poll_fn(|context| {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ending.as_mut().poll(context)
                })) {
                    Ok(poll) => poll.map(Ok),
                    Err(_) => {
                        std::task::Poll::Ready(Err("native ending settlement panicked".to_owned()))
                    }
                }
            })
            .await;
            drop(ending);
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error.to_string()),
                Err(failure) => {
                    self.ending_failure = Some(failure.clone());
                    return Err(failure);
                }
            }
            self.end.take();
        }
        self.remote.fenced.send_replace(true);
        self.remote
            .states
            .lock()
            .expect("remote observer states are healthy")
            .clear();
        Ok(())
    }

    async fn finish(&mut self) -> Result<Option<Arc<crate::Error>>, String> {
        self.end().await?;
        if let Some(scope) = &self.scope {
            let retirement: crate::Result<crate::observers::ObserverRetirement> =
                scope.manager.retire_scope(&scope.scope).await;
            match retirement {
                Ok(retirement) => {
                    let outcome = retirement.into_outcome().err();
                    *self
                        .remote
                        .outcome
                        .lock()
                        .expect("observer outcome is healthy") = outcome.clone();
                    return Ok(outcome);
                }
                Err(error) => {
                    return Err(error.to_string());
                }
            }
        }
        Ok(None)
    }
}

/* === Public Functions === */
pub(super) fn stage(
    manager: &ObserverManager,
    scope: &ObserverScope<Remote>,
    plugin: Option<&crate::load::PluginScope>,
    definitions: &[Declaration],
    publications: &[harmos_sidecar::StreamDeclaration],
) -> crate::Result<crate::observers::ObserverRegistration> {
    let backends = definitions.iter().map(|entry| Backend {
        definition: ObserverDefinition {
            id: entry.id.clone(),
            interval: entry.interval,
            demand_keys: entry.demand_keys.clone(),
            streams: publications
                .iter()
                .filter(|row| entry.streams.contains(&row.id))
                .cloned()
                .collect(),
        },
    });
    match plugin {
        Some(plugin) => manager.stage_plugin(scope, plugin, backends),
        None => manager.stage(scope, backends),
    }
}

pub(super) async fn demand(
    manager: ObserverManager,
    scope: ObserverScope<Remote>,
    plugin: Option<crate::load::PluginScope>,
    definitions: Vec<Declaration>,
    mut changes: watch::Receiver<BTreeMap<String, bool>>,
    arrivals: mpsc::Sender<Arrival>,
) {
    if let Some(plugin) = plugin
        && plugin.ready().await.is_err()
    {
        return;
    }

    let mut leases = BTreeMap::new();
    loop {
        let wanted = changes.borrow_and_update().clone();
        for definition in &definitions {
            let active = definition.activation == Activation::Plugin
                || definition
                    .streams
                    .iter()
                    .chain(&definition.demand_keys)
                    .any(|key| wanted.get(key).copied().unwrap_or(false));
            if !active {
                leases.remove(&definition.id);
            } else if !leases.contains_key(&definition.id) {
                match manager.activate(&definition.id, &scope).await {
                    Ok(lease) => {
                        leases.insert(definition.id.clone(), lease);
                    }
                    Err(crate::Error::Closing { .. }) => return,
                    Err(error) => {
                        let _ = arrivals
                            .send(Arrival::ObserverFailed(error.to_string()))
                            .await;
                        return;
                    }
                }
            }
        }
        if changes.changed().await.is_err() {
            break;
        }
    }
}

/* === Private Functions === */
fn receipt(bytes: &[u8]) -> crate::Result<(ObserverExit, bool)> {
    use harmos_contract::Message;
    let status = harmos_contract::observers::Status::decode(bytes).map_err(failure)?;
    let exit = ObserverExit::try_from(status.exit).map_err(failure)?;
    Ok((exit, status.settled))
}

fn failure(error: impl std::error::Error + Send + Sync + 'static) -> crate::Error {
    crate::Error::TaskFailed {
        domain: "remote observer",
        source: Arc::new(error),
    }
}

/* === Tests === */
