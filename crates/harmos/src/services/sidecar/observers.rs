/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Remote observers of one sidecar connection. Each activation submits the
// plugin's observe control and holds that invocation for as long as the
// activation runs; stopping cancels it, and the sidecar stops and cleans the
// observer up through the invocation's ordinary cleanup. The activation keeps
// the connection alive until its invocation settles.

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
    ChildSettlement, ObserverActivation, ObserverBackend, ObserverDefinition, ObserverRegistration,
};
use crate::operations::{OperationManager, OperationPhase};
use harmos_sidecar::inventory::observers::{Activation, Declaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct ControlContext {
    operations: OperationManager,
    scope: crate::plugin::PluginScope,
    binding: OnceLock<crate::operations::PluginControl>,
}

pub(super) struct Remote {
    pub(super) provider: Option<Arc<crate::supervision::instance::Instance>>,
    arrivals: mpsc::Sender<Arrival>,
    control: OnceLock<ControlContext>,
    fenced: watch::Sender<bool>,
    retirement: OnceLock<Weak<Retirement>>,
    outcome: Arc<Mutex<Option<Arc<crate::Error>>>>,
}

struct Backend {
    definition: ObserverDefinition,
}

struct RetainedControl(Arc<crate::supervision::instance::Instance>);

pub(super) struct Session {
    closing: Option<Closing>,
    outcome: Option<Arc<crate::Error>>,
}

struct Retirement {
    operations: OperationManager,
    remote: Arc<Remote>,
}

struct Closing {
    remote: Arc<Remote>,
    operations: OperationManager,
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
        let retired = original
            .operations
            .retire_scope_of(&original.remote)
            .await?;
        *self.outcome.lock().expect("observer outcome is healthy") =
            retired.and_then(|retired| retired.into_outcome().err());
        Ok(())
    }

    pub(super) fn configure(
        &self,
        operations: OperationManager,
        scope: crate::plugin::PluginScope,
    ) -> crate::Result<()> {
        self.control
            .set(ControlContext {
                operations,
                scope,
                binding: OnceLock::new(),
            })
            .map_err(|_| crate::Error::InvalidInput {
                field: "observer control",
                reason: "original transport was already configured",
            })
    }

    pub(super) fn capture_control(&self) -> crate::Result<()> {
        let original = self.control.get().ok_or(crate::Error::NotReady {
            domain: "observer control",
        })?;
        let Some(contract) = original.scope.observer_control() else {
            return Ok(());
        };
        if original.binding.get().is_some() {
            return Ok(());
        }
        let binding = original
            .operations
            .capture_host_control(&original.scope, contract)?;
        let _ = original.binding.set(binding);
        Ok(())
    }

    // Submit the observe control for one activation, owned by its children.
    fn observe(
        &self,
        children: &ChildSettlement,
        control: &harmos_contract::observers::Control,
    ) -> crate::Result<crate::operations::EncodedInvocation> {
        let original = self.control.get().ok_or(crate::Error::NotReady {
            domain: "observer control",
        })?;
        original
            .scope
            .observer_control()
            .ok_or(crate::Error::NotReady {
                domain: "observer control",
            })?;
        self.capture_control()?;
        let binding = original
            .binding
            .get()
            .expect("original captured observe control");
        let bytes = control
            .observer
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(64))
            .and_then(std::num::NonZeroUsize::new)
            .ok_or(crate::Error::ResourceExhausted {
                resource: "observer control encoding",
            })?;
        children.submit(false, || {
            binding.submit_with(bytes, |_| {
                Ok(harmos_contract::Message::encode_to_vec(control))
            })
        })
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

impl crate::supervision::cleanup::Resource for RetainedControl {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move {
            self.0
                .settle()
                .await
                .map_err(|source| Arc::new(crate::Error::OperationSettlement { source }))
        })
    }
}

impl ObserverBackend for Backend {
    type Resources = Remote;

    fn definition(&self) -> crate::Result<ObserverDefinition> {
        Ok(self.definition.clone())
    }

