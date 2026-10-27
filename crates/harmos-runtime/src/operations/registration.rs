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
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
use super::Binding;
use super::Manager;
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use super::manager::Entry;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
use crate::catalog::Projection;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
use crate::catalog::{Catalog, Visitor};

#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
use crate::operations::Declarations;
use crate::operations::OperationDeclaration;
#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
use crate::operations::OperationDefinition;
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::plugin::PluginScope;

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::Error;
use crate::{Result, operations::Lane};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type BindCatalog = dyn Fn(&Manager) -> Result<()> + Send + Sync;

/* === Structs === */

#[derive(Clone)]
pub struct Registration<L: Lane> {
    pub(crate) batch: harmos_sdk::RegistrationBatch,
    pub(crate) lane: Option<L>,
    pub(crate) defaults: std::collections::BTreeMap<&'static str, L>,
}

#[derive(Clone)]
pub(crate) struct CatalogBinding {
    bind: Arc<BindCatalog>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub struct Bindings<'a> {
    pub(super) operations: &'a Manager,
    owner: &'a str,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) scope: Option<&'a PluginScope>,

    pub(super) entries: Vec<Arc<Entry>>,
}

#[must_use = "retain plugin operation registration until shutdown settles its invocations"]
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub struct PluginRegistration {
    scope: Arc<PluginScope>,
    operations: Manager,
    entries: Vec<Arc<Entry>>,
}

/* === Enums === */

/* === Traits === */

pub trait RegistrationExt {
    fn lane<L: Lane>(self, lane: L) -> Registration<L>;
    fn defaults<L: Lane>(self) -> Registration<L>;
}

/* === Implementations === */

impl RegistrationExt for harmos_sdk::RegistrationBatch {
    fn lane<L: Lane>(self, lane: L) -> Registration<L> {
        Registration {
            batch: self,
            lane: Some(lane),
            defaults: std::collections::BTreeMap::new(),
        }
    }

    fn defaults<L: Lane>(self) -> Registration<L> {
        Registration {
            batch: self,
            lane: None,
            defaults: std::collections::BTreeMap::new(),
        }
    }
}

impl<L: Lane> Registration<L> {
    pub fn default_lane_for<O: OperationDeclaration>(mut self, lane: L) -> Self {
        self.defaults.insert(O::ID, lane);
        self
    }
}

impl CatalogBinding {
    pub(crate) fn new<L: Lane>(registration: Registration<L>) -> Self {
        Self {
            bind: Arc::new(move |operations| operations.register_native(registration.clone())),
        }
    }
    pub(crate) fn bind(&self, operations: &Manager) -> Result<()> {
        (self.bind)(operations)
    }
}

impl Manager {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        #[cfg(feature = "plugin-sidecar")]
        for entry in &bindings.entries {
            if let Some(native) = &entry.native
                && let Err(error) = native.capture_observe_contract()
            {
                self.fence(&bindings.entries, true);
                self.remove_settled(&bindings.entries);
                return Err(error);
            }
        }
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

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl Bindings<'_> {
    pub fn register<L: Lane>(&mut self, registration: Registration<L>) -> Result<()> {
        let entries = self.operations.native_entries(registration, self.owner, self.scope)?;
        self.entries.extend(entries);
        Ok(())
    }

    #[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
    pub(super) fn stage_implementation<D, O>(&mut self, lane: &'static str, dependencies: harmos_sdk::Dependencies) -> Result<()>
    where
        D: OperationDeclaration,
        O: harmos_sdk::operations::Carrier<Declaration = D>,
    {
        let contract = harmos_sdk::operations::WireDeclaration::of::<D>()
            .contract_with_settings(&self.operations.schema_settings)
            .map_err(|source| Error::OperationFailed {
                operation: D::ID.to_owned(),
                source: Arc::new(source),
            })?;
        let wire = (contract.input().is_none() && contract.output().is_none())
            .then_some(crate::operations::wire::dispatch_native as crate::operations::wire::Dispatch);
        let mut batch = harmos_sdk::RegistrationBatch::new(dependencies).carrier::<D, O>();
        let factory = batch.operations.pop().expect("one native implementation");
        let mut entry = self.operations.binding_erased(self.owner, lane, Arc::new(factory), D::definition(), wire, self.scope.cloned())?;
        #[cfg(feature = "grpc")]
        if contract.grpc().is_some() {
            Arc::get_mut(&mut entry).expect("new native entry").grpc = Some(crate::grpc::routing::handler::<D>());
        }
        Arc::get_mut(&mut entry).expect("new native entry").contract = Some(contract);
        self.entries.push(entry);
        Ok(())
    }
}

#[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
impl Bindings<'_> {
    #[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
    pub(super) fn project<C, V, H>(&mut self, lane: impl Lane, handler: Arc<H>) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<V>,
        V: Visitor<Entry = Binding<H>>,
        H: Send + Sync + 'static,
    {
        self.project_at::<C, V, H>(lane.name(), handler)
    }

    #[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
    fn project_at<C, V, H>(&mut self, lane: &'static str, handler: Arc<H>) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<V>,
        V: Visitor<Entry = Binding<H>>,
        H: Send + Sync + 'static,
    {
        let definitions = Declarations::of::<C>().map_err(|source| Error::OperationDeclaration { source })?;
        self.apply_projection(lane, handler, &definitions, &<C as Catalog<V>>::ENTRIES)
    }

    fn apply_projection<H: Send + Sync + 'static>(
        &mut self, lane: &'static str, handler: Arc<H>, definitions: &[OperationDefinition], entries: &impl Projection<Binding<H>>,
    ) -> Result<()> {
        let previous = self.entries.len();
        if let Err(error) = entries.try_for_each(|entry| entry.apply(self, lane, Arc::clone(&handler))) {
            self.entries.truncate(previous);
            return Err(error);
        }
        let projected = &self.entries[previous..];
        if projected.len() != definitions.len() || projected.iter().zip(definitions).any(|(entry, declared)| entry.declared() != declared) {
            self.entries.truncate(previous);
            return Err(Error::InvalidInput {
                field: "operation catalog",
                reason: "metadata and handler projections differ",
            });
        }
        Ok(())
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
