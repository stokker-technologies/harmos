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

pub(crate) struct ScopeCore {
    instances: Arc<Instances>,
    parent: Weak<Instance>,
    admission: Admission,
    cause: Option<Position>,
    children: Arc<Mutex<BTreeMap<OperationInstanceId, Child>>>,
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
    pub(crate) fn over(core: &'a ScopeCore) -> Self {
        Self { core }
    }

    #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
    pub(crate) fn original_control(&self) -> Weak<Instance> {
        self.core.parent.clone()
    }

    pub(crate) fn reserve_tracked<O>(
        &self,
        id: OperationInstanceId,
        lane: &'static str,
        submitted: tokio::time::Instant,
        privileged: bool,
        retired: impl Fn() + Send + Sync + 'static,
    ) -> crate::Result<Reservation<O>>
    where
        O: Send + 'static,
    {
        let children = Arc::downgrade(&self.core.children);
        let preparation =
            self.core
                .instances
                .reserve(id, lane, submitted, privileged, move || {
                    if let Some(children) = children.upgrade() {
                        let removed = children
                            .lock()
                            .expect("scope child lock is healthy")
                            .remove(&id);
                        drop(removed);
                    }
                    retired();
                })?;
        let child = Child::new(preparation.control());
        if let Some(parent) = self.core.parent.upgrade() {
            parent.track(&child.settlement());
        }
        self.core
            .children
            .lock()
            .expect("scope child lock is healthy")
            .insert(id, child);
        Ok(preparation.inherit(self.core.cause))
    }

    pub fn now(&self) -> tokio::time::Instant {
        crate::supervision::executor::now()
    }

    pub fn resources<A: crate::Application>(&self) -> &'a A::Resources {
        self.core.instances.resources_of::<A>()
    }

    pub fn records<A: crate::Application>(&self) -> crate::RecordManager<A> {
        crate::RecordManager::new(self.state::<A>())
    }

    pub fn state<A: crate::Application>(&self) -> crate::StateManager<A> {
        let journal = self.core.instances.state_of::<A>();
        match self.core.cause {
            Some(cause) => journal.caused_by(cause),
            None => journal,
        }
    }
}

impl std::fmt::Debug for Scope<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scope")
            .field("lane", &self.core.admission.name())
            .field("cause", &self.core.cause)
            .finish()
    }
}

impl ScopeCore {
    pub(crate) fn new(
        instances: Arc<Instances>,
        admission: Admission,
        cause: Option<Position>,
        parent: Weak<Instance>,
    ) -> Self {
        Self {
            instances,
            parent,
            admission,
            cause,
            children: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(crate) fn close(self) -> ClosedScope {
        let children: Vec<_> =
            std::mem::take(&mut *self.children.lock().expect("scope child lock is healthy"))
                .into_values()
                .collect();
        for child in &children {
            child.control.cancel();
        }

        ClosedScope { children }
    }
}

impl ClosedScope {
    pub(crate) async fn drain(&mut self) {
        for child in &mut self.children {
            let _ = child
                .settled
                .wait_for(|status| status.phase() == Phase::Settled)
                .await;
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