    async fn run(
        &self,
        remote: Arc<Remote>,
        mut activation: ObserverActivation,
    ) -> crate::Result<()> {
        if let Some(provider) = &remote.provider {
            activation.depend_on(provider)?;
        }
        let control = harmos_contract::observers::Control {
            observer: self.definition.id.clone(),
            generation: activation.id().value(),
        };
        let children = activation.child_settlement();
        let mut invocation = match remote.observe(&children, &control) {
            Ok(invocation) => invocation,
            Err(error) => return Err(remote.failed(error).await),
        };

        // Follow The Remote Observer Until Stopped Or Ended
        let control = Arc::clone(&invocation.control);
        let mut status = control.settlement();
        let ended = loop {
            if status.borrow_and_update().phase() == OperationPhase::Running {
                activation.ready();
            }
            tokio::select! {
                () = activation.stopping() => break None,
                () = remote.fence() => return Err(failure(SidecarError::Unavailable)),
                result = &mut invocation => break Some(result),
                changed = status.changed() => {
                    if changed.is_err() {
                        break Some((&mut invocation).await);
                    }
                }
            }
        };

        // Cancel The Invocation; The Sidecar Stops And Cleans Up The Observer
        activation.begin_stopping();
        let stopped = ended.is_none();
        let result = match ended {
            Some(result) => result,
            None => {
                invocation.cancel();
                tokio::select! {
                    result = &mut invocation => result,
                    () = remote.fence() => return Err(failure(SidecarError::Unavailable)),
                }
            }
        };
        activation.cleaning_up();

        // A Failed Remote Cleanup Stays Retained For A Later Stop To Retry
        if let Some(failure) = control.status().cleanup_failure().cloned() {
            children.forget(&control);
            activation.retain(RetainedControl(control), Arc::clone(&failure));
            return Err(crate::Error::TaskFailed {
                domain: "remote observer cleanup",
                source: failure,
            });
        }
        match result {
            Ok(_) => Ok(()),
            Err(_) if stopped => Ok(()),
            // Cancelled from outside, as runtime admission closes.
            Err(crate::Error::OperationRefused(crate::Refusal::Cancelled)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Session {
    pub(super) fn new(
        remote: Arc<Remote>,
        operations: OperationManager,
        end: Option<harmos_sidecar::attach::Ending>,
    ) -> Self {
        Self {
            closing: Some(Closing {
                remote,
                operations,
                scope: None,
                end,
                ending_failure: None,
            }),
            outcome: None,
        }
    }

    // Bind retirement of this connection's observers to the session.
    pub(super) fn bind(&mut self) -> Result<(), String> {
        let closing = self
            .closing
            .as_mut()
            .ok_or("native session already ended")?;
        if closing.scope.is_some() {
            return Err("native observer scope already bound".to_owned());
        }
        let original = Arc::new(Retirement {
            operations: closing.operations.clone(),
            remote: Arc::clone(&closing.remote),
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
        Ok(())
    }

    async fn finish(&mut self) -> Result<Option<Arc<crate::Error>>, String> {
        self.end().await?;
        if let Some(scope) = &self.scope {
            let retirement = scope.operations.retire_scope_of(&scope.remote).await;
            match retirement {
                Ok(retirement) => {
                    let outcome = retirement.and_then(|retirement| retirement.into_outcome().err());
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
    operations: &OperationManager,
    remote: &Arc<Remote>,
    plugin: Option<&crate::plugin::PluginScope>,
    definitions: &[Declaration],
    publications: &[harmos_sidecar::StreamDeclaration],
) -> crate::Result<ObserverRegistration> {
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
        Some(plugin) => operations.stage_plugin_observers(remote, plugin, backends),
        None => operations.stage_observers(remote, backends),
    }
}

// Hold one activation lease per observer whose plugin or demand wants it.
pub(super) async fn demand(
    operations: OperationManager,
    remote: Arc<Remote>,
    plugin: Option<crate::plugin::PluginScope>,
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
                match operations
                    .submit_with_resources_at(&definition.id, Arc::clone(&remote))
                    .await
                {
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
fn failure(error: impl std::error::Error + Send + Sync + 'static) -> crate::Error {
    crate::Error::TaskFailed {
        domain: "remote observer",
        source: Arc::new(error),
    }
}

/* === Tests === */
