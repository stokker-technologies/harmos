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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::{RecordManager, Registry};
use crate::load::PluginScope;
use crate::operations::{EncodedInvocation, OperationManager, PluginCallback};
use crate::schema::Described;
use crate::supervision::{Listener, ListenerDelivery};
use crate::{
    Application, CatalogCodec, Error, Message, Position, Record, RecordProjection, Recorded, Result,
};
use harmos_contract::listeners::{Context, Delivery, Source};

/* === Constants === */
const PAGE_ROWS: usize = 64;
const REGISTRATIONS: usize = 64;

/* === Macros === */

/* === Type Aliases === */
type Submit<A> =
    fn(&<A as Application>::Record, Context, &PluginCallback) -> Result<Option<EncodedInvocation>>;

/* === Structs === */
pub struct PluginListenerBindings<'a, A: Application> {
    records: &'a RecordManager<A>,
    scope: &'a PluginScope,
    operations: &'a OperationManager,
    entries: Vec<Binding<A>>,
}

struct Binding<A: Application> {
    ordinal: usize,
    callback: PluginCallback,
    submit: Submit<A>,
}

#[must_use = "retain the listener registration until shutdown positively settles"]
pub struct PluginListenerRegistration {
    registry: Arc<Registry>,
    listening: Option<Arc<Listening>>,
}

pub(super) struct Listening {
    delivery: Listener,
    frontier: Arc<AtomicU64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> RecordManager<A> {
    pub fn stage_listeners(
        &self,
        scope: &PluginScope,
        operations: &OperationManager,
        after: Position,
        configure: impl FnOnce(&mut PluginListenerBindings<'_, A>) -> Result<()>,
    ) -> Result<PluginListenerRegistration> {
        if scope.is_closed() || scope.has_committed() {
            return Err(Error::Closing {
                domain: "record listener staging",
            });
        }
        if after > self.state.applied() {
            return Err(Error::FrontierAhead);
        }
        let mut bindings = PluginListenerBindings {
            records: self,
            scope,
            operations,
            entries: Vec::new(),
        };
        configure(&mut bindings)?;
        let expected = scope
            .listeners()
            .iter()
            .filter(|listener| matches!(listener.source(), Source::Record { .. }))
            .count();
        if bindings.entries.len() != expected {
            return Err(Error::InvalidInput {
                field: "record listeners",
                reason: "the complete frozen native source inventory must be bound",
            });
        }
        if expected == 0 {
            return Ok(PluginListenerRegistration {
                registry: Arc::clone(&self.schemas),
                listening: None,
            });
        }
        bindings.entries.sort_by_key(|entry| entry.ordinal);
        let mut registry = self
            .schemas
            .listeners
            .lock()
            .expect("record listener registry poisoned");
        registry.retain(|_, listening| !listening.delivery.cleaned());
        if registry.contains_key(scope.owner()) {
            return Err(Error::AlreadyRegistered {
                domain: "record listeners",
                name: scope.owner().to_owned(),
            });
        }
        if registry.len() >= REGISTRATIONS {
            return Err(Error::ResourceExhausted {
                resource: "record listener registrations",
            });
        }
        let frontier = Arc::new(AtomicU64::new(after.0));
        let delivery = Listener::spawn(scope, {
            let journal = self.state.clone();
            let scope = scope.clone();
            let frontier = Arc::clone(&frontier);
            move |delivery, stopping| async move {
                deliver(
                    journal,
                    &scope,
                    bindings.entries,
                    &delivery,
                    &frontier,
                    stopping,
                )
                .await
            }
        });
        let listening = Arc::new(Listening { delivery, frontier });
        registry.insert(scope.owner().to_owned(), Arc::clone(&listening));
        drop(registry);
        let mut registration = PluginListenerRegistration {
            registry: Arc::clone(&self.schemas),
            listening: Some(listening),
        };
        // The Receipt Is Retained Before Publishing Its Readiness Contribution.
        if let Err(error) = scope.bind_record_listeners() {
            registration.stop();
            return Err(error);
        }
        Ok(registration)
    }

    pub(crate) fn stop_listeners(&self) {
        let owners: Vec<_> = self
            .schemas
            .listeners
            .lock()
            .expect("record listener registry poisoned")
            .values()
            .cloned()
            .collect();
        for owner in owners {
            owner.stop();
        }
    }

    pub async fn shutdown_listeners(&self) -> Result<()> {
        let owners: Vec<_> = self
            .schemas
            .listeners
            .lock()
            .expect("record listener registry poisoned")
            .values()
            .cloned()
            .collect();
        for owner in &owners {
            owner.stop();
        }
        for owner in owners {
            owner.shutdown().await?;
        }
        self.schemas
            .listeners
            .lock()
            .expect("record listener registry poisoned")
            .retain(|_, owner| !owner.delivery.cleaned());
        Ok(())
    }

    pub async fn shutdown_listeners_for(&self, scope: &PluginScope) -> Result<()> {
        let original = self
            .schemas
            .listeners
            .lock()
            .expect("record listener registry poisoned")
            .get(scope.owner())
            .cloned();
        let Some(original) = original else {
            return Ok(());
        };
        if !original.delivery.scope().same_activation(scope) {
            return Err(Error::InvalidInput {
                field: "record listener scope",
                reason: "scope belongs to a different registered activation",
            });
        }
        original.shutdown().await?;
        self.schemas.remove_listening(&original);
        Ok(())
    }
}

impl<A: Application> PluginListenerBindings<'_, A> {
    pub fn record<R>(&mut self) -> Result<()>
    where
        R: Record + Message + Described,
        A::Record: RecordProjection<R>,
    {
        let (id, version) = R::definition();
        if !self
            .records
            .catalog
            .has_record(&id, version)
            .map_err(|source| Error::RecordDeclaration { source })?
        {
            return Err(Error::NotRegistered {
                domain: "native listener record",
                name: id.to_string(),
            });
        }
        let mut selected = false;
        for (ordinal, listener) in self.scope.listeners().iter().enumerate() {
            let Source::Record {
                id: source,
                version: revision,
                ..
            } = listener.source()
            else {
                continue;
            };
            if source.as_ref() != id.as_str() || *revision != version {
                continue;
            }
            let expected = harmos_contract::listeners::Definition::new(
                listener.id().clone(),
                Source::Record {
                    id: id.as_str().to_owned().into(),
                    version,
                    fields: std::borrow::Cow::Borrowed(R::FIELDS),
                },
                listener.callback().clone(),
            )
            .map_err(|source| Error::OperationFailed {
                operation: listener.callback().operation().id.to_string(),
                source: Arc::new(source),
            })?;
            if expected != *listener {
                return Err(Error::InvalidInput {
                    field: "record listener source",
                    reason: "native row schema differs from the frozen declaration",
                });
            }
            if self.entries.iter().any(|entry| entry.ordinal == ordinal) {
                return Err(Error::AlreadyRegistered {
                    domain: "record listener source",
                    name: listener.id().to_string(),
                });
            }
            self.entries.push(Binding {
                ordinal,
                callback: self
                    .operations
                    .capture_callback(self.scope, listener.callback())?,
                submit: submit::<A, R>,
            });
            selected = true;
        }
        if !selected {
            return Err(Error::NotRegistered {
                domain: "record listener source",
                name: id.to_string(),
            });
        }
        Ok(())
    }
}

impl PluginListenerRegistration {
    pub fn frontier(&self) -> Option<Position> {
        self.listening
            .as_ref()
            .map(|owner| Position(owner.frontier.load(Ordering::Acquire)))
    }

