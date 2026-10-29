/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{Notify, broadcast, watch};

use super::{CleanupOwner, PluginEvent, PluginEventKind, PluginGeneration, PluginState, Router};
use crate::Error;
use crate::supervision::cleanup::{Cleanup, Finalize, Resource};
use crate::supervision::dependencies;

type Outcomes = Arc<Mutex<BTreeMap<String, Result<(), Arc<Error>>>>>;
pub(super) type Notice = Arc<dyn Fn(PluginState) + Send + Sync>;

#[derive(Clone)]
pub(super) struct Owner {
    pub generation: PluginGeneration,
    pub state: watch::Sender<PluginState>,
    pub cleanup: Option<CleanupOwner>,
    pub notice: Option<Notice>,
}

pub(super) struct Driver {
    pub schedule: dependencies::Retirement<String>,
    pub owners: BTreeMap<String, Owner>,
    pub router: Router,
    pub events: broadcast::Sender<PluginEvent>,
    pub revision: u64,
    pub outcomes: Outcomes,
    pub previous: Option<Arc<Wait>>,
}

pub(super) struct Wait {
    pub cleanup: Arc<Cleanup<Driver>>,
    pub selected: Vec<(String, PluginGeneration)>,
    pub outcomes: Outcomes,
}

#[derive(Clone)]
pub struct PluginCleanup {
    shared: Arc<ClosingState>,
}

struct ClosingState {
    status: watch::Sender<PluginCleanupStatus>,
    retry: Notify,
}

pub(super) struct Closing {
    pub topology: dependencies::Graph<String>,
    pub owners: BTreeMap<String, Owner>,
    pub previous: Option<Arc<Wait>>,
    pub router: Router,
    pub events: broadcast::Sender<PluginEvent>,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub enum PluginCleanupStatus {
    Waiting,
    Running,
    Failed(Arc<Error>),
    Settled,
}

impl Resource for Driver {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(async move {
            let mut pending = FuturesUnordered::new();
            loop {
                for id in self.schedule.take_ready() {
                    let owner = &self.owners[&id];
                    let previous =
                        self.previous.as_ref().filter(|previous| previous.selected.iter().any(|(selected, _)| selected == &id)).cloned();
                    if previous.is_none() {
                        owner.state.send_replace(PluginState::Stopping);
                        self.router.quiesce(&id);
                        self.publish(&id, PluginState::Stopping, PluginEventKind::Stopping);
                    }
                    let cleanup = owner.cleanup.clone();
                    pending.push(async move {
                        let inherited = previous.is_some();
                        let result = if let Some(previous) = previous {
                            match previous.cleanup.settle().await {
                                Ok(()) => previous
                                    .outcomes
                                    .lock()
                                    .expect("retirement outcomes are healthy")
                                    .get(&id)
                                    .expect("an eligible previous owner has an outcome")
                                    .clone(),
                                Err(error) => Err(error),
                            }
                        } else {
                            match cleanup {
                                Some(cleanup) => cleanup.settle().await,
                                None => Ok(()),
                            }
                        };
                        (id, result, inherited)
                    });
                }
                let Some((id, result, inherited)) = pending.next().await else {
                    break;
                };
                self.schedule.complete(&id, result.is_ok());
                let (state, event) = if result.is_ok() {
                    (PluginState::Registered, PluginEventKind::Stopped)
                } else {
                    (PluginState::Failed, PluginEventKind::Failed)
                };
                if !inherited {
                    self.router.revoke(&id);
                    self.owners[&id].state.send_replace(state);
                }
                self.outcomes.lock().expect("retirement outcomes are healthy").insert(id.clone(), result);
                if !inherited {
                    self.publish(&id, state, event);
                }
            }
            Ok(())
        })
    }
}

impl Driver {
    fn publish(&self, id: &str, state: PluginState, kind: PluginEventKind) {
        if let Some(notice) = &self.owners[id].notice {
            notice(state);
        }
        let _ = self.events.send(PluginEvent::Changed {
            id: id.to_owned(),
            generation: Some(self.owners[id].generation),
            state,
            revision: self.revision,
            kind,
        });
    }
}

impl PluginCleanup {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(ClosingState {
                status: watch::channel(PluginCleanupStatus::Waiting).0,
                retry: Notify::new(),
            }),
        }
    }

    pub fn status(&self) -> PluginCleanupStatus {
        self.shared.status.borrow().clone()
    }

    pub async fn join(&self) -> crate::Result<()> {
        let mut status = self.shared.status.subscribe();
        loop {
            match status.borrow_and_update().clone() {
                PluginCleanupStatus::Settled => return Ok(()),
                PluginCleanupStatus::Failed(source) => {
                    return Err(Error::TaskFailed { domain: "plugin manager cleanup", source });
                }
                PluginCleanupStatus::Waiting | PluginCleanupStatus::Running => {}
            }
            status.changed().await.expect("cleanup receipt owns its status");
        }
    }

    pub async fn retry(&self) -> crate::Result<()> {
        let retry = self.shared.status.send_if_modified(|status| {
            if matches!(status, PluginCleanupStatus::Failed(_)) {
                *status = PluginCleanupStatus::Running;
                true
            } else {
                false
            }
        });
        if retry {
            self.shared.retry.notify_one();
        }
        self.join().await
    }

    pub(super) fn settled(&self) {
        self.shared.status.send_replace(PluginCleanupStatus::Settled);
    }

    pub(super) fn start(&self, closing: Closing) {
        self.shared.status.send_replace(PluginCleanupStatus::Running);
        let shared = self.shared.clone();
        let owner = Cleanup::new(closing);
        crate::supervision::executor::task(async move {
            loop {
                match owner.settle().await {
                    Ok(()) => {
                        drop(owner);
                        shared.status.send_replace(PluginCleanupStatus::Settled);
                        return;
                    }
                    Err(error) => {
                        shared.status.send_replace(PluginCleanupStatus::Failed(error));
                        shared.retry.notified().await;
                    }
                }
            }
        });
    }
}

impl Resource for Closing {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(async move {
            let outcomes = Arc::new(Mutex::new(BTreeMap::new()));
            let mut driver = Driver {
                schedule: self.topology.retirement(self.owners.keys().cloned().collect()),
                owners: self.owners.clone(),
                previous: self.previous.take(),
                router: self.router.clone(),
                events: self.events.clone(),
                revision: self.revision,
                outcomes: outcomes.clone(),
            };
            driver.finalize().await?;
            let outcomes = std::mem::take(&mut *outcomes.lock().expect("retirement outcomes are healthy"));
            let mut failure = None;
            for (id, result) in outcomes {
                match result {
                    Ok(()) => {
                        self.owners.remove(&id);
                    }
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        })
    }
}
