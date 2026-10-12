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
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports

// Local Imports
use super::{ObserverBackend, ObserverManager, ObserverScope, Registration, Scope, validate};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[must_use = "commit the observer catalog or drop it to release its reservations"]
pub struct ObserverRegistration {
    scope: Arc<Scope>,
    entries: Vec<Arc<Registration>>,
    published: Arc<AtomicBool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl ObserverManager {
    pub fn stage<B: ObserverBackend>(
        &self,
        scope: &ObserverScope<B::Resources>,
        backends: impl IntoIterator<Item = B>,
    ) -> Result<ObserverRegistration> {
        self.reserve(
            scope,
            backends,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            None,
        )
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn stage_plugin<B: ObserverBackend>(
        &self,
        scope: &ObserverScope<B::Resources>,
        plugin: &crate::plugin::PluginScope,
        backends: impl IntoIterator<Item = B>,
    ) -> Result<ObserverRegistration> {
        self.reserve(scope, backends, Some(plugin.clone()))
    }

    fn reserve<B: ObserverBackend>(
        &self,
        scope: &ObserverScope<B::Resources>,
        backends: impl IntoIterator<Item = B>,
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        plugin: Option<crate::plugin::PluginScope>,
    ) -> Result<ObserverRegistration> {
        self.check_scope(&scope.scope)?;
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if plugin.as_ref().is_some_and(|scope| scope.is_closed()) {
            return Err(Error::Closing {
                domain: "plugin scope",
            });
        }
        let published = Arc::new(AtomicBool::new(false));
        let mut entries = Vec::new();
        let mut ids = BTreeSet::new();
        for backend in backends {
            let definition = backend.definition();
            validate(&definition)?;
            if !ids.insert(definition.id.clone()) {
                return Err(Error::AlreadyRegistered {
                    domain: "observer",
                    name: definition.id,
                });
            }
            entries.push(Arc::new(Registration {
                definition,
                backend: Arc::new(backend),
                closing: AtomicBool::new(false),
                scope: Some(scope.id()),
                published: published.clone(),
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                plugin: plugin.clone(),
            }));
        }

        let mut registry = self
            .inner
            .registry
            .lock()
            .expect("observer registry is healthy");
        registry.admit()?;
        if scope.scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "observer scope",
            });
        }
        for entry in &entries {
            if registry.definitions.contains_key(&entry.definition.id) {
                return Err(Error::AlreadyRegistered {
                    domain: "observer",
                    name: entry.definition.id.clone(),
                });
            }
        }
        for entry in &entries {
            registry
                .definitions
                .insert(entry.definition.id.clone(), entry.clone());
        }
        Ok(ObserverRegistration {
            scope: scope.scope.clone(),
            entries,
            published,
        })
    }
}

impl ObserverRegistration {
    pub fn commit(self) -> Result<()> {
        let owner = self
            .scope
            .owner
            .upgrade()
            .ok_or(Error::Closing { domain: "observer" })?;
        let registry = owner.registry.lock().expect("observer registry is healthy");
        registry.admit()?;
        if self.scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "observer scope",
            });
        }
        for entry in &self.entries {
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            if entry.plugin.as_ref().is_some_and(|scope| scope.is_closed()) {
                return Err(Error::Closing {
                    domain: "plugin scope",
                });
            }
            if entry.closing.load(Ordering::Acquire)
                || !registry
                    .definitions
                    .get(&entry.definition.id)
                    .is_some_and(|current| Arc::ptr_eq(current, entry))
            {
                return Err(Error::NotRegistered {
                    domain: "observer reservation",
                    name: entry.definition.id.clone(),
                });
            }
        }
        self.published.store(true, Ordering::Release);
        Ok(())
    }
}

impl Drop for ObserverRegistration {
    fn drop(&mut self) {
        if self.published.load(Ordering::Acquire) {
            return;
        }
        let Some(owner) = self.scope.owner.upgrade() else {
            return;
        };
        let mut registry = owner.registry.lock().expect("observer registry is healthy");
        for entry in &self.entries {
            if registry
                .definitions
                .get(&entry.definition.id)
                .is_some_and(|current| Arc::ptr_eq(current, entry))
            {
                registry.definitions.remove(&entry.definition.id);
            }
        }
        // self.entries retains every backend until this lock has been released.
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
