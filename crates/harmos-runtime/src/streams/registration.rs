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
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports

// Local Imports
use super::definition::{Table, Wired};
use super::publications::{publication_definitions, validate_existing};
use super::{PluginStream, PublicationLease, StreamAdmissionError, StreamManager, window};
use crate::plugin::PluginScope;
use crate::{Application, state::Position};
use harmos_domain::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "retain the stream registration through plugin settlement"]
pub struct StreamRegistration {
    table: Arc<Table>,
    scope: PluginScope,
    bound: bool,
    activation: Arc<super::Activation>,
}

#[derive(Debug)]
pub(super) struct Staged {
    pub(super) scope: PluginScope,
    pub(super) previous: Option<Arc<Wired>>,
    pub(super) activation: Weak<super::Activation>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> StreamManager<A> {
    pub async fn stage_plugin(
        &self, scope: &PluginScope, declarations: impl IntoIterator<Item = StreamDeclaration>,
    ) -> Result<StreamRegistration, StreamAdmissionError> {
        let definitions = publication_definitions(scope.owner(), declarations)?;
        if !scope.matches_streams(&definitions) {
            return Err(StreamAdmissionError::Inventory);
        }
        let validations = definitions
            .iter()
            .map(|definition| {
                crate::schema::Validator::from_schema(
                    definition.schema().expect("plugin definitions own a row schema"),
                    &harmos_domain::config::SchemaSettings::default(),
                )
                .map_err(crate::schema::Error::from)
                .map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if scope.is_closed() || self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(StreamAdmissionError::Closed);
        }
        let activation = Arc::new(super::Activation::new(&self.windows));
        let mut registration = {
            let _visibility = self.windows.visibility.read().expect("stream visibility poisoned");
            let mut windows = self.windows.windows.write().expect("stream registry poisoned");
            validate_existing(&windows, &definitions, true)?;
            // A transport restart within an already committed activation may
            // renew its exact writers, but cannot mutate the admitted inventory.
            if scope.has_committed() {
                for definition in &definitions {
                    let previous = windows
                        .get(definition.id())
                        .and_then(|revisions| revisions.get(&definition.version()))
                        .ok_or_else(|| StreamAdmissionError::Changed(definition.id().into()))?;
                    if previous.definition.metadata() != definition.metadata()
                        || previous.definition.row_metadata() != definition.row_metadata()
                    {
                        return Err(StreamAdmissionError::Changed(definition.id().into()));
                    }
                }
            }
            let mut leases = Vec::with_capacity(definitions.len());
            for (definition, validation) in definitions.into_iter().zip(validations) {
                let revisions = windows.entry(Arc::<str>::from(definition.id())).or_default();
                // One settled snapshot avoids retaining a chain of generations.
                // Existing readers independently own their immutable descriptors.
                let previous = revisions.get(&definition.version()).map(|previous| {
                    Arc::new(Wired {
                        validation: previous.validation.clone(),
                        definition: previous.definition.clone(),
                        window: previous.window.clone(),
                        publisher: Mutex::new(Weak::new()),
                        staged: None,
                    })
                });
                let window = previous.as_ref().map_or_else(
                    || {
                        Arc::new(
                            window::Window::wired(
                                definition.window(),
                                Position::ORIGIN,
                                self.windows.sequence.clone(),
                                self.windows.pump.clone(),
                            )
                            .visible_with(self.windows.visibility.clone()),
                        )
                    },
                    |previous| previous.window.clone(),
                );
                let active = Arc::new(Mutex::new(true));
                let wired = Arc::new(Wired {
                    validation: Some(validation),
                    definition,
                    window,
                    publisher: Mutex::new(Arc::downgrade(&active)),
                    staged: Some(Staged { scope: scope.clone(), previous, activation: Arc::downgrade(&activation) }),
                });
                revisions.insert(wired.definition.version(), wired.clone());
                leases.push(Arc::new(PublicationLease {
                    stream: PluginStream {
                        wired,
                        tier: self.tier.clone(),
                        closed: self.closed.clone(),
                        pool: self.windows.publications.clone(),
                        ingest: self.windows.ingest.clone(),
                    },
                    active,
                    applied: self.applied.clone(),
                    sequence: std::sync::atomic::AtomicU64::new(0),
                }));
            }
            activation.retain_leases(leases);
            StreamRegistration { table: self.windows.clone(), scope: scope.clone(), bound: false, activation }
        };

        for lease in registration.leases() {
            let wired = &lease.stream.wired;
            let floor = self.tier.seal(wired.definition.clone(), Vec::new()).await;
            if scope.is_closed() || self.closed.load(std::sync::atomic::Ordering::Acquire) {
                return Err(StreamAdmissionError::Closed);
            }
            if wired.staged.as_ref().expect("staged inventory").previous.is_none()
                && floor > Position::ORIGIN
                && floor <= *self.applied.borrow()
            {
                wired.window.stands_on(floor);
            }
        }
        scope.attach_streams(&registration.activation);
        if !scope.bind_streams() {
            return Err(StreamAdmissionError::Closed);
        }
        registration.bound = true;
        Ok(registration)
    }
}

impl StreamRegistration {
    pub fn leases(&self) -> &[Arc<PublicationLease>] {
        self.activation.leases()
    }
}

impl Drop for StreamRegistration {
    fn drop(&mut self) {
        if self.bound {
            self.scope.unbind_streams();
        }
        for lease in self.leases() {
            lease.revoke();
        }
        self.activation.abort();
        if self.scope.has_committed() {
            return;
        }
        let _visibility = self.table.visibility.read().expect("stream visibility poisoned");
        let mut windows = self.table.windows.write().expect("stream registry poisoned");
        for lease in self.leases() {
            let wired = &lease.stream.wired;
            let Some(revisions) = windows.get_mut(wired.definition.id()) else {
                continue;
            };
            if !revisions.get(&wired.definition.version()).is_some_and(|current| Arc::ptr_eq(current, wired)) {
                continue;
            }
            match &wired.staged.as_ref().expect("staged inventory").previous {
                Some(previous) => {
                    revisions.insert(wired.definition.version(), previous.clone());
                }
                None => {
                    revisions.remove(&wired.definition.version());
                }
            }
            if revisions.is_empty() {
                windows.remove(wired.definition.id());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
