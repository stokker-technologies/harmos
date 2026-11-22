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
#[cfg(feature = "plugin-guest")]
use std::sync::Weak;
use std::sync::{Arc, Mutex};
// Third Party Imports

// Local Imports
#[cfg(feature = "plugin-guest")]
use super::definition::Table;
use super::definition::{Contract, Wired};
use super::publications::{publication_definitions, validate_existing};
use super::{PublishedStream, StreamManager, StreamRegistrationError, StreamWriter, window};
#[cfg(feature = "plugin-guest")]
use crate::guest::host::plugin::PluginScope;
use crate::{Application, state::Position};
use harmos_operations::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "retain the stream registration through plugin settlement"]
pub struct StreamRegistration {
    leases: Vec<Arc<StreamWriter>>,

    #[cfg(feature = "plugin-guest")]
    guest: Option<GuestRegistration>,
}

#[cfg(feature = "plugin-guest")]
struct GuestRegistration {
    table: Arc<Table>,
    scope: PluginScope,
    bound: bool,
    activation: Arc<super::Activation>,
}

#[cfg(feature = "plugin-guest")]
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
    pub(crate) fn validate_connection(
        &self, owner: &str, declarations: &[StreamDeclaration], replace: bool,
    ) -> Result<(), StreamRegistrationError> {
        let definitions = publication_definitions(owner, declarations.iter().cloned())?;
        let _visibility = self.windows.visibility.read().expect("stream visibility poisoned");
        let windows = self.windows.windows.read().expect("stream registry poisoned");
        validate_existing(&windows, &definitions, !replace)
    }

    pub(crate) fn register_plugin(
        &self, owner: &str, generation: u64, declarations: impl IntoIterator<Item = StreamDeclaration>, replace: bool,
        #[cfg(feature = "plugin-guest")] scope: Option<&PluginScope>,
    ) -> Result<StreamRegistration, StreamRegistrationError> {
        let definitions = publication_definitions(owner, declarations)?;
        #[cfg(feature = "plugin-guest")]
        if scope.is_some_and(|scope| !scope.matches_streams(&definitions)) {
            return Err(StreamRegistrationError::Inventory);
        }
        let validations = definitions
            .iter()
            .map(|definition| {
                crate::schema::Validator::from_schema(definition.schema(), &self.schema_settings)
                    .map_err(crate::schema::Error::from)
                    .map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        #[cfg(feature = "plugin-guest")]
        if scope.is_some_and(PluginScope::is_closed) {
            return Err(StreamRegistrationError::Closed);
        }
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(StreamRegistrationError::Closed);
        }
        #[cfg(feature = "plugin-guest")]
        let activation = scope.map(|_| Arc::new(super::Activation::new(&self.windows)));
        let registration = {
            let _visibility = self.windows.visibility.write().expect("stream visibility poisoned");
            let mut windows = self.windows.windows.write().expect("stream registry poisoned");
            self.windows.retire(&mut windows);
            validate_existing(&windows, &definitions, !replace)?;
            // A transport restart within an already committed activation may
            // renew its exact writers, but cannot mutate the admitted inventory.
            #[cfg(feature = "plugin-guest")]
            if scope.is_some_and(PluginScope::has_committed) {
                for definition in &definitions {
                    let previous = windows
                        .get(definition.id())
                        .and_then(|revisions| revisions.wires.get(&definition.version()))
                        .ok_or(StreamRegistrationError::Inventory)?;
                    if previous.definition.metadata() != definition.metadata()
                        || previous.definition.row_metadata() != definition.row_metadata()
                    {
                        return Err(StreamRegistrationError::Inventory);
                    }
                }
            }
            if replace {
                for revisions in windows.values() {
                    for wired in revisions.wires.values() {
                        if wired.definition.owner() == Some(owner)
                            && let Some(active) = wired.publisher.lock().expect("stream publisher poisoned").upgrade()
                        {
                            *active.lock().expect("stream lease poisoned") = false;
                        }
                    }
                }
            }
            let mut leases = Vec::with_capacity(definitions.len());
            for (definition, validation) in definitions.into_iter().zip(validations) {
                let revisions = windows.entry(Arc::<str>::from(definition.id())).or_default();
                // One settled snapshot avoids retaining a chain of generations.
                // Existing readers independently own their immutable descriptors.
                #[cfg(feature = "plugin-guest")]
                let previous = scope.and_then(|_| revisions.wires.get(&definition.version())).map(|previous| {
                    Arc::new(Wired {
                        generation: previous.generation,
                        validation: previous.validation.clone(),
                        definition: previous.definition.clone(),
                        window: previous.window.clone(),
                        publisher: Mutex::new(Weak::new()),
                        staged: None,
                    })
                });
                let window = revisions.wires.get(&definition.version()).map_or_else(
                    || {
                        Arc::new(
                            window::Window::wired(
                                definition.window(),
                                Position::ORIGIN,
                                self.windows.sequence.clone(),
                                self.windows.pump.clone(),
                                self.windows.clock.clone(),
                            )
                            .visible_with(self.windows.visibility.clone()),
                        )
                    },
                    |previous| previous.window.clone(),
                );
                let active = Arc::new(Mutex::new(true));
                let wired = Arc::new(Wired {
                    generation: Some(generation),
                    validation: Some(validation),
                    definition,
                    window,
                    publisher: Mutex::new(Arc::downgrade(&active)),
                    #[cfg(feature = "plugin-guest")]
                    staged: scope.map(|scope| Staged {
                        scope: scope.clone(),
                        previous,
                        activation: Arc::downgrade(activation.as_ref().expect("guest activation")),
                    }),
                });
                revisions.definitions.insert(wired.definition.version(), Contract::of(&wired.definition));
                #[cfg(feature = "plugin-guest")]
                let staged = scope.is_some();
                #[cfg(not(feature = "plugin-guest"))]
                let staged = false;
                if !staged {
                    revisions.current = Some(wired.definition.version());
                }
                revisions.wires.insert(wired.definition.version(), wired.clone());
                leases.push(Arc::new(StreamWriter {
                    stream: PublishedStream {
                        wired,
                        closed: self.closed.clone(),
                        pool: self.windows.publications.clone(),
                        ingest: self.windows.ingest.clone(),
                    },
                    active,
                    applied: self.applied.clone(),
                    sequence: std::sync::atomic::AtomicU64::new(0),
                }));
            }
            #[cfg(feature = "plugin-guest")]
            let guest = scope.zip(activation).map(|(scope, activation)| {
                activation.retain_leases(leases.clone());
                GuestRegistration {
                    table: self.windows.clone(),
                    scope: scope.clone(),
                    bound: false,
                    activation,
                }
            });
            StreamRegistration {
                leases,
                #[cfg(feature = "plugin-guest")]
                guest,
            }
        };

        Ok(registration)
    }

    #[cfg(feature = "plugin-guest")]
    pub async fn stage_plugin(
        &self, scope: &PluginScope, declarations: impl IntoIterator<Item = StreamDeclaration>,
    ) -> Result<StreamRegistration, StreamRegistrationError> {
        let mut registration = self.register_plugin(scope.owner(), scope.generation().get(), declarations, false, Some(scope))?;
        if scope.is_closed() || self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(StreamRegistrationError::Closed);
        }
        let guest = registration.guest.as_mut().expect("guest registration");
        scope.attach_streams(&guest.activation);
        if !scope.bind_streams() {
            return Err(StreamRegistrationError::Closed);
        }
        guest.bound = true;
        Ok(registration)
    }
}

