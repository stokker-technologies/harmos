/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The resident kinds of the operation manager: registering observers and
// services, activating them over shared resources, reporting their instances
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
use super::super::{OperationDeclaration, OperationInstance, OperationInstanceId, Response};
use super::OperationManager;
use crate::observers::{Backend, ObserverBackend, ObserverRegistration};
use crate::services::{Resident, ServiceDefinition};
use crate::{Application, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationManager {
    // Register an observer visible at once, activated over any resources of
    // its backend's resource type.
    pub fn register_observer<B: ObserverBackend>(&self, backend: B) -> Result<()> {
        let definition = backend.definition()?;
        self.resident.register(Definition::Observer(definition), Arc::new(Backend(backend)), None, false, false)
    }

    // Reserve observers bound to these resources; they become visible when the
    // registration commits.
    pub fn stage_observers<B: ObserverBackend>(
        &self, resources: &Arc<B::Resources>, backends: impl IntoIterator<Item = B>,
    ) -> Result<ObserverRegistration> {
        self.stage_scoped(
            resources,
            backends,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            None,
        )
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub fn stage_plugin_observers<B: ObserverBackend>(
        &self, resources: &Arc<B::Resources>, plugin: &crate::plugin::PluginScope, backends: impl IntoIterator<Item = B>,
    ) -> Result<ObserverRegistration> {
        self.stage_scoped(resources, backends, Some(plugin.clone()))
    }

    fn stage_scoped<B: ObserverBackend>(
        &self, resources: &Arc<B::Resources>, backends: impl IntoIterator<Item = B>,
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))] plugin: Option<crate::plugin::PluginScope>,
    ) -> Result<ObserverRegistration> {
        let scope = self.resident.scope(Arc::clone(resources))?;
        let entries = backends
            .into_iter()
            .map(|backend| {
                let definition = backend.definition()?;
                let runner: Arc<dyn resident::Runner> = Arc::new(Backend(backend));
                Ok((Definition::Observer(definition), runner))
            })
            .collect::<Result<Vec<_>>>()?;
        self.resident.stage(
            &scope,
            entries,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            plugin,
        )
    }

    // Register an authored observer that runs in process; `factory` binds its
    // host for each activation.
    #[cfg(feature = "plugin-sidecar")]
    pub fn register_native_observer<R, H, F>(&self, instance: harmos_sidecar::Instance<R, H>, factory: F) -> Result<()>
    where
        R: Send + Sync + 'static,
        H: Send + Sync + 'static,
        F: Fn(Arc<R>) -> H + Send + Sync + 'static,
    {
        self.register_observer(crate::observers::Native {
            instance,
            factory: Arc::new(factory),
            schema_settings: Arc::clone(&self.schema_settings),
        })
    }

    #[cfg(feature = "plugin-sidecar")]
    pub fn stage_native_observers<R, H, F>(
        &self, resources: &Arc<R>, instances: impl IntoIterator<Item = harmos_sidecar::Instance<R, H>>, factory: F,
    ) -> Result<ObserverRegistration>
    where
        R: Send + Sync + 'static,
        H: Send + Sync + 'static,
        F: Fn(Arc<R>) -> H + Send + Sync + 'static,
    {
        let factory = Arc::new(factory);
        self.stage_observers(
            resources,
            instances.into_iter().map(|instance| crate::observers::Native {
                instance,
                factory: Arc::clone(&factory),
                schema_settings: Arc::clone(&self.schema_settings),
            }),
        )
    }

    // Register a service whose resident each activation creates from the
    // resources it is activated over.
    pub fn register_service<A, R, F, T>(&self, definition: ServiceDefinition, factory: F) -> Result<()>
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

    pub(crate) fn resident_instance<O: OperationDeclaration<Response = ()>>(lease: Lease) -> OperationInstance<O> {
        let id = lease.id();
        let generation = lease.generation();
        let control = lease.lifecycle();
        let settled = lease.settlement();
        OperationInstance {
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

    pub(in crate::operations) async fn activate_resident<O, R>(
        &self, id: &str, resources: Arc<R>, holder: Identity,
    ) -> Result<OperationInstance<O>>
    where
        O: OperationDeclaration<Response = ()>,
        R: Send + Sync + 'static,
    {
        let scope = self.resident.scope(resources)?;
        let lease = self.resident.activate(self, id, &scope, std::any::TypeId::of::<R>(), holder, false).await?;
        Ok(Self::resident_instance(lease))
    }

    // Activate the resident `id` over `resources`. Activations sharing the same
    // resources share its instance; each handle holds one lease.
    pub async fn submit_with_resources_at<R: Send + Sync + 'static>(
        &self, id: &str, resources: Arc<R>,
    ) -> Result<OperationInstance<Activation>> {
        self.activate_resident::<Activation, R>(id, resources, Identity::Host).await
    }

    // Activate the resident `id` over the runtime's own resources.
    pub async fn submit_resident(&self, id: &str) -> Result<OperationInstance<Activation>> {
        let root = self.resident.root();
        let lease = self.resident.activate(self, id, &root, std::any::TypeId::of::<()>(), Identity::Host, false).await?;
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
    pub fn root_resources(&self) -> Arc<()> {
        self.resident.root_resources()
    }

    // Stop every resident over these resources and forget them with the
    // definitions bound to them.
    pub async fn retire_resources<R: Send + Sync + 'static>(&self, resources: &Arc<R>) -> Result<()> {
        let Some(scope) = self.resident.scope_of(resources) else {
            return Ok(());
        };
        self.resident.retire_scope(&scope).await?.into_outcome().map_err(|source| crate::Error::TaskFailed { domain: "resident", source })
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) async fn retire_scope_of<R: Send + Sync + 'static>(&self, resources: &Arc<R>) -> Result<Option<resident::Retirement>> {
        let Some(scope) = self.resident.scope_of(resources) else {
            return Ok(None);
        };
        self.resident.retire_scope(&scope).await.map(Some)
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

    // Stop every observer instance and wait for each to settle. A cleanup that
    // fails stays owned by its instance; stopping again retries it.
    pub async fn stop_observers(&self) -> Result<()> {
        self.resident.stop_kind(Kind::Observer).await
    }

    pub(crate) async fn shutdown_observers(&self) -> Result<()> {
        self.resident.shutdown_observers().await
    }

    pub(crate) async fn shutdown_application_services(&self) -> Result<()> {
        self.resident.shutdown_application().await
    }

    // Close resident admission, stop every observer and service including
    // infrastructure, and forget their definitions once they settle.
    pub async fn shutdown_residents(&self) -> Result<()> {
        self.resident.shutdown().await
    }

    pub(crate) fn observer_cleanup_pending(&self) -> bool {
        self.resident.cleanup_pending(|kind, _| kind == Kind::Observer)
    }

    pub(crate) fn service_cleanup_pending(&self, infrastructure: bool) -> bool {
        self.resident.cleanup_pending(|kind, internal| kind == Kind::Service && (infrastructure || !internal))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
