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
    pub(in crate::operations) fn capture_routed(&self, id: &str, owner: &str, origin: &Origin) -> Result<Arc<Entry>> {
        if origin.has_lifecycle_loan() {
            let state = self.registry.state.lock().expect("operation registry lock is healthy");
            return state.select(id, Some(owner)).cloned();
        }
        self.capture_provider(id, Some(owner))
    }

    pub(in crate::operations) fn entry(&self, id: &str) -> Result<Arc<Entry>> {
        let entry = self.capture_entry(id)?;
        entry.available()?;
        Ok(entry)
    }

    pub(in crate::operations) fn entry_as(&self, id: &str, _origin: &Origin) -> Result<Arc<Entry>> {
        #[cfg(feature = "plugin")]
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

    #[cfg(feature = "plugin-sidecar")]
    pub(in crate::operations) fn encode_assignments(&self, scope: &PluginScope, allowance: usize) -> Result<Vec<u8>> {
        let exhausted = || Error::ResourceExhausted { resource: "SDK encoding memory" };
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        let contracts = || {
            scope
                .wire_contracts()
                .filter(|contract| scope.requirement(&contract.operation().id).is_none())
                .chain(scope.required_contracts().into_iter().flatten().map(|required| &required.contract))
        };
        let select = |contract: &crate::operations::Contract| -> Result<&Arc<Entry>> {
            let id = contract.operation().id.as_str();
            if scope.requirement(&contract.operation().id).is_some() {
                if let Some(owner) = scope.required_provider(id) {
                    state.select(id, Some(owner))
                } else {
                    state.host_provider(id, contract)
                }
            } else {
                state.owned(id, scope.owner()).ok_or_else(|| Error::NotRegistered {
                    domain: "assigned operation",
                    name: id.to_owned(),
                })
            }
        };
        let mut peak = std::mem::size_of::<Vec<harmos_sdk::operations::AssignedOperation>>() + 20;
        for contract in contracts() {
            let entry = select(contract)?;
            if entry.contract.as_ref() != Some(contract) {
                return Err(Error::InvalidInput {
                    field: "operation assignment",
                    reason: "provider differs from the frozen contract",
                });
            }
            let name =
                entry.owner.as_str().len().checked_add(entry.id().len()).and_then(|length| length.checked_add(1)).ok_or_else(exhausted)?;
            let encoded = name.checked_add(52).and_then(|length| length.checked_mul(2)).ok_or_else(exhausted)?;
            peak = peak
                .checked_add(std::mem::size_of::<harmos_sdk::operations::AssignedOperation>())
                .and_then(|length| length.checked_add(name))
                .and_then(|length| length.checked_add(encoded))
                .ok_or_else(exhausted)?;
            if peak > allowance {
                return Err(exhausted());
            }
        }
        if peak > allowance {
            return Err(exhausted());
        }
        let assignments = contracts()
            .map(|contract| {
                let entry = select(contract)?;
                Ok(harmos_sdk::operations::AssignedOperation {
                    operation: format!("{}.{}", entry.owner, entry.id()),
                    fingerprint: entry.definition.fingerprint.0,
                    generation: entry.generation.value(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        harmos_sdk::operations::AssignedOperation::encode(&assignments).map_err(|source| Error::OperationFailed {
            operation: "SDK assignments".into(),
            source: Arc::new(source),
        })
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
                #[cfg(feature = "plugin")]
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
