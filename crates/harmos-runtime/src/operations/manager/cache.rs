/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl Manager {
    pub fn set_cache_capacity(&self, capacity: usize) {
        let retired = {
            let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
            state.cache_capacity = capacity;
            state.evict_to(capacity)
        };
        drop(retired);
    }

    pub fn clear_cache(&self) {
        let retired = std::mem::take(&mut self.registry.state.lock().expect("operation registry lock is healthy").cache);
        drop(retired);
    }

    pub fn invalidate<O: OperationDeclaration>(&self, key: &CacheKey) -> Result<()> {
        let entry = self.entry(O::ID)?;
        self.invalidate_entry(&entry, key);
        Ok(())
    }

    pub fn invalidate_from<O: OperationDeclaration>(&self, owner: &str, key: &CacheKey) -> Result<()> {
        let entry = self.capture_provider(O::ID, Some(owner))?;
        entry.available()?;
        self.invalidate_entry(&entry, key);
        Ok(())
    }

    pub(super) fn invalidate_entry(&self, entry: &Arc<Entry>, key: &CacheKey) {
        let retired = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .remove_cache(|cached| cached.key == *key && cached.target.upgrade().is_some_and(|target| Arc::ptr_eq(&target, entry)));
        drop(retired);
    }

    pub(in crate::operations) fn lookup_erased(
        &self, entry: &Arc<Entry>, key: &CacheKey, identity: &Identity,
    ) -> Option<harmos_sdk::__private::ErasedResponse> {
        let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || entry.available().is_err() {
            return None;
        }
        let index = state.cache.iter().position(|cached| {
            cached.key == *key
                && cached.identity == *identity
                && cached.target.upgrade().is_some_and(|target| Arc::ptr_eq(&target, entry) && target.generation == entry.generation)
        })?;
        let cached = state.cache.remove(index)?;
        let response = Some(cached.response.clone());
        state.cache.push_back(cached);
        response
    }
}
