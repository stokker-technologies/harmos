/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl OperationManager {
    pub fn register_operation<T>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_operation_at::<T>(owner, lane.name(), resources)
    }

    pub fn register_operation_id<T>(
        &self,
        owner: impl Into<String>,
        id: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_operation_id::<T>(id, lane, resources)
        })
    }

    pub(in crate::operations) fn register_operation_at<T>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_owned_at::<T>(lane, resources)
        })
    }

    pub fn register_wire_operation<T>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_wire_operation::<T>(lane, resources)
        })
    }

    pub fn register_wire_contract<T>(
        &self,
        owner: impl Into<String>,
        contract: &crate::operations::Contract,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_wire_contract::<T>(contract, lane, resources)
        })
    }

    pub(in crate::operations) fn binding_owned_definition<T>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        resources: Arc<T::Resources>,
        definition: Definition,
        wire: Option<crate::operations::wire::Dispatch>,
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>>
    where
        T: for<'ctx> crate::operations::Operation<Context<'ctx> = Context<'ctx>>,
    {
        definition
            .validate()
            .map_err(|source| Error::OperationDeclaration { source })?;
        if <T::Endpoints as crate::operations::EndpointSet>::item_types()
            != crate::operations::item_types::<T::Declaration>()
        {
            return Err(Error::InvalidInput {
                field: "operation endpoint types",
                reason: "authored endpoints do not match declared stream item types",
            });
        }
        let binding = crate::operations::authored::factory::<T>(self, resources);
        self.binding_typed(
            owner,
            lane,
            binding,
            definition,
            wire,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            plugin,
        )
    }

    pub(super) fn binding_typed<O: OperationDeclaration, P: Send + 'static>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        binding: Typed<O, P>,
        definition: Definition,
        wire: Option<crate::operations::wire::Dispatch>,
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>> {
        let owner =
            Identifier::new(owner.into()).map_err(|source| Error::OperationDeclaration {
                source: source.into(),
            })?;
        let bounded = self
            .instances
            .lane_bounded(lane)
            .ok_or_else(|| Error::NotRegistered {
                domain: "operation lane",
                name: lane.to_owned(),
            })?;
        let generation = OperationGeneration::fresh()?;
        Ok(Arc::new(Entry {
            owner,
            definition,
            contract: None,
            #[cfg(feature = "grpc")]
            grpc: None,
            generation,
            lane,
            bounded,
            #[cfg(feature = "plugin-sidecar")]
            resident: false,
            active: AtomicBool::new(true),
            retirement: AtomicU8::new(Retirement::Retained as u8),
            pending: Mutex::new(BTreeMap::new()),
            binding: Arc::new(binding),
            #[cfg(feature = "plugin-sidecar")]
            native_binding: None,
            wire,
            #[cfg(feature = "plugin-sidecar")]
            native: None,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            plugin,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            sdk: None,
        }))
    }
}
