/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::CallContext;

#[derive(Clone, Default)]
pub struct ResourceSet {
    entries: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

#[derive(Clone, Default)]
pub struct ExecutionContext {
    resources: ResourceSet,
    pub call: CallContext,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssignedOperation {
    pub operation: String,
    pub fingerprint: [u8; 32],
    pub generation: u64,
}

#[derive(Clone, Default)]
pub struct OperationAssignments {
    entries: Arc<OnceLock<Vec<AssignedOperation>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("resource `{name}` is already present")]
    Duplicate { name: &'static str },

    #[error("resource `{name}` is missing")]
    Missing { name: &'static str },
}

impl ResourceSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Result<(), ResourceError> {
        let key = TypeId::of::<T>();
        if self.entries.contains_key(&key) {
            return Err(ResourceError::Duplicate { name: type_name::<T>() });
        }
        Arc::make_mut(&mut self.entries).insert(key, value);
        Ok(())
    }

    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ResourceError> {
        self.entries
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|entry| entry.downcast().ok())
            .ok_or(ResourceError::Missing { name: type_name::<T>() })
    }

    pub fn replace<T: Send + Sync + 'static>(&mut self, value: Arc<T>) {
        Arc::make_mut(&mut self.entries).insert(TypeId::of::<T>(), value);
    }

    pub fn contains(&self, resource: TypeId) -> bool {
        self.entries.contains_key(&resource)
    }
}

impl ExecutionContext {
    #[doc(hidden)]
    pub fn new(resources: ResourceSet, call: CallContext) -> Self {
        Self { resources, call }
    }

    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ResourceError> {
        self.resources.resource::<T>()
    }

    #[doc(hidden)]
    pub fn resources(&self) -> &ResourceSet {
        &self.resources
    }

    #[doc(hidden)]
    pub fn with_resources(&self, mut resources: ResourceSet) -> Self {
        // Trusted Carrier Handles Retain Priority In Each Invocation Snapshot.
        Arc::make_mut(&mut resources.entries).extend(self.resources.entries.iter().map(|(key, value)| (*key, value.clone())));
        Self::new(resources, self.call.clone())
    }
}

impl AssignedOperation {
    pub fn encode(entries: &[Self]) -> Result<Vec<u8>, super::ServingError> {
        postcard::to_allocvec(entries).map_err(|_| super::ServingError::Catalog)
    }
}

impl OperationAssignments {
    pub fn install(&self, bytes: &[u8]) -> Result<(), super::ServingError> {
        let entries: Vec<AssignedOperation> =
            if bytes.is_empty() { Vec::new() } else { postcard::from_bytes(bytes).map_err(|_| super::ServingError::Catalog)? };
        let mut locals = std::collections::BTreeSet::new();
        for entry in &entries {
            harmos_domain::QualifiedId::parse(&entry.operation).map_err(|_| super::ServingError::Catalog)?;
            let (_, local) = entry.operation.split_once('.').ok_or(super::ServingError::Catalog)?;
            if entry.generation == 0 || !locals.insert(local) {
                return Err(super::ServingError::Catalog);
            }
        }
        if let Some(original) = self.entries.get() {
            return if original == &entries { Ok(()) } else { Err(super::ServingError::Catalog) };
        }
        self.entries.set(entries).map_err(|_| super::ServingError::Catalog)
    }

    pub fn resolve<O: super::OperationDeclaration>(&self) -> Result<&AssignedOperation, super::ServingError> {
        self.resolve_target(&super::Target::of::<O>())
    }

    pub fn resolve_target(&self, target: &super::Target) -> Result<&AssignedOperation, super::ServingError> {
        let local = |entry: &&AssignedOperation| entry.operation.split_once('.').is_some_and(|(_, local)| local == target.id);
        let entry = self
            .entries
            .get()
            .and_then(|entries| entries.iter().find(local))
            .ok_or(super::ServingError::Unassigned { operation: target.id })?;
        if entry.fingerprint != (target.definition)().fingerprint.0 {
            return Err(super::ServingError::Identity { operation: entry.operation.clone() });
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_resource_snapshots() {
        let mut resources = ResourceSet::new();
        let value = Arc::new(7u32);
        resources.insert(value.clone()).unwrap();
        let snapshot = resources.clone();
        resources.insert(Arc::new(9u64)).unwrap();
        assert!(Arc::ptr_eq(&snapshot.resource::<u32>().unwrap(), &value));
        assert!(matches!(snapshot.resource::<u64>(), Err(ResourceError::Missing { .. })));
        assert!(matches!(resources.insert(Arc::new(8u32)), Err(ResourceError::Duplicate { .. })));
        assert_eq!(*resources.resource::<u32>().unwrap(), 7);
    }
}
