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
mod cache;
mod construction;
mod entry;
mod grants;
mod inspection;
mod lookup;
mod registration;
mod registry;
mod retirement;
mod shutdown;
mod staging;
mod submission;

mod discovery;
pub(in crate::operations) use discovery::limits;
mod residents;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// Local Imports
use super::admission::{Admission, Lanes};
use super::binding::Typed;
use super::caller::{Identity, Origin, Source};
use super::{OperationGeneration, OperationInstanceId, Running};

use super::instances::Instances;
use crate::operations::{CacheKey, OperationDeclaration, OperationDefinition};
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::plugin::PluginScope;
use crate::supervision::executor;
use crate::supervision::instance::Instance;
use crate::{Error, Identifier, Result, operations::Lane, operations::Scope, state::Position};

/* === Constants === */

const DEFAULT_CACHE_CAPACITY: usize = 64;

/* === Macros === */

/* === Type Aliases === */

type CachedResponse = harmos_sdk::__private::ErasedResponse;

/* === Structs === */

#[derive(Clone)]
pub struct OperationManager {
    pub(super) registry: Arc<Registry>,
    pub(super) instances: Arc<Instances>,
    pub(super) lanes: Arc<Lanes>,
    pub(super) preparation: Option<Arc<Semaphore>>,
    pub(super) context: Arc<std::sync::OnceLock<super::CallContext>>,
    pub(crate) resident: super::resident::Registry,
}

pub(crate) struct Registry {
    middleware: super::middleware::Configuration,
    state: Mutex<State>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
pub(crate) struct GenerationCommit<'a> {
    _state: std::sync::MutexGuard<'a, State>,
}

struct State {
    closing: bool,
    entries: BTreeMap<String, BTreeMap<String, Arc<Entry>>>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    staging: Vec<std::sync::Weak<PluginScope>>,

    #[cfg(feature = "grpc")]
    grpc: BTreeMap<String, Weak<Entry>>,

    #[cfg(feature = "grpc")]
    grpc_services: std::collections::BTreeSet<String>,

    cache: VecDeque<Cached>,
    cache_capacity: usize,
}

pub(super) struct Entry {
    owner: Identifier,
    definition: OperationDefinition,
    pub(super) contract: Option<crate::operations::Contract>,

    #[cfg(feature = "grpc")]
    pub(super) grpc: Option<crate::grpc::routing::Handler>,

    generation: OperationGeneration,
    lane: &'static str,

    // A resident control runs until cancelled, even while its plugin retires.
    #[cfg(feature = "plugin-sidecar")]
    resident: bool,

    active: AtomicBool,
    retirement: AtomicU8,
    pending: Mutex<BTreeMap<OperationInstanceId, Arc<Instance>>>,
    pub(in crate::operations) binding: Arc<dyn Any + Send + Sync>,

    #[cfg(feature = "plugin-sidecar")]
    pub(super) native_binding: Option<Arc<dyn Any + Send + Sync>>,

    pub(super) wire: Option<super::wire::Dispatch>,

    #[cfg(feature = "plugin-sidecar")]
    pub(super) native: Option<Arc<super::native::Binding>>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    plugin: Option<PluginScope>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) sdk: Option<harmos_domain::operations::lifecycle::Action>,
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
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    plugin: Option<(String, Option<harmos_domain::inventory::contracts::Required>)>,

    // External Authority Never Replaces A Plugin Caller's Own Requirement.
    external: Option<crate::operations::authorization::Authorization>,
}

struct Cached {
    target: Weak<Entry>,
    key: CacheKey,
    identity: Identity,
    response: CachedResponse,
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

/* === Public Functions === */

/* === Private Functions === */

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