    pub fn failure(&self) -> Option<Arc<Error>> {
        self.listening
            .as_ref()
            .and_then(|owner| owner.delivery.failure())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let Some(owner) = &self.listening else {
            return Ok(());
        };
        owner.shutdown().await?;
        self.registry.remove_listening(owner);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(owner) = &self.listening {
            owner.stop();
        }
    }
}

impl Registry {
    fn remove_listening(&self, owner: &Arc<Listening>) {
        let mut registry = self
            .listeners
            .lock()
            .expect("record listener registry poisoned");
        if registry
            .get(owner.delivery.scope().owner())
            .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            registry.remove(owner.delivery.scope().owner());
        }
    }
}

impl Drop for PluginListenerRegistration {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Listening {
    fn stop(&self) {
        self.delivery.stop();
    }

    async fn shutdown(&self) -> Result<()> {
        self.delivery.shutdown().await
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn submit<A, R>(
    record: &A::Record,
    context: Context,
    callback: &PluginCallback,
) -> Result<Option<EncodedInvocation>>
where
    A: Application,
    R: Record + Message + Described,
    A::Record: RecordProjection<R>,
{
    let Some(row) = record.project() else {
        return Ok(None);
    };
    let (id, version) = R::definition();
    if record.identify() != (id, version) {
        return Err(Error::InvalidInput {
            field: "record listener projection",
            reason: "projection differs from the original native catalog member",
        });
    }
    let bytes =
        Delivery::encoding_memory_bytes(context, row).map_err(|source| Error::OperationFailed {
            operation: callback.contract().operation().id.to_string(),
            source: Arc::new(source),
        })?;
    let bytes = NonZeroUsize::new(bytes).ok_or(Error::ResourceExhausted {
        resource: "record callback encoding",
    })?;
    callback
        .submit_with(bytes, |budget| {
            Delivery::encode_borrowed(context, row, budget).map_err(|source| {
                Error::OperationFailed {
                    operation: callback.contract().operation().id.to_string(),
                    source: Arc::new(source),
                }
            })
        })
        .map(Some)
}

async fn deliver<A: Application>(
    journal: crate::StateManager<A>,
    scope: &PluginScope,
    entries: Vec<Binding<A>>,
    delivery: &ListenerDelivery,
    frontier: &AtomicU64,
    mut stopping: watch::Receiver<bool>,
) -> Result<()> {
    tokio::select! {
        biased;
        _ = stopping.changed() => return Ok(()),
        ready = scope.ready() => ready?,
    }
    let mut progress = journal.progress();
    let feed = journal.feed();
    loop {
        progress.borrow_and_update();
        if *stopping.borrow() || scope.is_closed() {
            return Ok(());
        }
        let after = Position(frontier.load(Ordering::Acquire));
        let page = feed
            .shared_page(
                after,
                journal.applied(),
                NonZeroUsize::new(PAGE_ROWS).unwrap(),
            )
            .await?;
        if page.entries.is_empty() {
            tokio::select! {
                biased;
                _ = stopping.changed() => return Ok(()),
                _ = scope.closed() => return Ok(()),
                changed = progress.changed() => if changed.is_err() { return Ok(()); },
            }
            continue;
        }
        for source in page.entries {
            if *stopping.borrow() || scope.is_closed() {
                return Ok(());
            }
            if let Recorded::Record(record) = &source.value {
                for binding in &entries {
                    let Some(invocation) = (binding.submit)(
                        record,
                        Context::Record {
                            position: source.position,
                        },
                        &binding.callback,
                    )?
                    else {
                        continue;
                    };
                    if !delivery
                        .invoke(invocation, source.clone(), scope, &mut stopping)
                        .await?
                    {
                        return Ok(());
                    }
                }
            }
            frontier.store(source.position.0, Ordering::Release);
        }
    }
}

/* === Tests === */
