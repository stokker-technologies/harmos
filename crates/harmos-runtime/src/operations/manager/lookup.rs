/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl OperationManager {
    pub(in crate::operations) fn entry(&self, id: &str) -> Result<Arc<Entry>> {
        let entry = self.capture_entry(id)?;
        entry.available()?;
        Ok(entry)
    }

    pub(in crate::operations) fn entry_as(&self, id: &str, _origin: &Origin) -> Result<Arc<Entry>> {
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if let Some(scope) = &_origin.scope {
            if let Some(required) = Identifier::new(id)
                .ok()
                .and_then(|identifier| scope.requirement(&identifier).cloned())
                .filter(|_| scope.required_provider(id).is_none())
            {
                let entry = {
                    let state = self.registry.state.lock().expect("operation registry lock is healthy");
                    Arc::clone(state.host_provider(id, &required.contract)?)
                };
                entry.available_to(_origin)?;
                return Ok(entry);
            }
            if let Some(provider) = scope.required_provider(id) {
                let entry = if _origin.has_lifecycle_loan() {
                    let state = self.registry.state.lock().expect("operation registry lock is healthy");
                    Arc::clone(state.select(id, Some(provider))?)
                } else {
                    self.capture_provider(id, Some(provider))?
                };
                entry.available_to(_origin)?;
                return Ok(entry);
            }
            let owned = self.registry.state.lock().expect("operation registry lock is healthy").owned(id, scope.owner()).cloned();
            if let Some(entry) = owned {
                entry.available_to(_origin)?;
                return Ok(entry);
            }
        }
        self.entry(id)
    }

    // Capture Does Not Grant Readiness; Submission Rechecks The Exact Entry.
    pub(in crate::operations) fn capture_entry(&self, id: &str) -> Result<Arc<Entry>> {
        self.capture_provider(id, None)
    }

    pub(in crate::operations) fn capture_provider(&self, id: &str, owner: Option<&str>) -> Result<Arc<Entry>> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing { domain: "operations" });
        }
        let entry = Some(state.select(id, owner)?)
            .filter(|entry| {
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                if entry.sdk.is_some() {
                    return false;
                }
                entry.active.load(Ordering::Acquire)
            })
            .cloned()
            .ok_or_else(|| Error::NotRegistered { domain: "operation", name: id.to_owned() })?;
        Ok(entry)
    }
}
