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
use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// Third Party Imports

// Local Imports
use super::CallContext;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct Dependencies {
    entries: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

#[derive(Clone)]
pub struct ExecutionContext {
    dependencies: Dependencies,
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

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("resource `{name}` is already present")]
    Duplicate { name: &'static str },

    #[error("resource `{name}` is missing")]
    Missing { name: &'static str },
}

/* === Traits === */

/* === Implementations === */

impl Dependencies {
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

    pub fn contains(&self, resource: TypeId) -> bool {
        self.entries.contains_key(&resource)
    }
}

impl ExecutionContext {
    pub fn new(dependencies: Dependencies, call: CallContext) -> Self {
        Self { dependencies, call }
    }

    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ResourceError> {
        self.dependencies.resource::<T>()
    }

    pub fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    pub(crate) fn for_registration(&self, mut dependencies: Dependencies) -> Self {
        // Carrier Dependencies Retain Their Trusted Handles In Each Invocation Snapshot.
        Arc::make_mut(&mut dependencies.entries).extend(self.dependencies.entries.iter().map(|(key, value)| (*key, value.clone())));
        Self::new(dependencies, self.call.clone())
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_share_values_and_isolate_insertions() {
        let mut dependencies = Dependencies::new();
        let value = Arc::new(7u32);
        dependencies.insert(value.clone()).unwrap();
        let snapshot = dependencies.clone();
        dependencies.insert(Arc::new(9u64)).unwrap();
        assert!(Arc::ptr_eq(&snapshot.resource::<u32>().unwrap(), &value));
        assert!(matches!(snapshot.resource::<u64>(), Err(ResourceError::Missing { .. })));
        assert!(matches!(dependencies.insert(Arc::new(8u32)), Err(ResourceError::Duplicate { .. })));
        assert_eq!(*dependencies.resource::<u32>().unwrap(), 7);
    }
}
