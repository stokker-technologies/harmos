/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod admission;
mod construction;
mod entry;
mod grants;
mod inspection;
mod lookup;
mod plugin_grpc;
mod registration;
mod registry;
mod retirement;
mod shutdown;
mod staging;
mod submission;

mod discovery;

mod residents;

/* === Re-Exports === */

pub(crate) use plugin_grpc::Admission as GrpcAdmission;

/* === Imports === */

// Standard Library Imports
use std::any::Any;
use std::collections::BTreeMap;
#[cfg(feature = "grpc")]
use std::sync::Weak;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// Local Imports
use super::admission::{Admission, Lanes};
use super::binding::Typed;
use super::caller::{Origin, Source};
use super::{OperationGeneration, OperationInstanceId, Running};

use super::instances::Instances;
#[cfg(feature = "plugin-guest")]
use crate::guest::host::plugin::PluginScope;
use crate::operations::{OperationDeclaration, OperationDefinition};
use crate::supervision::instance::Instance;
use crate::{Error, Identifier, Result, operations::Lane, operations::Scope, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct OperationManager {
    inner: Manager,
}

#[derive(Clone)]
pub struct Manager {
    pub(crate) registry: Arc<Registry>,
    pub(super) schema_settings: Arc<crate::config::SchemaSettings>,
    pub(super) instances: Arc<Instances>,
    pub(super) lanes: Arc<Lanes>,
    pub(crate) preparation: Option<Arc<Semaphore>>,
    pub(super) context: Arc<std::sync::OnceLock<super::CallContext>>,
    pub(crate) resident: super::resident::Registry,
}

#[cfg(feature = "plugin-guest")]
#[derive(Clone)]
pub(crate) struct WeakOperationManager {
    schema_settings: std::sync::Weak<crate::config::SchemaSettings>,
    registry: std::sync::Weak<Registry>,
    instances: std::sync::Weak<Instances>,
    lanes: std::sync::Weak<Lanes>,
    preparation: Option<std::sync::Weak<Semaphore>>,
    context: std::sync::Weak<std::sync::OnceLock<super::CallContext>>,
    resident: super::resident::WeakRegistry,
}

pub(crate) struct Registry {
    pub(crate) service_runner: crate::services::ServiceRunner,
    middleware: super::middleware::Configuration,
    state: Mutex<State>,
}

#[cfg(feature = "plugin-guest")]
pub(crate) struct GenerationCommit<'a> {
    _state: std::sync::MutexGuard<'a, State>,
}

struct State {
    closing: bool,
    entries: BTreeMap<String, BTreeMap<String, Arc<Entry>>>,

    #[cfg(feature = "plugin-guest")]
    staging: Vec<std::sync::Weak<PluginScope>>,

    #[cfg(feature = "grpc")]
    grpc: BTreeMap<String, std::sync::Weak<Entry>>,

    #[cfg(feature = "grpc")]
    grpc_services: std::collections::BTreeSet<String>,
}

pub(super) struct Entry {
    pub(super) carrier: harmos_operations::execution::__private::ResourceSet,
    owner: Identifier,
    definition: OperationDefinition,
    pub(super) contract: Option<crate::operations::Contract>,

    #[cfg(feature = "grpc")]
    pub(super) grpc: Option<crate::grpc::routing::Handler>,

    pub(super) generation: OperationGeneration,
    lane: &'static str,

    // A resident control runs until cancelled, even while its plugin retires.
    active: AtomicBool,
    publication: Arc<AtomicBool>,
    retirement: AtomicU8,
    pending: Mutex<BTreeMap<OperationInstanceId, Arc<Instance>>>,
    pub(in crate::operations) binding: Arc<dyn Any + Send + Sync>,

    pub(super) wire: Option<super::wire::Dispatch>,

    #[cfg(feature = "plugin-guest")]
    plugin: Option<PluginScope>,

    #[cfg(feature = "plugin-guest")]
    pub(super) sdk: Option<harmos_operations::lifecycle::Action>,
}

pub(super) struct Submission {
    pub(super) submitted: tokio::time::Instant,
    pub(super) entry: Arc<Entry>,
    pub(super) cause: Option<Position>,
    pub(super) preparation: Option<OwnedSemaphorePermit>,
}

