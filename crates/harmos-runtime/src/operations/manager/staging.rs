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
    #[cfg(feature = "plugin")]
    pub(in crate::operations) fn track_staging(&self, scope: &Arc<PluginScope>) -> Result<()> {
        let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing { domain: "plugin operation staging" });
        }
        scope.bind_operation_registry(&self.registry, self.downgrade())?;
        state.staging.retain(|scope| scope.strong_count() != 0);
        state.staging.push(Arc::downgrade(scope));
        Ok(())
    }

    #[cfg(feature = "plugin")]
    pub(in crate::operations) fn validate_staging(&self, entries: &[Arc<Entry>]) -> Result<()> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing
            || !self.instances.is_accepting()
            || entries.iter().any(|entry| {
                !entry.active.load(Ordering::Acquire)
                    || !state.owned(entry.id(), entry.owner.as_str()).is_some_and(|original| Arc::ptr_eq(original, entry))
            })
        {
            return Err(Error::Closing { domain: "plugin operation staging" });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
