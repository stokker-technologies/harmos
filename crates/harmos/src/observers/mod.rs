/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod manager;

#[cfg(feature = "sidecar")]
mod native;

/* === Re-Exports === */
pub use crate::supervision::instance::{
    Outcome as ObserverOutcome, Phase as ObserverLifecyclePhase, Status as ObserverLifecycle,
};
#[cfg(feature = "sidecar-host")]
pub(crate) use manager::ObserverRetirement;
pub use manager::{
    ObserverActivation, ObserverInstance, ObserverLease, ObserverManager, ObserverRegistration,
    ObserverScope,
};

#[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
pub(crate) use manager::ChildSettlement;

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_contract::inventory::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct ObserverDefinition {
    pub id: String,
    pub interval: Duration,
    pub streams: Vec<StreamDeclaration>,
    pub demand_keys: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObserverScopeId(pub(super) u64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObserverKey {
    pub definition: String,
    pub scope: ObserverScopeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObserverInstanceId(pub(super) u64);

#[derive(Clone, Debug)]
pub struct ObserverStatus {
    pub id: ObserverInstanceId,
    pub phase: ObserverPhase,
    pub leases: usize,
    pub failure: Option<Arc<crate::Error>>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverPhase {
    Starting,
    Running,
    Stopping,
    Stopped,
}

/* === Traits === */
pub trait ObserverBackend: Send + Sync + 'static {
    type Resources: Send + Sync + 'static;
    fn definition(&self) -> ObserverDefinition;
    fn run(
        &self,
        resources: Arc<Self::Resources>,
        activation: ObserverActivation,
    ) -> impl Future<Output = crate::Result<()>> + Send;
}

/* === Implementations === */
impl ObserverInstanceId {
    pub fn value(self) -> u64 {
        self.0
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