impl StreamRegistration {
    pub fn leases(&self) -> &[Arc<StreamWriter>] {
        &self.leases
    }
}

impl Drop for StreamRegistration {
    fn drop(&mut self) {
        for lease in self.leases() {
            lease.revoke();
        }
    }
}

#[cfg(feature = "plugin-guest")]
impl Drop for GuestRegistration {
    fn drop(&mut self) {
        if self.bound {
            self.scope.unbind_streams();
        }
        self.activation.abort();
        if self.scope.has_committed() {
            return;
        }
        let _visibility = self.table.visibility.read().expect("stream visibility poisoned");
        let mut windows = self.table.windows.write().expect("stream registry poisoned");
        for lease in self.activation.leases() {
            let wired = &lease.stream.wired;
            let Some(revisions) = windows.get_mut(wired.definition.id()) else {
                continue;
            };
            if !revisions.wires.get(&wired.definition.version()).is_some_and(|current| Arc::ptr_eq(current, wired)) {
                continue;
            }
            match &wired.staged.as_ref().expect("staged inventory").previous {
                Some(previous) => {
                    revisions.wires.insert(wired.definition.version(), previous.clone());
                }
                None => {
                    revisions.wires.remove(&wired.definition.version());
                }
            }
            if revisions.current.is_none() && revisions.wires.is_empty() {
                windows.remove(wired.definition.id());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
