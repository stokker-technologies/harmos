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
use super::manager::Entry;
use super::{Binding, Implementations, WireImplementations};
use super::{Context, OperationManager};
use crate::catalog::{Catalog, Projection, Visitor};

#[cfg(any(
    feature = "plugin-sidecar",
    all(feature = "plugin-guest", not(target_arch = "wasm32"))
))]
use crate::operations::Declarations;
use crate::operations::{
    AuthoredDeclarations, Definition, OperationDeclaration, WireDeclaration, WireOperation,
};
#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
use crate::plugin::PluginScope;
use crate::schema::Described;
use crate::{Error, Message, Result, operations::Lane};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type BindCatalog = dyn Fn(&OperationManager) -> Result<()> + Send + Sync;

/* === Structs === */
struct WireProjection {
    declaration: std::any::TypeId,
    contract: crate::operations::Contract,
    dispatch: Option<super::wire::Dispatch>,
    #[cfg(feature = "grpc")]
    grpc: Option<crate::grpc::routing::Handler>,
}

struct ProjectWire;

#[derive(Clone)]
pub(crate) struct CatalogBinding {
    bind: Arc<BindCatalog>,
}

pub struct Bindings<'a> {
    pub(super) operations: &'a OperationManager,
    owner: &'a str,
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) scope: Option<&'a PluginScope>,
    pub(super) entries: Vec<Arc<Entry>>,
}

#[must_use = "retain plugin operation registration until shutdown settles its invocations"]
#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
pub struct PluginRegistration {
    scope: Arc<PluginScope>,
    operations: OperationManager,
    entries: Vec<Arc<Entry>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl super::WireVisitor for ProjectWire {
    type Entry = Result<WireProjection>;

    fn wire<O>(self) -> Self::Entry
    where
        O: WireOperation,
        O::Request: Message + Described,
        O::Response: Message + Described,
    {
        let contract =
            WireDeclaration::of::<O>()
                .contract()
                .map_err(|source| Error::OperationFailed {
                    operation: O::ID.to_owned(),
                    source: Arc::new(source),
                })?;
        let streaming = contract.input().is_some() || contract.output().is_some();
        #[cfg(feature = "grpc")]
        let grpc = O::visit_grpc(crate::grpc::routing::Project);
        #[cfg(feature = "grpc")]
        if contract.grpc().is_some() && grpc.is_none() {
            return Err(Error::InvalidInput {
                field: "operation gRPC binding",
                reason: "declaration has no typed gRPC adapter",
            });
        }
        Ok(WireProjection {
            declaration: std::any::TypeId::of::<O>(),
            contract,
            dispatch: if streaming {
                None
            } else {
                Some(super::wire::dispatch::<O>)
            },
            #[cfg(feature = "grpc")]
            grpc,
        })
    }
}

impl CatalogBinding {
    pub(crate) fn operation<T>(owner: String, lane: impl Lane, resources: Arc<T::Resources>) -> Self
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        let lane = lane.name();
        Self {
            bind: Arc::new(move |operations| {
                operations.register_operation_at::<T>(owner.clone(), lane, resources.clone())
            }),
        }
    }

    pub(crate) fn local<C, R>(owner: String, lane: impl Lane, resources: Arc<R>) -> Self
    where
        C: Catalog<AuthoredDeclarations> + Catalog<Implementations<R>>,
        R: Send + Sync + 'static,
    {
        let lane = lane.name();
        Self {
            bind: Arc::new(move |operations| {
                operations.register_with(owner.clone(), |bindings| {
                    bindings
                        .project_owned_at::<C, Implementations<R>, R>(lane, Arc::clone(&resources))
                })
            }),
        }
    }

    pub(crate) fn wire<C, R>(owner: String, lane: impl Lane, resources: Arc<R>) -> Self
    where
        C: Catalog<AuthoredDeclarations> + Catalog<WireImplementations<R>>,
        R: Send + Sync + 'static,
    {
        let lane = lane.name();
        Self {
            bind: Arc::new(move |operations| {
                operations.register_with(owner.clone(), |bindings| {
                    bindings.project_owned_at::<C, WireImplementations<R>, R>(
                        lane,
                        Arc::clone(&resources),
                    )
                })
            }),
        }
    }

    pub(crate) fn bind(&self, operations: &OperationManager) -> Result<()> {
        (self.bind)(operations)
    }
}

