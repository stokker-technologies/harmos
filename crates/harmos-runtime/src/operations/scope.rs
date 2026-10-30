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
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::lane::Admission;
use crate::operations::OperationInstanceId;
use crate::operations::instances::{Instances, Reservation};
use crate::state::Position;
use crate::supervision::instance::{Instance, Phase, Status};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy)]
pub struct Scope<'a> {
    core: &'a ScopeCore,
}

#[derive(Clone)]
pub(crate) struct ScopeCore {
    cancellation: harmos_sdk::operations::Cancellation,
    instances: Arc<Instances>,
    parent: Weak<Instance>,
    admission: Admission,
    cause: Option<Position>,
    children: Arc<Mutex<BTreeMap<OperationInstanceId, Child>>>,
    closed: Arc<AtomicBool>,
}

pub(crate) struct ClosedScope {
    children: Vec<Child>,
}

pub(crate) struct Child {
    settled: watch::Receiver<Status>,
    control: Arc<Instance>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<'a> Scope<'a> {
    pub fn cancellation(&self) -> &harmos_sdk::operations::Cancellation {
        &self.core.cancellation
    }

    pub(crate) fn owned(&self) -> ScopeCore {
        self.core.clone()
    }

    pub(crate) fn over(core: &'a ScopeCore) -> Self {
        Self { core }
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn original_control(&self) -> Weak<Instance> {
        self.core.parent.clone()
    }

    pub(crate) fn reserve_tracked<O>(
        &self, id: OperationInstanceId, lane: &'static str, submitted: tokio::time::Instant, privileged: bool,
        retired: impl Fn() + Send + Sync + 'static,
    ) -> crate::Result<Reservation<O>>
    where
        O: Send + 'static,
    {
        let mut tracked = self.core.children.lock().expect("scope child lock is healthy");
        if self.core.closed.load(Ordering::Acquire) {
            return Err(crate::Error::Closing { domain: "operation scope" });
        }
        let children = Arc::downgrade(&self.core.children);
        let preparation = self.core.instances.reserve(id, lane, submitted, privileged, move || {
            if let Some(children) = children.upgrade() {
                let removed = children.lock().expect("scope child lock is healthy").remove(&id);
                drop(removed);
            }
            retired();
        })?;
        let child = Child::new(preparation.control());
        if let Some(parent) = self.core.parent.upgrade() {
            parent.track(&child.settlement());
        }
        tracked.insert(id, child);
        Ok(preparation.inherit(self.core.cause))
    }

    pub fn now(&self) -> tokio::time::Instant {
        crate::supervision::executor::now()
    }

    pub fn records<A: crate::Application>(&self) -> crate::records::RecordManager<A> {
        crate::records::RecordManager::new(self.state::<A>())
    }

    pub fn storage<A: crate::Application>(&self) -> Option<&'a crate::storage::StorageManager<crate::storage::ErasedCatalog>> {
        self.core.instances.handles_of::<A>().storage.get()
    }

    pub fn streams<A: crate::Application>(&self) -> crate::streams::StreamManager<A> {
        self.core.instances.handles_of::<A>().streams.clone()
    }

    pub fn state<A: crate::Application>(&self) -> crate::state::StateManager<A> {
        let journal = self.core.instances.state_of::<A>();
        match self.core.cause {
            Some(cause) => journal.caused_by(cause),
            None => journal,
        }
    }
}

impl std::fmt::Debug for Scope<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Scope").field("lane", &self.core.admission.name()).field("cause", &self.core.cause).finish()
    }
}

impl ScopeCore {
    pub(super) fn cancellation(&self) -> &harmos_sdk::operations::Cancellation {
        &self.cancellation
    }

    pub(crate) fn new(instances: Arc<Instances>, admission: Admission, cause: Option<Position>, parent: Weak<Instance>) -> Self {
        Self {
            cancellation: Default::default(),
            instances,
            parent,
            admission,
            cause,
            children: Arc::new(Mutex::new(BTreeMap::new())),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn close(self) -> ClosedScope {
        let children: Vec<_> = {
            let mut children = self.children.lock().expect("scope child lock is healthy");
            self.closed.store(true, Ordering::Release);
            std::mem::take(&mut *children).into_values().collect()
        };
        for child in &children {
            child.control.request_cancel();
        }

        ClosedScope { children }
    }
}

impl ClosedScope {
    pub(crate) async fn drain(&mut self) {
        for child in &mut self.children {
            let _ = child.settled.wait_for(|status| status.phase() == Phase::Settled).await;
        }
    }
}

impl Child {
    pub(crate) fn settlement(&self) -> Arc<Instance> {
        Arc::clone(&self.control)
    }

    pub(crate) fn new(control: Arc<Instance>) -> Self {
        let settled = control.settlement();
        Self { settled, control }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
