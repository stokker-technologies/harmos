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
    pub(crate) fn provided_resources() -> Vec<std::any::TypeId> {
        vec![
            std::any::TypeId::of::<harmos_operations::execution::operations::Calls>(),
            std::any::TypeId::of::<harmos_operations::execution::operations::access::DiscoveryAccess>(),
            std::any::TypeId::of::<harmos_operations::execution::operations::access::GrantAccess>(),
            std::any::TypeId::of::<harmos_operations::execution::operations::instances::InstanceAccess>(),
            std::any::TypeId::of::<crate::operations::HostContext>(),
            std::any::TypeId::of::<harmos_operations::clock::ClockAccess>(),
        ]
    }

    pub(in crate::operations) fn register_with_services(
        &self, registration: crate::operations::Registration, staged: crate::operations::resident::Staged,
    ) -> Result<()> {
        self.register_scoped_services(
            registration,
            staged,
            "host",
            #[cfg(feature = "plugin-guest")]
            None,
        )
    }

    pub(in crate::operations) fn register_scoped_services(
        &self, registration: crate::operations::Registration, staged: crate::operations::resident::Staged, owner: &str,
        #[cfg(feature = "plugin-guest")] scope: Option<&PluginScope>,
    ) -> Result<()> {
        let mut entries = self.native_entries(
            registration,
            owner,
            #[cfg(feature = "plugin-guest")]
            scope,
        )?;
        let publication = staged.publication();
        for entry in &mut entries {
            Arc::get_mut(entry).expect("unpublished operation has one owner").publication = publication.clone();
        }
        self.reserve(&entries)?;
        if let Err(error) = staged.commit() {
            let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
            for entry in &entries {
                if state.owned(entry.id(), entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, entry)) {
                    state.remove_owned(entry.id(), entry.owner.as_str());
                }
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn register_native(&self, registration: crate::operations::Registration) -> Result<()> {
        let entries = self.native_entries(
            registration,
            "host",
            #[cfg(feature = "plugin-guest")]
            None,
        )?;
        self.reserve(&entries)
    }

    pub(in crate::operations) fn native_entries(
        &self, registration: crate::operations::Registration, owner: &str, #[cfg(feature = "plugin-guest")] scope: Option<&PluginScope>,
    ) -> Result<Vec<Arc<Entry>>> {
        let crate::operations::Registration { batch, lane, defaults } = registration;
        if !batch.services.is_empty() {
            return Err(Error::InvalidInput {
                field: "operation registration",
                reason: "services belong to the service manager",
            });
        }
        let mut entries = Vec::with_capacity(batch.operations.len());
        for factory in batch.operations {
            let provided = Self::provided_resources();
            factory.verify_resources(self.resources(), &provided).map_err(|source| Error::OperationFailed {
                operation: factory.definition.id.to_string(),
                source: Arc::new(source),
            })?;
            factory.validate().map_err(|source| match source {
                harmos_operations::execution::ServingError::Declaration(source) => Error::OperationDeclaration { source },
                harmos_operations::execution::ServingError::EndpointTypes => Error::InvalidInput {
                    field: "operation endpoint types",
                    reason: "endpoints differ from the declaration",
                },
                source => Error::OperationFailed {
                    operation: factory.definition.id.to_string(),
                    source: Arc::new(source),
                },
            })?;
            let contract = match factory.contract_with_settings(&self.schema_settings) {
                Ok(contract) => Some(contract),
                Err(crate::operations::ContractError::Codec { .. }) => None,
                Err(source) => {
                    return Err(Error::OperationFailed {
                        operation: factory.definition.id.to_string(),
                        source: Arc::new(source),
                    });
                }
            };
            let selected = defaults.get(factory.definition.id.as_str()).copied().or(lane).unwrap_or_else(Lane::default);
            let definition = factory.definition.clone();
            let wire: Option<crate::operations::wire::Dispatch> =
                if contract.as_ref().is_some_and(|contract| contract.input().is_none() && contract.output().is_none()) {
                    Some(crate::operations::wire::dispatch_native)
                } else {
                    None
                };
            let mut entry = self.binding_erased(
                owner,
                selected.name(),
                Arc::new(factory),
                definition,
                wire,
                #[cfg(feature = "plugin-guest")]
                scope.cloned(),
            )?;
            #[cfg(feature = "grpc")]
            if contract.as_ref().is_some_and(|contract| contract.grpc().is_some()) {
                Arc::get_mut(&mut entry).expect("unpublished entry has one owner").grpc = Some(crate::grpc::native::serve);
            }
            Arc::get_mut(&mut entry).expect("unpublished entry has one owner").contract = contract;
            entries.push(entry);
        }
        Ok(entries)
    }
    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    pub(in crate::operations) fn bind_native<D, O>(
        &self, owner: &str, lane: &'static str, dependencies: harmos_operations::execution::__private::ResourceSet,
        definition: OperationDefinition, wire: Option<crate::operations::wire::Dispatch>, plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>>
    where
        D: RequestShape,
        O: harmos_operations::execution::operations::Carrier<Declaration = D>,
    {
        let resources = harmos_operations::execution::ExecutionContext::new(dependencies, super::super::CallContext::default())
            .with_resources(self.resources().clone())
            .resources()
            .clone();
        let binding = crate::operations::authored::factory_with::<D, O::StreamEndpoints>(self, move |request, endpoints| {
            Ok(harmos_operations::execution::operations::OperationFactory::prepare_carrier::<O>(resources.clone(), request, endpoints))
        });
        self.binding_erased(owner, lane, Arc::new(binding), definition, wire, plugin)
    }

    pub(in crate::operations) fn binding_erased(
        &self, owner: impl Into<String>, lane: &'static str, binding: Arc<dyn Any + Send + Sync>, definition: OperationDefinition,
        wire: Option<crate::operations::wire::Dispatch>, #[cfg(feature = "plugin-guest")] plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>> {
        let owner = Identifier::new(owner.into()).map_err(|source| Error::OperationDeclaration { source: source.into() })?;
        let valid_owner = harmos_domain::identity::segment(owner.as_str());
        let valid_owner = valid_owner
            || (binding.is::<crate::plugin::grpc::invoke::Binding>() && owner.as_str().split('.').all(harmos_domain::identity::segment));
        if !valid_owner {
            return Err(Error::InvalidInput {
                field: "operation owner",
                reason: "owner must be a lowercase portable segment",
            });
        }
        let _bounded = self.instances.lane_bounded(lane).ok_or_else(|| Error::NotRegistered {
            domain: "operation lane",
            name: lane.to_owned(),
        })?;
        let generation = OperationGeneration::fresh()?;
        Ok(Arc::new(Entry {
            carrier: harmos_operations::execution::__private::ResourceSet::new(),
            owner,
            definition,
            contract: None,
            #[cfg(feature = "grpc")]
            grpc: None,
            generation,
            lane,

            active: AtomicBool::new(true),
            publication: Arc::new(AtomicBool::new(true)),
            retirement: AtomicU8::new(Retirement::Retained as u8),
            pending: Mutex::new(BTreeMap::new()),
            binding,

            wire,

            #[cfg(feature = "plugin-guest")]
            plugin,
            #[cfg(feature = "plugin-guest")]
            sdk: None,
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