// The caller's grant authority for one admitted request. Typed bindings check
// the request grant at admission; encoded bindings learn it from the provider
// while preparing and check it before any attempt runs.
#[derive(Clone, Debug)]
pub(super) struct GrantAuthority {
    owner: String,
    operation: String,

    // A cross-owner plugin caller and its declared requirement for this operation.
    #[cfg(feature = "plugin-guest")]
    plugin: Option<(String, Option<harmos_domain::inventory::contracts::Required>)>,

    // External Authority Never Replaces A Plugin Caller's Own Requirement.
    external: Option<crate::operations::authorization::Authorization>,
}

/* === Enums === */

#[repr(u8)]
enum Retirement {
    Retained,
    Abandoned,
    Explicit,
    Runtime,
}

/* === Traits === */

/* === Implementations === */

impl Manager {
    pub(crate) fn clock(&self) -> &crate::clock::Clock {
        &self.instances.clock
    }

    pub(crate) fn resources(&self) -> &harmos_operations::execution::__private::ResourceSet {
        &self.instances.resources
    }

    pub(super) fn resources_for(&self, entry: &Entry) -> harmos_operations::execution::__private::ResourceSet {
        let mut resources = harmos_operations::execution::ExecutionContext::new(entry.carrier.clone(), super::CallContext::default())
            .with_resources(self.resources().clone())
            .resources()
            .clone();
        resources.replace(Arc::new(harmos_operations::clock::ClockAccess(Arc::new(super::clock::Clock(self.clock().clone())))));
        resources
    }
}

#[cfg(feature = "plugin-guest")]
impl std::fmt::Debug for WeakOperationManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("WeakOperationManager").finish_non_exhaustive()
    }
}

#[cfg(feature = "plugin-guest")]
impl WeakOperationManager {
    pub(crate) fn upgrade(&self) -> Option<Manager> {
        Some(Manager {
            schema_settings: self.schema_settings.upgrade()?,
            registry: self.registry.upgrade()?,
            instances: self.instances.upgrade()?,
            lanes: self.lanes.upgrade()?,
            preparation: match &self.preparation {
                Some(budget) => Some(budget.upgrade()?),
                None => None,
            },
            context: self.context.upgrade()?,
            resident: self.resident.upgrade()?,
        })
    }
}

#[cfg(feature = "plugin-guest")]
impl Manager {
    pub(crate) fn downgrade(&self) -> WeakOperationManager {
        WeakOperationManager {
            schema_settings: Arc::downgrade(&self.schema_settings),
            registry: Arc::downgrade(&self.registry),
            instances: Arc::downgrade(&self.instances),
            lanes: Arc::downgrade(&self.lanes),
            preparation: self.preparation.as_ref().map(Arc::downgrade),
            context: Arc::downgrade(&self.context),
            resident: self.resident.downgrade(),
        }
    }
}

impl OperationManager {
    pub fn clock(&self) -> &crate::clock::Clock {
        self.inner.clock()
    }

    pub(crate) fn new(inner: Manager) -> Self {
        Self { inner }
    }

    pub(crate) fn execution(&self) -> Manager {
        self.inner.clone()
    }

    pub async fn register<C: harmos_operations::execution::operations::OperationCatalog>(&self, lane: Lane) -> Result<()> {
        use crate::operations::RegistrationExt;
        self.register_batch(C::batch().lane(lane)).await
    }

    pub async fn register_batch(&self, registration: crate::operations::Registration) -> Result<()> {
        self.inner.register_native(registration)
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        self.inner.unregister(id).await
    }

    pub fn with_context(&self, context: super::CallContext) -> Self {
        Self::new(self.inner.with_context(context))
    }
}

impl std::ops::Deref for OperationManager {
    type Target = Manager;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "plugin-guest")]
fn grant_allowed(required: Option<&harmos_domain::inventory::contracts::Required>, grant: Option<&harmos_domain::plugin::Grant>) -> bool {
    match grant {
        None => true,
        Some(grant) => required.is_some_and(|required| required.grants(grant)),
    }
}

async fn settle(entry: &Entry) -> Result<()> {
    let pending = entry.pending.lock().expect("operation settlement lock is healthy").clone();
    for settled in pending.into_values() {
        settled.settle().await.map_err(|source| Error::OperationSettlement { source })?;
    }
    Ok(())
}

/* === Tests === */