impl OperationManager {
    pub fn register_operations<C, R>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<R>,
    ) -> Result<()>
    where
        R: Send + Sync + 'static,
        C: Catalog<AuthoredDeclarations> + Catalog<Implementations<R>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_operations::<C, R>(lane, resources)
        })
    }

    pub fn register_wire_operations<C, R>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<R>,
    ) -> Result<()>
    where
        R: Send + Sync + 'static,
        C: Catalog<AuthoredDeclarations> + Catalog<WireImplementations<R>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_wire_operations::<C, R>(lane, resources)
        })
    }

    pub(super) fn register_with(
        &self,
        owner: impl Into<String>,
        configure: impl FnOnce(&mut Bindings<'_>) -> Result<()>,
    ) -> Result<()> {
        let owner = owner.into();
        let mut bindings = Bindings {
            operations: self,
            owner: &owner,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            scope: None,
            entries: Vec::new(),
        };
        configure(&mut bindings)?;
        self.reserve(&bindings.entries)
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn stage_plugin(
        &self,
        scope: &PluginScope,
        configure: impl FnOnce(&mut Bindings<'_>) -> Result<()>,
    ) -> Result<PluginRegistration> {
        if scope.is_closed() {
            return Err(Error::Closing {
                domain: "plugin scope",
            });
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
        scope.validate_wire_contracts(
            bindings
                .entries
                .iter()
                .filter_map(|entry| entry.contract.as_ref()),
        )?;
        self.reserve(&bindings.entries)?;
        #[cfg(feature = "plugin-sidecar")]
        for entry in &bindings.entries {
            if let Some(native) = &entry.native
                && let Err(error) = native.capture_observer_controls()
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

impl Bindings<'_> {
    pub fn register_operations<C, R>(&mut self, lane: impl Lane, resources: Arc<R>) -> Result<()>
    where
        R: Send + Sync + 'static,
        C: Catalog<AuthoredDeclarations> + Catalog<Implementations<R>>,
    {
        self.project_owned_at::<C, Implementations<R>, R>(lane.name(), resources)
    }

    pub fn register_wire_operations<C, R>(
        &mut self,
        lane: impl Lane,
        resources: Arc<R>,
    ) -> Result<()>
    where
        R: Send + Sync + 'static,
        C: Catalog<AuthoredDeclarations> + Catalog<WireImplementations<R>>,
    {
        self.project_owned_at::<C, WireImplementations<R>, R>(lane.name(), resources)
    }

    pub fn register_operation<T>(
        &mut self,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_owned_at::<T>(lane.name(), resources)
    }

    pub(super) fn register_owned_at<T>(
        &mut self,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_operation_id_at::<T>(T::Declaration::ID, lane, resources)
    }

    pub fn register_operation_id<T>(
        &mut self,
        id: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_operation_id_at::<T>(id, lane.name(), resources)
    }

    fn register_operation_id_at<T>(
        &mut self,
        id: impl Into<String>,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        let mut definition = Definition::of::<T::Declaration>()
            .map_err(|source| Error::OperationDeclaration { source })?;
        definition.id =
            crate::Identifier::new(id.into()).map_err(|source| Error::OperationDeclaration {
                source: source.into(),
            })?;
        let id = definition.id.as_str();
        if self.entries.iter().any(|entry| entry.id() == id) {
            return Err(Error::AlreadyRegistered {
                domain: "operation",
                name: id.to_owned(),
            });
        }
        let projected = T::Declaration::visit_wire(ProjectWire).transpose()?;
        if let Some(projected) = &projected
            && (projected.declaration != std::any::TypeId::of::<T::Declaration>()
                || projected.contract.operation() != &definition)
        {
            return Err(Error::InvalidInput {
                field: "operation wire declaration",
                reason: "wire projection must match the original typed declaration and identity",
            });
        }
        let mut entry = self.operations.binding_owned_definition::<T>(
            self.owner,
            lane,
            resources,
            definition,
            projected.as_ref().and_then(|wire| wire.dispatch),
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            self.scope.cloned(),
        )?;
        if let Some(projected) = projected {
            let original = Arc::get_mut(&mut entry).expect("new owned entry has one owner");
            #[cfg(feature = "plugin-sidecar")]
            {
                original.observer_stop = self
                    .scope
                    .and_then(|scope| scope.observer_controls())
                    .is_some_and(|pair| pair[1] == projected.contract);
            }
            original.contract = Some(projected.contract);
            #[cfg(feature = "grpc")]
            {
                original.grpc = projected.grpc;
            }
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn register_wire_operation<T>(
        &mut self,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: WireOperation,
        <T::Declaration as OperationDeclaration>::Request: Message + Described,
        <T::Declaration as OperationDeclaration>::Response: Message + Described,
    {
        self.register_owned_wire_at::<T>(lane.name(), resources)
    }

    pub(super) fn register_owned_wire_at<T>(
        &mut self,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: WireOperation,
        <T::Declaration as OperationDeclaration>::Request: Message + Described,
        <T::Declaration as OperationDeclaration>::Response: Message + Described,
    {
        let contract = WireDeclaration::of::<T::Declaration>()
            .contract()
            .map_err(|source| Error::OperationFailed {
                operation: T::Declaration::ID.to_owned(),
                source: Arc::new(source),
            })?;
        self.register_wire_contract_at::<T>(&contract, lane, resources)
    }

    pub fn register_wire_contract<T>(
        &mut self,
        contract: &crate::operations::Contract,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: WireOperation,
        <T::Declaration as OperationDeclaration>::Request: Message + Described,
        <T::Declaration as OperationDeclaration>::Response: Message + Described,
    {
        self.register_wire_contract_at::<T>(contract, lane.name(), resources)
    }

    fn register_wire_contract_at<T>(
        &mut self,
        contract: &crate::operations::Contract,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: WireOperation,
        <T::Declaration as OperationDeclaration>::Request: Message + Described,
        <T::Declaration as OperationDeclaration>::Response: Message + Described,
    {
        let projected = super::WireVisitor::wire::<T::Declaration>(ProjectWire)?;
        let declared = &projected.contract;
        let mut definition = declared.operation().clone();
        definition.id = contract.operation().id.clone();
        if definition != *contract.operation()
            || declared.version() != contract.version()
            || declared.codec() != contract.codec()
            || ![
                (Some(declared.request()), Some(contract.request())),
                (Some(declared.response()), Some(contract.response())),
                (declared.input(), contract.input()),
                (declared.output(), contract.output()),
            ]
            .into_iter()
            .all(|(left, right)| match (left, right) {
                (None, None) => true,
                (Some(left), Some(right)) => left
                    .schema()
                    .equivalent(
                        left.index(),
                        right.schema(),
                        right.index(),
                        &harmos_contract::config::SchemaSettings::default(),
                    )
                    .unwrap_or(false),
                _ => false,
            })
            || declared.grpc() != contract.grpc()
        {
            return Err(Error::InvalidInput {
                field: "operation contract",
                reason: "runtime binding may override identity only",
            });
        }
        let id = contract.operation().id.as_str();
        if self.entries.iter().any(|entry| entry.id() == id) {
            return Err(Error::AlreadyRegistered {
                domain: "operation",
                name: id.to_owned(),
            });
        }
        let mut entry = self.operations.binding_owned_definition::<T>(
            self.owner,
            lane,
            resources,
            contract.operation().clone(),
            projected.dispatch,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            self.scope.cloned(),
        )?;
        let original = Arc::get_mut(&mut entry).expect("new owned entry has one owner");
        #[cfg(feature = "plugin-sidecar")]
        {
            original.observer_stop = self
                .scope
                .and_then(|scope| scope.observer_controls())
                .is_some_and(|pair| pair[1] == *contract);
        }
        original.contract = Some(contract.clone());
        #[cfg(feature = "grpc")]
        {
            original.grpc = projected.grpc;
        }
        self.entries.push(entry);
        Ok(())
    }

    #[cfg(any(
        feature = "plugin-sidecar",
        all(feature = "plugin-guest", not(target_arch = "wasm32"))
    ))]
    pub(super) fn project<C, V, H>(&mut self, lane: impl Lane, handler: Arc<H>) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<V>,
        V: Visitor<Entry = Binding<H>>,
        H: Send + Sync + 'static,
    {
        self.project_at::<C, V, H>(lane.name(), handler)
    }

    #[cfg(any(
        feature = "plugin-sidecar",
        all(feature = "plugin-guest", not(target_arch = "wasm32"))
    ))]
    fn project_at<C, V, H>(&mut self, lane: &'static str, handler: Arc<H>) -> Result<()>
    where
        C: Catalog<Declarations> + Catalog<V>,
        V: Visitor<Entry = Binding<H>>,
        H: Send + Sync + 'static,
    {
        let definitions =
            Declarations::of::<C>().map_err(|source| Error::OperationDeclaration { source })?;
        self.apply_projection(lane, handler, &definitions, &<C as Catalog<V>>::ENTRIES)
    }

    fn project_owned_at<C, V, R>(&mut self, lane: &'static str, resources: Arc<R>) -> Result<()>
    where
        C: Catalog<AuthoredDeclarations> + Catalog<V>,
        V: Visitor<Entry = Binding<R>>,
        R: Send + Sync + 'static,
    {
        let definitions = AuthoredDeclarations::of::<C>()
            .map_err(|source| Error::OperationDeclaration { source })?;
        self.apply_projection(lane, resources, &definitions, &<C as Catalog<V>>::ENTRIES)
    }

    fn apply_projection<H: Send + Sync + 'static>(
        &mut self,
        lane: &'static str,
        handler: Arc<H>,
        definitions: &[Definition],
        entries: &impl Projection<Binding<H>>,
    ) -> Result<()> {
        let previous = self.entries.len();
        if let Err(error) =
            entries.try_for_each(|entry| entry.apply(self, lane, Arc::clone(&handler)))
        {
            self.entries.truncate(previous);
            return Err(error);
        }
        let projected = &self.entries[previous..];
        if projected.len() != definitions.len()
            || projected
                .iter()
                .zip(definitions)
                .any(|(entry, declared)| entry.declared() != declared)
        {
            self.entries.truncate(previous);
            return Err(Error::InvalidInput {
                field: "operation catalog",
                reason: "metadata and handler projections differ",
            });
        }
        Ok(())
    }
}

#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
impl PluginRegistration {
    pub async fn shutdown(&mut self) -> Result<()> {
        self.scope.unbind_operations();
        self.operations.fence(&self.entries, false);
        self.operations.settle_entries(&self.entries).await?;
        self.entries.clear();
        Ok(())
    }
}

#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
impl Drop for PluginRegistration {
    fn drop(&mut self) {
        self.scope.close_staging();
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
