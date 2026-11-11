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
pub struct ResourceSet {
    entries: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

#[derive(Clone, Default)]
pub struct ExecutionContext {
    resources: ResourceSet,
    pub call: CallContext,
    pub cancellation: super::Cancellation,
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
        Self {
            resources,
            call,
            cancellation: super::Cancellation::default(),
        }
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
        Self {
            resources,
            call: self.call.clone(),
            cancellation: self.cancellation.clone(),
        }
    }
}

impl AssignedOperation {
    pub fn encode(entries: &[Self]) -> Result<Vec<u8>, super::ServingError> {
        use prost::Message;
        let assignments = harmos_protocol::host::OperationAssignments {
            operations: entries
                .iter()
                .map(|entry| harmos_protocol::host::AssignedOperation {
                    operation: entry.operation.clone(),
                    fingerprint: entry.fingerprint.to_vec(),
                    generation: entry.generation,
                })
                .collect(),
        };
        Ok(assignments.encode_to_vec())
    }
}

impl OperationAssignments {
    pub fn install(&self, bytes: &[u8]) -> Result<(), super::ServingError> {
        use prost::Message;
        let assignments = harmos_protocol::host::OperationAssignments::decode(bytes).map_err(|_| super::ServingError::Catalog)?;
        let entries = assignments
            .operations
            .into_iter()
            .map(|entry| {
                Ok(AssignedOperation {
                    operation: entry.operation,
                    fingerprint: entry.fingerprint.try_into().map_err(|_| super::ServingError::Catalog)?,
                    generation: entry.generation,
                })
            })
            .collect::<Result<Vec<_>, super::ServingError>>()?;
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

    pub fn resolve_dependency(
        &self, id: &str, fingerprint: crate::execution::Fingerprint,
    ) -> Result<&AssignedOperation, super::ServingError> {
        let entry = self
            .entries
            .get()
            .and_then(|entries| entries.iter().find(|entry| entry.operation.split_once('.').is_some_and(|(_, local)| local == id)))
            .ok_or_else(|| super::ServingError::Unassigned { operation: id.into() })?;
        if entry.fingerprint != fingerprint.0 {
            return Err(super::ServingError::Identity { operation: entry.operation.clone() });
        }
        Ok(entry)
    }

    pub fn resolve_contract(&self, contract: &super::Contract) -> Result<&AssignedOperation, super::ServingError> {
        let id = contract.operation().id.as_str();
        let entry = self
            .entries
            .get()
            .and_then(|entries| entries.iter().find(|entry| entry.operation.split_once('.').is_some_and(|(_, local)| local == id)))
            .ok_or_else(|| super::ServingError::Unknown { operation: id.to_owned() })?;
        if entry.fingerprint != contract.fingerprint().0 {
            return Err(super::ServingError::Identity { operation: entry.operation.clone() });
        }
        Ok(entry)
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
            .ok_or(super::ServingError::Unassigned { operation: target.id.into() })?;
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
    fn protobuf_assignments_validate_and_freeze_the_catalog() {
        use prost::Message;
        let entry = AssignedOperation {
            operation: "host.catalog.read".into(),
            fingerprint: [7; 32],
            generation: 1,
        };
        let encoded = AssignedOperation::encode(std::slice::from_ref(&entry)).unwrap();
        let decoded = harmos_protocol::host::OperationAssignments::decode(encoded.as_slice()).unwrap();
        assert_eq!(decoded.operations[0].operation, entry.operation);
        let assignments = OperationAssignments::default();
        assignments.install(&encoded).unwrap();
        assignments.install(&encoded).unwrap();
        let replacement = AssignedOperation { generation: 2, ..entry.clone() };
        assert!(assignments.install(&AssignedOperation::encode(&[replacement]).unwrap()).is_err());
        assert!(OperationAssignments::default().install(&AssignedOperation::encode(&[entry.clone(), entry]).unwrap()).is_err());
        let mut invalid = decoded;
        invalid.operations[0].fingerprint.pop();
        assert!(OperationAssignments::default().install(&invalid.encode_to_vec()).is_err());
        assert!(OperationAssignments::default().install(&[0xff]).is_err());
    }

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
