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
use std::sync::Arc;
// Third Party Imports

// Local Imports

use super::Manager;
#[cfg(feature = "plugin-guest")]
use super::manager::Entry;

#[cfg(feature = "plugin-guest")]
use crate::guest::host::plugin::PluginScope;

#[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
use crate::operations::RequestShape;

#[cfg(feature = "plugin-guest")]
use crate::Error;
use crate::{Result, operations::Lane};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type BindCatalog = dyn Fn(&Manager) -> Result<()> + Send + Sync;

/* === Structs === */

#[derive(Clone)]
pub struct Registration {
    pub(crate) batch: harmos_operations::execution::__private::Catalog,
    pub(crate) lane: Option<Lane>,
    pub(crate) defaults: std::collections::BTreeMap<&'static str, Lane>,
}

#[derive(Clone)]
pub(crate) struct CatalogBinding {
    bind: Arc<BindCatalog>,
    factories: Vec<harmos_operations::execution::operations::OperationFactory>,
    lanes: Vec<&'static str>,
}

#[cfg(feature = "plugin-guest")]
pub struct Bindings<'a> {
    pub(super) operations: &'a Manager,
    pub(super) owner: &'a str,

    #[cfg(feature = "plugin-guest")]
    pub(super) scope: Option<&'a PluginScope>,

    pub(super) entries: Vec<Arc<Entry>>,
}

#[must_use = "retain plugin operation registration until shutdown settles its invocations"]
#[cfg(feature = "plugin-guest")]
pub struct PluginRegistration {
    scope: Arc<PluginScope>,
    operations: Manager,
    entries: Vec<Arc<Entry>>,
}

/* === Enums === */

/* === Traits === */

pub(crate) trait RegistrationExt {
    fn lane(self, lane: Lane) -> Registration;
}

/* === Implementations === */

impl Registration {
    pub fn entry(mut self, operation: harmos_operations::execution::operations::OperationFactory) -> Self {
        self.batch.operations.push(operation);
        self
    }
    pub fn catalog(catalog: crate::operations::Catalog, lane: Lane) -> Result<Self> {
        let name = catalog.name;
        catalog.build().map(|batch| batch.lane(lane)).map_err(|source| crate::Error::OperationFailed {
            operation: name.into(),
            source: Arc::new(source),
        })
    }

    pub fn new(lane: Lane) -> Self {
        harmos_operations::execution::__private::Catalog::new().lane(lane)
    }

    #[cfg(feature = "grpc")]
    pub fn protobuf<O: harmos_operations::execution::operations::ProtobufOperation>(mut self) -> Result<Self> {
        let factory = harmos_operations::execution::operations::OperationFactory::protobuf::<O>().map_err(|source| {
            crate::Error::OperationFailed {
                operation: O::ID.into(),
                source: Arc::new(source),
            }
        })?;
        self.batch.operations.push(factory);
        Ok(self)
    }

    pub fn operation(mut self, operation: harmos_operations::execution::operations::OperationFactory, id: impl AsRef<str>) -> Result<Self> {
        let operation = operation.with_id(id.as_ref()).map_err(|source| crate::Error::OperationFailed {
            operation: id.as_ref().to_owned(),
            source: Arc::new(source),
        })?;
        self.batch.operations.push(operation);
        Ok(self)
    }
}

impl RegistrationExt for harmos_operations::execution::__private::Catalog {
    fn lane(self, lane: Lane) -> Registration {
        Registration {
            batch: self,
            lane: Some(lane),
            defaults: std::collections::BTreeMap::new(),
        }
    }
}

impl CatalogBinding {
    pub(crate) fn new(registration: Registration) -> Self {
        let factories = registration.batch.operations.clone();
        let lanes = factories
            .iter()
            .map(|factory| {
                registration
                    .defaults
                    .get(factory.definition.id.as_str())
                    .copied()
                    .or(registration.lane)
                    .unwrap_or_else(Lane::default)
                    .name()
            })
            .collect();
        Self {
            bind: Arc::new(move |operations| operations.register_native(registration.clone())),
            factories,
            lanes,
        }
    }

    pub(crate) fn validate(
        &self, resources: &harmos_operations::execution::__private::ResourceSet, lanes: &crate::operations::LaneSet,
    ) -> Vec<harmos_operations::execution::StartupError> {
        let provided = Manager::provided_resources();
        let mut errors = Vec::new();
        for factory in &self.factories {
            errors.extend(factory.missing_resources(resources, &provided));
        }
        for lane in &self.lanes {
            if !lanes.contains(lane) {
                errors.push(harmos_operations::execution::StartupError::UnknownLane { lane });
            }
        }
        errors
    }
    pub(crate) fn bind(&self, operations: &Manager) -> Result<()> {
        (self.bind)(operations)
    }
}

