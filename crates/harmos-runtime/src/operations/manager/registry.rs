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

impl Registry {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn generation_commit(&self) -> Result<GenerationCommit<'_>> {
        let state = self.state.lock().expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing { domain: "plugin generation commit" });
        }
        Ok(GenerationCommit { _state: state })
    }

    pub(in crate::operations) fn insert<O: OperationDeclaration>(
        &self, entry: &Arc<Entry>, key: CacheKey, identity: Identity, response: crate::operations::Response<O::Response>,
    ) {
        let mut state = self.state.lock().expect("operation registry lock is healthy");
        if state.closing
            || state.cache_capacity == 0
            || entry.available().is_err()
            || !state.owned(entry.id(), entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            return;
        }
        let mut retired = state.remove_cache(|cached| {
            cached.key == key && cached.identity == identity && cached.target.upgrade().is_some_and(|target| Arc::ptr_eq(&target, entry))
        });
        let available = state.cache_capacity - 1;
        retired.extend(state.evict_to(available));
        state.cache.push_back(Cached { target: Arc::downgrade(entry), key, identity, response: response.into_erased() });
        drop(state);
        drop(retired);
    }
}

impl State {
    pub(super) fn owned(&self, id: &str, owner: &str) -> Option<&Arc<Entry>> {
        self.entries.get(id)?.get(owner)
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) fn host_provider(&self, id: &str, contract: &crate::operations::Contract) -> Result<&Arc<Entry>> {
        let mut compatible = self
            .entries
            .get(id)
            .into_iter()
            .flat_map(|providers| providers.values())
            .filter(|entry| entry.plugin.is_none() && entry.contract.as_ref() == Some(contract));
        let selected = compatible.next().ok_or_else(|| Error::NotRegistered { domain: "host provider", name: id.to_owned() })?;
        if compatible.next().is_some() {
            return Err(Error::InvalidInput { field: "host provider", reason: "required contract has multiple host providers" });
        }
        Ok(selected)
    }

    pub(super) fn select(&self, id: &str, owner: Option<&str>) -> Result<&Arc<Entry>> {
        let providers = self.entries.get(id).ok_or_else(|| Error::NotRegistered { domain: "operation", name: id.to_owned() })?;
        if let Some(owner) = owner {
            return providers
                .get(owner)
                .ok_or_else(|| Error::NotRegistered { domain: "operation provider", name: format!("{owner}/{id}") });
        }
        if providers.len() != 1 {
            return Err(Error::InvalidInput {
                field: "operation provider",
                reason: "operation has multiple providers; an explicit provider is required",
            });
        }
        Ok(providers.values().next().expect("one original provider"))
    }

    pub(super) fn remove_owned(&mut self, id: &str, owner: &str) -> Option<Arc<Entry>> {
        let providers = self.entries.get_mut(id)?;
        let removed = providers.remove(owner);
        #[cfg(feature = "grpc")]
        if let Some(binding) = removed.as_ref().and_then(|entry| entry.contract.as_ref()).and_then(|contract| contract.grpc()) {
            self.grpc.remove(&binding.path());
        }
        if providers.is_empty() {
            self.entries.remove(id);
        }
        removed
    }

    pub(super) fn retired(&mut self) -> Vec<Arc<Entry>> {
        let names: Vec<_> = self
            .entries
            .iter()
            .flat_map(|(id, providers)| providers.iter().map(move |(owner, entry)| (id, owner, entry)))
            .filter(|(_, _, entry)| entry.retirement.load(Ordering::Acquire) == Retirement::Abandoned as u8 && entry.settled())
            .map(|(id, owner, _)| (id.clone(), owner.clone()))
            .collect();
        names.into_iter().filter_map(|(id, owner)| self.remove_owned(&id, &owner)).collect()
    }

    pub(super) fn definitions(&self) -> Vec<Definition> {
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        let mut ready = BTreeMap::new();
        self.entries
            .values()
            .flat_map(|providers| providers.values())
            .filter(|entry| {
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                if entry.sdk.is_some() {
                    return false;
                }
                if !entry.active.load(Ordering::Acquire) {
                    return false;
                }
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                if let Some(scope) = &entry.plugin {
                    return *ready.entry(scope.generation()).or_insert_with(|| scope.is_ready());
                }
                true
            })
            .map(|entry| entry.definition.clone())
            .collect()
    }

    pub(super) fn remove_cache(&mut self, mut remove: impl FnMut(&Cached) -> bool) -> Vec<Cached> {
        let mut kept = VecDeque::new();
        let mut retired = Vec::new();
        for cached in std::mem::take(&mut self.cache) {
            if remove(&cached) {
                retired.push(cached);
            } else {
                kept.push_back(cached);
            }
        }
        self.cache = kept;
        retired
    }

    pub(super) fn evict_to(&mut self, capacity: usize) -> Vec<Cached> {
        let mut retired = Vec::new();
        while self.cache.len() > capacity {
            if let Some(cached) = self.cache.pop_front() {
                retired.push(cached);
            }
        }
        retired
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
