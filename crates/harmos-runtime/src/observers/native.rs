/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runs an authored observer in process: the activation initializes and ticks
// it through its `Activity`, and stopping the activation stops and cleans it
// up. A failed cleanup stays owned by the activation for a later retry.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{ObserverActivation, ObserverBackend, ObserverDefinition};
use crate::{Error, Result};
use harmos_sidecar::{Activity, Instance};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Native<R, H, F> {
    pub(crate) instance: Instance<R, H>,
    pub(crate) factory: Arc<F>,
}

struct Retained(Activity);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl crate::supervision::cleanup::Resource for Retained {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move {
            self.0.stop().await.map_err(|source| Arc::new(failed(source)))?;
            self.0.cleanup().await.map_err(|source| Arc::new(failed(source)))
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

    fn definition(&self) -> Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: self.instance.id().to_owned(),
            interval: self.instance.interval(),
            demand_keys: self.instance.demand_keys().iter().map(|key| key.name()).collect(),
            streams: self
                .instance
                .streams()
                .iter()
                .map(|entry| {
                    Ok(harmos_domain::inventory::StreamDeclaration {
                        id: entry.id.to_owned(),
                        metadata: entry.metadata(),
                        row_metadata: entry.row_metadata(),
                        schema: entry.schema,
                        window: entry.window,
                        row: crate::schema::graph::Schema::from_declaration(entry.row, &crate::config::SchemaSettings::default())
                            .map_err(|source| Error::RecordSchema { source: source.into() })?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }

    async fn run(&self, resources: Arc<R>, mut activation: ObserverActivation) -> Result<()> {
        let host = (self.factory)(Arc::clone(&resources));
        let mut activity = self.instance.activate(resources, host).map_err(failed)?;

        // Observe Until Stopped Or Unable To Continue
        let readiness = activation.readiness();
        let interrupted = tokio::select! {
            failure = activity.observe(readiness) => Some(failure),
            () = activation.stopping() => None,
        };

        // Stop And Clean Up Whatever The Activation Acquired
        activation.begin_stopping();
        let closed = match activity.stop().await {
            Ok(()) => {
                activation.cleaning_up();
                activity.cleanup().await
            }
            Err(failure) => Err(failure),
        };
        if let Err(source) = closed {
            let failure = Arc::new(failed(source));
            activation.retain(Retained(activity), Arc::clone(&failure));
            return Err(Error::TaskFailed {
                domain: "native observer cleanup",
                source: failure,
            });
        }
        match interrupted {
            None => Ok(()),
            Some(failure) => Err(failed(failure)),
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
