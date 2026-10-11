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
use std::sync::{Arc, Weak};
// Third Party Imports

// Local Imports
use super::{
    ObserverActivation, ObserverBackend, ObserverDefinition, ObserverManager, ObserverRegistration,
    ObserverScope,
};
use crate::{Error, Result};
use harmos_sidecar::{Instance, ObserverExit, ObserverStatus as NativeStatus, ObserverTask};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Native<R, H, F> {
    instance: Instance<R, H>,
    factory: Arc<F>,
}

struct Retained(ObserverTask, Weak<super::ChildSettlement>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::supervision::cleanup::Resource for Retained {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move {
            let children = self.1.upgrade().ok_or_else(|| {
                Arc::new(Error::NotReady {
                    domain: "observer cleanup owner",
                })
            })?;
            let mut status = self.0.subscribe();
            children
                .cleanup(&mut status, self.0.recover())
                .await
                .map_err(|source| Arc::new(failed(source)))
        })
    }
}

impl ObserverManager {
    pub fn stage_native<R, H, F>(
        &self,
        scope: &ObserverScope<R>,
        instances: impl IntoIterator<Item = Instance<R, H>>,
        factory: F,
    ) -> Result<ObserverRegistration>
    where
        R: Send + Sync + 'static,
        H: Send + Sync + 'static,
        F: Fn(Arc<R>) -> H + Send + Sync + 'static,
    {
        let factory = Arc::new(factory);
        self.stage(
            scope,
            instances.into_iter().map(|instance| Native {
                instance,
                factory: factory.clone(),
            }),
        )
    }

    pub fn register_native<R, H, F>(&self, instance: Instance<R, H>, factory: F) -> Result<()>
    where
        R: Send + Sync + 'static,
        H: Send + Sync + 'static,
        F: Fn(Arc<R>) -> H + Send + Sync + 'static,
    {
        self.register(Native {
            instance,
            factory: Arc::new(factory),
        })
    }
}

impl<R, H, F> ObserverBackend for Native<R, H, F>
where
    R: Send + Sync + 'static,
    H: Send + Sync + 'static,
    F: Fn(Arc<R>) -> H + Send + Sync + 'static,
{
    type Resources = R;
    fn definition(&self) -> ObserverDefinition {
        ObserverDefinition {
            id: self.instance.id().to_owned(),
            interval: self.instance.interval(),
            demand_keys: self
                .instance
                .demand_keys()
                .iter()
                .map(|key| key.name())
                .collect(),
            streams: self
                .instance
                .streams()
                .iter()
                .map(|entry| harmos_contract::inventory::StreamDeclaration {
                    id: entry.id.to_owned(),
                    metadata: entry.metadata(),
                    row_metadata: entry.row_metadata(),
                    schema: entry.schema,
                    window: entry.window,
                    fields: entry.fields.to_vec(),
                })
                .collect(),
        }
    }

    async fn run(&self, resources: Arc<R>, mut activation: ObserverActivation) -> Result<()> {
        let factory = self.factory.clone();
        let bound = resources.clone();
        let mut task = self
            .instance
            .start_with(resources, move || factory(bound))
            .map_err(failed)?;
        let mut state = task.subscribe();
        loop {
            let current = *state.borrow_and_update();
            match current {
                NativeStatus::Running => activation.ready(),
                NativeStatus::Stopping | NativeStatus::Finalizing => activation.begin_stopping(),
                NativeStatus::Stopped(_) | NativeStatus::Retained(_) => break,
                _ => {}
            }
            tokio::select! {
                _ = activation.stopping() => { task.cancel(); break; }
                changed = state.changed() => { if changed.is_err() { break; } }
            }
        }
        // A Coalesced Terminal Update Must Also Close Lease Admission.
        activation.begin_stopping();
        let children = activation.child_settlement();
        if let Err(source) = children.cleanup(&mut state, task.join()).await {
            let failure = Arc::new(failed(source));
            activation.retain(Retained(task, Arc::downgrade(&children)), failure.clone());
            return Err(Error::TaskFailed {
                domain: "native observer cleanup",
                source: failure,
            });
        }
        match task.status() {
            NativeStatus::Stopped(ObserverExit::Cancelled) => Ok(()),
            NativeStatus::Stopped(exit) => Err(failed(exit)),
            _ => Err(Error::NotReady {
                domain: "observer lifecycle",
            }),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::TaskFailed {
        domain: "native observer",
        source: Arc::new(source),
    }
}

/* === Tests === */
