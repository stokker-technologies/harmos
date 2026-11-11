/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The resident services of the operation manager: registering services, activating them over shared resources, reporting their instances
// and retiring them in the runtime's shutdown order.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::Arc;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::super::caller::Identity;
use super::super::handler::Outcome;
use super::super::resident::{self, Activation, Definition, Key, Kind, Lease, Snapshot};
use super::super::{OperationDeclaration, OperationInstanceId, Response, Running};
use super::Manager;
use crate::services::{Resident, ServiceDefinition};
use crate::{Application, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    pub(crate) fn stage_services(
        &self, entries: Vec<(Definition, Arc<dyn resident::Runner>)>,
        #[cfg(feature = "plugin-guest")] plugin: Option<crate::guest::host::plugin::PluginScope>,
    ) -> Result<resident::Staged> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(crate::Error::Closing { domain: "operations" });
        }
        for (definition, _) in &entries {
            let (owner, local) = definition.id().split_once('.').expect("validated service identity");
            if state.owned(local, owner).is_some() {
                return Err(crate::Error::AlreadyRegistered {
                    domain: "service",
                    name: definition.id().to_owned(),
                });
            }
        }
        let staged = self.resident.stage(
            &self.resident.root(),
            entries,
            #[cfg(feature = "plugin-guest")]
            plugin,
        );
        drop(state);
        staged
    }

    pub(crate) async fn start_registered_service(&self, id: &str, owner: &crate::services::ServiceOwner) -> Result<()> {
        let root = self.resident.root();
        let identity = match owner {
            crate::services::ServiceOwner::Host => Identity::Host,
            #[cfg(feature = "plugin-guest")]
            crate::services::ServiceOwner::Plugin(scope) => Identity::Plugin {
                owner: scope.owner().into(),
                generation: scope.generation(),
            },
        };
        let lease = self
            .resident
            .activate(self, id, &root, std::any::TypeId::of::<harmos_operations::execution::ExecutionContext>(), identity, false)
            .await?;
        self.resident.hold(lease);
        Ok(())
    }

    // Register a service whose resident each activation creates from the
    // resources it is activated over.
    pub fn register_service<A, F, T>(&self, definition: ServiceDefinition, factory: F) -> Result<()>
    where
        A: Application,
        F: Fn(Arc<harmos_operations::execution::ExecutionContext>) -> T + Send + Sync + 'static,
        T: Resident<A>,
    {
        self.register_service_with_context::<A, harmos_operations::execution::ExecutionContext, F, T>(definition, factory)
    }

    pub(crate) fn register_service_with_context<A, R, F, T>(&self, definition: ServiceDefinition, factory: F) -> Result<()>
    where
        A: Application,
        R: Send + Sync + 'static,
        F: Fn(Arc<R>) -> T + Send + Sync + 'static,
        T: Resident<A>,
    {
        let runner = crate::services::Typed::new(definition.clone(), factory);
        self.resident.register(Definition::Service(definition), Arc::new(runner), None, false, false)
    }

    // Start a resident that runs once and belongs to the runtime: declared
    // services and infrastructure such as storage workers.
    pub(crate) fn start_service<A: Application>(&self, service: crate::services::Once<A>, internal: bool, reserved: bool) -> Result<Lease> {
        let definition = Definition::Service(service.definition());
        self.resident.start_once(self, definition, Arc::new(service), internal, reserved)
    }

    pub(crate) fn hold_resident(&self, lease: Lease) {
        self.resident.hold(lease);
    }

    pub(crate) fn resident_instance<O: OperationDeclaration<Response = ()>>(lease: Lease) -> Running<O> {
        let id = lease.id();
        let generation = lease.generation();
        let control = lease.lifecycle();
        let settled = lease.settlement();
        Running {
            id,
            generation,
            outcome: Outcome::Resident {
                lease: Some(lease),
                settled: std::sync::Mutex::new(Box::pin(async move { settled.await.map(|()| Response::new(())) })),
            },
            control,
            released: false,
        }
    }

    // Activate the resident `id` over `resources`. Activations sharing the same
    // resources share its instance; each handle holds one lease.
    #[cfg(feature = "plugin-guest")]
    pub async fn submit_resident_in(
        &self, id: &str, resources: Arc<harmos_operations::execution::ExecutionContext>,
    ) -> Result<Running<Activation>> {
        // Retirement Must Not Recreate A Scope For An Old Connection.
        let scope = self.resident.scope_of(&resources).ok_or(crate::Error::Closing { domain: "observer scope" })?;
        let lease = self
            .resident
            .activate(self, id, &scope, std::any::TypeId::of::<harmos_operations::execution::ExecutionContext>(), Identity::Host, false)
            .await?;
        Ok(Self::resident_instance(lease))
    }

    #[cfg(feature = "grpc")]
    pub(crate) async fn submit_infrastructure<R: Send + Sync + 'static>(&self, id: &str, resources: Arc<R>) -> Result<Running<Activation>> {
        let scope = self.resident.scope(resources)?;
        let lease = self.resident.activate(self, id, &scope, std::any::TypeId::of::<R>(), Identity::Host, false).await?;
        Ok(Self::resident_instance(lease))
    }

    // Activate the resident `id` over the runtime's own resources.
    pub async fn submit_resident(&self, id: &str) -> Result<Running<Activation>> {
        let root = self.resident.root();
        let lease = self
            .resident
            .activate(self, id, &root, std::any::TypeId::of::<harmos_operations::execution::ExecutionContext>(), Identity::Host, false)
            .await?;
        Ok(Self::resident_instance(lease))
    }

    pub fn resident_definition(&self, id: &str) -> Option<Definition> {
        self.resident.definition(id)
    }

    pub fn resident_definitions(&self) -> Vec<Definition> {
        self.resident.definitions()
    }

    pub fn resident(&self, key: &Key) -> Option<Snapshot> {
        self.resident.status(key)
    }

    pub fn residents(&self) -> BTreeMap<Key, Snapshot> {
        self.resident.statuses()
    }

    pub fn watch_residents(&self) -> watch::Receiver<BTreeMap<Key, Snapshot>> {
        self.resident.subscribe()
    }

    // The key of a running resident instance.
    pub fn resident_key(&self, instance: OperationInstanceId) -> Option<Key> {
        self.resident.statuses().into_iter().find_map(|(key, snapshot)| (snapshot.instance == instance).then_some(key))
    }

    // The resources the runtime's own residents share.
    pub fn context(&self) -> harmos_operations::execution::ExecutionContext {
        self.resident.root_resources().as_ref().clone()
    }

    pub async fn unregister_resident(&self, id: &str) -> Result<()> {
        self.resident.unregister(id).await
    }

    pub(crate) fn resident_identity(&self) -> Arc<()> {
        self.resident.identity()
    }

    pub(crate) fn handles<A: Application>(&self) -> crate::services::Handles<A> {
        self.instances.handles_of::<A>().clone()
    }

    pub(crate) fn close_resident_admission(&self) {
        self.resident.close_admission();
    }

    pub(crate) async fn shutdown_application_services(&self) -> Result<()> {
        self.resident.shutdown_application().await
    }

    // Close resident admission, stop every service including
    // infrastructure, and forget their definitions once they settle.
    pub async fn shutdown_residents(&self) -> Result<()> {
        self.resident.shutdown().await
    }

    pub(crate) fn service_cleanup_pending(&self, infrastructure: bool) -> bool {
        self.resident.cleanup_pending(|kind, internal| kind == Kind::Service && (infrastructure || !internal))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
