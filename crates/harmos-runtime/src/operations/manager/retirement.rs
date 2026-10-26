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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) fn scope_entries(&self, scope: &crate::plugin::PluginScope) -> Vec<Arc<Entry>> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        state
            .entries
            .values()
            .flat_map(|owners| owners.values())
            .filter(|entry| entry.plugin.as_ref().is_some_and(|original| original.same_activation(scope)))
            .cloned()
            .collect()
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        let (owner, local) = id.as_str().split_once('.').expect("qualified identity has two validated segments");
        self.unregister_selected(local, Some(owner)).await
    }

    pub async fn unregister_from(&self, owner: &str, id: &str) -> Result<()> {
        self.unregister_selected(id, Some(owner)).await
    }

    pub(super) async fn unregister_selected(&self, id: &str, owner: Option<&str>) -> Result<()> {
        let entry = {
            let state = self.registry.state.lock().expect("operation registry lock is healthy");
            Arc::clone(state.select(id, owner)?)
        };
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if let Some(scope) = &entry.plugin {
            // Notify the staging gate before revocation, outside registry guards.
            scope.unbind_operations();
        }
        let retired = {
            let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
            if !state.owned(id, entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, &entry)) {
                return Err(Error::NotRegistered { domain: "operation", name: id.to_owned() });
            }
            let _pending = entry.pending.lock().expect("operation settlement lock is healthy");
            entry.retirement.store(Retirement::Explicit as u8, Ordering::Release);
            entry.active.store(false, Ordering::Release);
            for control in _pending.values() {
                control.cancel();
            }
            let retired = state.remove_cache(|cached| !cached.target.upgrade().is_some_and(|target| !Arc::ptr_eq(&target, &entry)));
            drop(_pending);
            retired
        };
        drop(retired);
        settle(&entry).await?;
        let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.owned(id, entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, &entry)) {
            state.remove_owned(id, entry.owner.as_str());
        }
        Ok(())
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) fn fence(&self, entries: &[Arc<Entry>], abandoned: bool) {
        let retired = {
            let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
            for entry in entries {
                let _pending = entry.pending.lock().expect("operation settlement lock is healthy");
                // A by-name unregister retains its reservation for retry even
                // when the plugin receipt is abandoned during that wait.
                let _ = entry.retirement.try_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                    (state != Retirement::Explicit as u8).then_some(if abandoned {
                        Retirement::Abandoned as u8
                    } else {
                        Retirement::Retained as u8
                    })
                });
                entry.active.store(false, Ordering::Release);
            }
            state.remove_cache(|cached| {
                cached.target.upgrade().is_some_and(|target| entries.iter().any(|entry| Arc::ptr_eq(&target, entry)))
            })
        };
        drop(retired);
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) fn remove_settled(&self, entries: &[Arc<Entry>]) {
        let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
        for entry in entries {
            if entry.settled() && state.owned(entry.id(), entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, entry)) {
                state.remove_owned(entry.id(), entry.owner.as_str());
            }
        }
        // The caller owns every entry until after this guard is released.
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) async fn settle_entries(&self, entries: &[Arc<Entry>]) -> Result<()> {
        self.settle_owned_entries(entries).await?;
        self.remove_settled(entries);
        Ok(())
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) async fn settle_owned_entries(&self, entries: &[Arc<Entry>]) -> Result<()> {
        for entry in entries {
            settle(entry).await?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
