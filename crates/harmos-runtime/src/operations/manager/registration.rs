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

impl OperationManager {
    pub(crate) fn register_native<L: Lane>(&self, registration: crate::operations::Registration<L>) -> Result<()> {
        let entries = self.native_entries(
            registration,
            "host",
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            None,
        )?;
        self.reserve(&entries)
    }

    pub(in crate::operations) fn native_entries<L: Lane>(
        &self, registration: crate::operations::Registration<L>, owner: &str,
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))] scope: Option<&PluginScope>,
    ) -> Result<Vec<Arc<Entry>>> {
        let crate::operations::Registration { batch, lane, defaults } = registration;
        if !batch.services.is_empty() {
            return Err(Error::InvalidInput { field: "operation registration", reason: "services belong to the service manager" });
        }
        let mut entries = Vec::with_capacity(batch.operations.len());
        for factory in batch.operations {
            factory
                .validate()
                .map_err(|source| Error::OperationFailed { operation: factory.definition.id.to_string(), source: Arc::new(source) })?;
            let contract = match factory.contract() {
                Ok(contract) => Some(contract),
                Err(crate::operations::ContractError::Codec { .. }) => None,
                Err(source) => {
                    return Err(Error::OperationFailed { operation: factory.definition.id.to_string(), source: Arc::new(source) });
                }
            };
            let selected = defaults.get(factory.definition.id.as_str()).copied().or(lane).unwrap_or_else(L::default_lane);
            let definition = factory.definition.clone();
            definition.validate().map_err(|source| Error::OperationDeclaration { source })?;
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
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                scope.cloned(),
            )?;
            Arc::get_mut(&mut entry).expect("unpublished entry has one owner").contract = contract;
            entries.push(entry);
        }
        Ok(entries)
    }
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(in crate::operations) fn bind_native<D, O>(
        &self, owner: &str, lane: &'static str, dependencies: harmos_sdk::Dependencies, definition: OperationDefinition,
        wire: Option<crate::operations::wire::Dispatch>, plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>>
    where
        D: OperationDeclaration,
        O: harmos_sdk::Operation<Request = D::Request, Response = D::Response>,
    {
        let mut batch = harmos_sdk::RegistrationBatch::new(dependencies).implementation::<D, O>();
        let factory = batch.operations.pop().expect("one native implementation");
        let binding = crate::operations::authored::factory::<D, O::Endpoints>(self, factory);
        self.binding_erased(owner, lane, Arc::new(binding), definition, wire, plugin)
    }

    pub(in crate::operations) fn binding_erased(
        &self, owner: impl Into<String>, lane: &'static str, binding: Arc<dyn Any + Send + Sync>, definition: OperationDefinition,
        wire: Option<crate::operations::wire::Dispatch>,
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))] plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>> {
        let owner = Identifier::new(owner.into()).map_err(|source| Error::OperationDeclaration { source: source.into() })?;
        let _bounded =
            self.instances.lane_bounded(lane).ok_or_else(|| Error::NotRegistered { domain: "operation lane", name: lane.to_owned() })?;
        let generation = OperationGeneration::fresh()?;
        Ok(Arc::new(Entry {
            owner,
            definition,
            contract: None,
            #[cfg(feature = "grpc")]
            grpc: None,
            generation,
            lane,
            #[cfg(feature = "plugin-sidecar")]
            resident: false,
            active: AtomicBool::new(true),
            retirement: AtomicU8::new(Retirement::Retained as u8),
            pending: Mutex::new(BTreeMap::new()),
            binding,
            #[cfg(feature = "plugin-sidecar")]
            native_binding: None,
            wire,
            #[cfg(feature = "plugin-sidecar")]
            native: None,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            plugin,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            sdk: None,
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