impl Manager {
    #[cfg(feature = "plugin-guest")]
    pub fn stage_plugin(&self, scope: &PluginScope, configure: impl FnOnce(&mut Bindings<'_>) -> Result<()>) -> Result<PluginRegistration> {
        if scope.is_closed() {
            return Err(Error::Closing { domain: "plugin scope" });
        }
        if scope.has_committed() {
            return Err(Error::InvalidInput {
                field: "operation catalog",
                reason: "plugin generation is already committed",
            });
        }
        let mut bindings = Bindings {
            operations: self,
            owner: scope.owner(),
            scope: Some(scope),
            entries: Vec::new(),
        };
        configure(&mut bindings)?;
        scope.validate_operations(bindings.entries.iter().map(|entry| entry.declared()))?;
        scope.validate_wire_contracts(bindings.entries.iter().filter_map(|entry| entry.contract.as_ref()))?;
        self.reserve(&bindings.entries)?;

        let receipt_scope = Arc::new(scope.clone());
        let staged = self
            .track_staging(&receipt_scope)
            .and_then(|_| self.validate_staging(&bindings.entries))
            .and_then(|_| scope.bind_operations())
            .and_then(|_| self.validate_staging(&bindings.entries));
        if let Err(error) = staged {
            scope.close_staging();
            self.fence(&bindings.entries, true);
            self.remove_settled(&bindings.entries);
            return Err(error);
        }
        Ok(PluginRegistration {
            scope: receipt_scope,
            operations: self.clone(),
            entries: bindings.entries,
        })
    }
}

#[cfg(feature = "plugin-guest")]
impl Bindings<'_> {
    pub fn catalog(&mut self, catalog: crate::operations::Catalog, lane: Lane) -> Result<()> {
        self.register(Registration::catalog(catalog, lane)?)
    }

    pub fn register(&mut self, registration: Registration) -> Result<()> {
        let entries = self.operations.native_entries(registration, self.owner, self.scope)?;
        self.entries.extend(entries);
        Ok(())
    }

    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    pub(super) fn stage_implementation<D, O>(
        &mut self, specification: harmos_operations::operations::OperationSpec<D>, lane: &'static str,
        mut dependencies: harmos_operations::execution::__private::ResourceSet,
    ) -> Result<()>
    where
        D: RequestShape,
        O: harmos_operations::execution::operations::Carrier<Declaration = D>,
    {
        let definition = specification.definition().map_err(|source| Error::OperationDeclaration { source })?;
        let contract = specification.declaration.contract_with_settings(&self.operations.schema_settings).map_err(|source| {
            Error::OperationFailed {
                operation: definition.id.to_string(),
                source: Arc::new(source),
            }
        })?;
        let wire = (contract.input().is_none() && contract.output().is_none())
            .then_some(crate::operations::wire::dispatch_native as crate::operations::wire::Dispatch);
        dependencies.insert(Arc::new(specification))?;
        let mut batch = harmos_operations::execution::__private::Catalog::new().carrier::<D, O>(specification).map_err(|source| {
            Error::OperationFailed {
                operation: definition.id.to_string(),
                source: Arc::new(source),
            }
        })?;
        let factory = batch.operations.pop().expect("one native implementation");
        let mut entry = self.operations.binding_erased(self.owner, lane, Arc::new(factory), definition, wire, self.scope.cloned())?;
        Arc::get_mut(&mut entry).expect("new native entry").carrier = dependencies;
        #[cfg(feature = "grpc")]
        if contract.grpc().is_some() {
            Arc::get_mut(&mut entry).expect("new native entry").grpc = Some(crate::grpc::routing::handler::<D>());
        }
        Arc::get_mut(&mut entry).expect("new native entry").contract = Some(contract);
        self.entries.push(entry);
        Ok(())
    }
}

#[cfg(feature = "plugin-guest")]
impl PluginRegistration {
    pub async fn shutdown(&mut self) -> Result<()> {
        self.scope.close();
        self.entries = self.operations.scope_entries(&self.scope);
        self.scope.unbind_operations();
        self.operations.fence(&self.entries, false);
        self.operations.settle_entries(&self.entries).await?;
        self.operations.resident.shutdown_plugin(&self.scope).await?;
        self.entries.clear();
        Ok(())
    }
}

#[cfg(feature = "plugin-guest")]
impl Drop for PluginRegistration {
    fn drop(&mut self) {
        self.scope.close();
        self.entries = self.operations.scope_entries(&self.scope);
        self.scope.unbind_operations();
        self.operations.fence(&self.entries, true);
        self.operations.remove_settled(&self.entries);
        // Running entries remain fenced and reserved in the manager. A later
        // reservation prunes settled tombstones; runtime shutdown drains all.
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
