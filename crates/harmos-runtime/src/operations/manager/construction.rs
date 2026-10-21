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
    pub(crate) fn new(
        instances: Arc<Instances>, settings: &crate::operations::Settings, reserved: std::collections::BTreeSet<String>, identity: Arc<()>,
    ) -> Self {
        Self {
            resident: crate::operations::resident::Registry::new(&instances, reserved, identity),
            context: Arc::new(std::sync::OnceLock::from(crate::operations::CallContext::default())),
            instances,
            preparation: settings.preparation_bytes.map(|bytes| Arc::new(Semaphore::new(bytes.get()))),
            lanes: Arc::new(Lanes::default()),
            registry: Arc::new(Registry {
                middleware: crate::operations::middleware::Configuration::default(),
                state: Mutex::new(State {
                    closing: false,
                    entries: BTreeMap::new(),
                    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                    staging: Vec::new(),
                    #[cfg(feature = "grpc")]
                    grpc: BTreeMap::new(),
                    #[cfg(feature = "grpc")]
                    grpc_services: std::collections::BTreeSet::new(),
                    cache: VecDeque::new(),
                    cache_capacity: DEFAULT_CACHE_CAPACITY,
                }),
            }),
        }
    }

    pub fn register_middleware(&self, middleware: Arc<dyn crate::operations::middleware::Middleware>) -> Result<()> {
        self.registry.middleware.register(middleware)
    }

    pub fn register_authorizer(&self, authorizer: Arc<dyn crate::operations::authorization::Authorizer>) -> Result<()> {
        self.registry.middleware.register_authorizer(authorizer)
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn submit_external_charged<O: OperationDeclaration, P: Send + 'static>(
        &self, request: crate::operations::StreamItem<O>, endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>((crate::operations::binding::Request::charged(request)?, endpoints), None, &[], &Source::Host.external()?, None)
    }

    pub(in crate::operations) fn authorize_external(
        &self, entry: &Entry, origin: &Origin,
    ) -> Result<Option<crate::operations::authorization::Authorization>> {
        #[cfg(feature = "grpc")]
        if matches!(origin.identity, crate::operations::caller::Identity::External(_)) {
            let Some(authorizer) = self.registry.middleware.authorizer() else {
                // Preserve Existing Explicit Anonymous Routes When No Policy Is Configured.
                return Ok(None);
            };
            let denied = || Error::OperationUnauthorized {
                caller: "external".to_owned(),
                owner: entry.owner.to_string(),
                operation: entry.id().to_owned(),
            };
            let contract = entry.contract.as_ref().ok_or_else(denied)?;
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            let plugin = entry.plugin.is_some();
            #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
            let plugin = false;
            let selected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                authorizer.authorize(
                    self.call_context(),
                    crate::operations::authorization::Provider {
                        owner: &entry.owner,
                        plugin,
                        generation: entry.generation,
                        definition: &entry.definition,
                        contract,
                    },
                )
            }))
            .map_err(|_| Error::InvalidInput { field: "operation authorizer", reason: "authorization callback panicked" })??
            .ok_or_else(denied)?;
            let selected_contract = match &selected {
                crate::operations::authorization::Authorization::Full(contract) => contract,
                crate::operations::authorization::Authorization::Restricted(required) => &required.contract,
            };
            if !contract.compatible(selected_contract) {
                return Err(denied());
            }
            return Ok(Some(selected));
        }
        #[cfg(not(feature = "grpc"))]
        let _ = (entry, origin);
        Ok(None)
    }

    pub fn with_context(&self, context: crate::operations::CallContext) -> Self {
        let mut handle = self.clone();
        handle.context = Arc::new(std::sync::OnceLock::from(context));
        handle
    }

    pub(in crate::operations) fn call_context(&self) -> &crate::operations::CallContext {
        self.context.get().expect("context freezes before operation execution")
    }

    pub fn status(&self, id: OperationInstanceId) -> Option<crate::operations::OperationStatus> {
        self.instances.status(id)
    }

    pub(in crate::operations) fn claim_preparation(&self, bytes: std::num::NonZeroUsize) -> Result<OwnedSemaphorePermit> {
        let count = u32::try_from(bytes.get()).map_err(|_| Error::ResourceExhausted { resource: "operation preparation bytes" })?;
        self.preparation
            .as_ref()
            .ok_or(Error::InvalidInput {
                field: "operation preparation bytes",
                reason: "encoded preparation requires an explicit preparation budget",
            })?
            .clone()
            .try_acquire_many_owned(count)
            .map_err(|_| Error::ResourceExhausted { resource: "operation preparation bytes" })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
